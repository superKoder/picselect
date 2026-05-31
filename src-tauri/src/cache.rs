use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::path::Path;
use std::fs;
use tokio::task::JoinHandle;
use serde::Serialize;

#[derive(Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl DecodedImage {
    pub fn size_in_bytes(&self) -> usize {
        self.rgba.len() + 16 // Pixels size + small metadata overhead
    }
}

pub struct CacheState {
    pub current_memory_bytes: usize,
    pub max_memory_bytes: usize,

    // L1: Pre-encoded JPEG bytes for HEIC images (media_id -> Vec<u8>)
    pub l1_cache: HashMap<String, Arc<Vec<u8>>>,
    pub l1_order: VecDeque<String>,

    // L2: Raw file bytes (file_path -> Vec<u8>)
    pub l2_cache: HashMap<String, Arc<Vec<u8>>>,
    pub l2_order: VecDeque<String>,

    // Active window of lookahead/active media IDs currently in view
    pub active_window: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct CacheStats {
    pub current_memory_mib: f64,
    pub max_memory_mib: f64,
    pub l1_count: usize,
    pub l2_count: usize,
}

#[derive(Clone)]
pub struct CacheManager {
    pub state: Arc<Mutex<CacheState>>,
    // Track active loading tasks: media_id -> JoinHandle
    pub loading_tasks: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
}

impl CacheManager {
    pub fn new(max_memory_mib: u64) -> Self {
        let max_memory_bytes = (max_memory_mib as usize) * 1024 * 1024;
        Self {
            state: Arc::new(Mutex::new(CacheState {
                current_memory_bytes: 0,
                max_memory_bytes,
                l1_cache: HashMap::new(),
                l1_order: VecDeque::new(),
                l2_cache: HashMap::new(),
                l2_order: VecDeque::new(),
                active_window: Vec::new(),
            })),
            loading_tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn set_max_memory_mib(&self, max_memory_mib: u64) {
        let mut state = self.state.lock().unwrap();
        state.max_memory_bytes = (max_memory_mib as usize) * 1024 * 1024;
        self.enforce_limit(&mut state);
    }

    pub fn get_stats(&self) -> CacheStats {
        let state = self.state.lock().unwrap();
        CacheStats {
            current_memory_mib: state.current_memory_bytes as f64 / 1024.0 / 1024.0,
            max_memory_mib: state.max_memory_bytes as f64 / 1024.0 / 1024.0,
            l1_count: state.l1_cache.len(),
            l2_count: state.l2_cache.len(),
        }
    }

    /// Tries to get an L1 pre-encoded JPEG from cache.
    pub fn get_l1(&self, media_id: &str) -> Option<Arc<Vec<u8>>> {
        let mut state = self.state.lock().unwrap();
        if let Some(bytes) = state.l1_cache.get(media_id).cloned() {
            // Move to back of LRU (most recently used)
            state.l1_order.retain(|id| id != media_id);
            state.l1_order.push_back(media_id.to_string());
            Some(bytes)
        } else {
            None
        }
    }

    /// Tries to get L2 raw bytes from cache.
    pub fn get_l2(&self, file_path: &str) -> Option<Arc<Vec<u8>>> {
        let mut state = self.state.lock().unwrap();
        if let Some(bytes) = state.l2_cache.get(file_path).cloned() {
            // Move to back of LRU
            state.l2_order.retain(|p| p != file_path);
            state.l2_order.push_back(file_path.to_string());
            Some(bytes)
        } else {
            None
        }
    }

    /// Inserts an L1 pre-encoded JPEG, enforcing the memory limit.
    pub fn insert_l1(&self, media_id: String, bytes: Arc<Vec<u8>>) {
        let mut state = self.state.lock().unwrap();
        let size = bytes.len();
        
        // If already present, subtract old size
        if let Some(old) = state.l1_cache.insert(media_id.clone(), bytes) {
            state.current_memory_bytes -= old.len();
            state.l1_order.retain(|id| id != &media_id);
        }
        
        state.current_memory_bytes += size;
        state.l1_order.push_back(media_id);
        
        self.enforce_limit(&mut state);
    }

    /// Inserts L2 raw bytes, enforcing the memory limit.
    pub fn insert_l2(&self, file_path: String, bytes: Arc<Vec<u8>>) {
        let mut state = self.state.lock().unwrap();
        let size = bytes.len();
        
        if let Some(old) = state.l2_cache.insert(file_path.clone(), bytes) {
            state.current_memory_bytes -= old.len();
            state.l2_order.retain(|p| p != &file_path);
        }
        
        state.current_memory_bytes += size;
        state.l2_order.push_back(file_path);
        
        self.enforce_limit(&mut state);
    }

    /// Evicts LRU items until current_memory_bytes <= max_memory_bytes.
    fn enforce_limit(&self, state: &mut CacheState) {
        // Evict L1 items first (they take the most space)
        while state.current_memory_bytes > state.max_memory_bytes && !state.l1_order.is_empty() {
            if let Some(evict_id) = state.l1_order.pop_front() {
                if let Some(bytes) = state.l1_cache.remove(&evict_id) {
                    state.current_memory_bytes -= bytes.len();
                }
            }
        }

        // Evict L2 items next if still over the limit
        while state.current_memory_bytes > state.max_memory_bytes && !state.l2_order.is_empty() {
            if let Some(evict_path) = state.l2_order.pop_front() {
                if let Some(bytes) = state.l2_cache.remove(&evict_path) {
                    state.current_memory_bytes -= bytes.len();
                }
            }
        }
    }

    /// Aborts any background loading tasks not inside the active window.
    pub fn update_lookahead_window(&self, active_media_ids: &[String]) {
        // Save the active window first so the scheme protocol handler knows what is current
        {
            let mut state = self.state.lock().unwrap();
            state.active_window = active_media_ids.to_vec();
        }

        let mut tasks = self.loading_tasks.lock().unwrap();
        
        // Retain only tasks that are in the active window. Abort the rest!
        let active_set: std::collections::HashSet<&String> = active_media_ids.iter().collect();
        let mut to_remove = Vec::new();
        
        for (media_id, handle) in tasks.iter() {
            if !active_set.contains(media_id) {
                handle.abort();
                to_remove.push(media_id.clone());
            }
        }
        
        for id in to_remove {
            tasks.remove(&id);
        }
    }

    /// Decodes a HEIC file using pure-Rust heic crate.
    pub fn decode_heic_to_rgba(bytes: &[u8]) -> Result<DecodedImage, String> {
        let decoder = heic::DecoderConfig::new();
        let output = decoder
            .decode(bytes, heic::PixelLayout::Rgba8)
            .map_err(|e| format!("HEIC decoding failed: {:?}", e))?;
        
        Ok(DecodedImage {
            width: output.width,
            height: output.height,
            rgba: output.data,
        })
    }

    /// Decodes a standard JPEG/PNG image to RGBA using image crate.
    pub fn decode_standard_to_rgba(bytes: &[u8]) -> Result<DecodedImage, String> {
        let img = image::load_from_memory(bytes)
            .map_err(|e| format!("Image load failed: {}", e))?;
        let rgba = img.to_rgba8();
        
        Ok(DecodedImage {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        })
    }
}
