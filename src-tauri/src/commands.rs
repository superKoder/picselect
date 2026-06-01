use tauri::State;
use std::path::{Path, PathBuf};
use std::fs;
use std::sync::{Arc, Mutex};
use crate::config::{ProjectConfig, RecentProjects};
use crate::media::{discover_media, MediaItem};
use crate::cache::{CacheManager, CacheStats, DecodedImage};
use crate::undo::{UndoManager, UndoTransaction, OperationType};

pub struct AppState {
    pub project_file_path: Mutex<Option<PathBuf>>,
    pub config: Mutex<Option<ProjectConfig>>,
    pub cache: CacheManager,
    pub undo: Mutex<UndoManager>,
    pub app_data_dir: PathBuf,
}

#[tauri::command]
pub fn get_recent_projects(state: State<'_, AppState>) -> Result<RecentProjects, String> {
    Ok(RecentProjects::load_global(&state.app_data_dir))
}

#[tauri::command]
pub fn open_project(file_path: String, state: State<'_, AppState>) -> Result<ProjectConfig, String> {
    let path = PathBuf::from(&file_path);
    let config = ProjectConfig::load(&path)?;
    
    // Save to active state
    *state.project_file_path.lock().unwrap() = Some(path.clone());
    *state.config.lock().unwrap() = Some(config.clone());
    
    // Enforce cache limit from config
    state.cache.set_max_memory_mib(config.cache_limit_mib);
    
    // Update recent projects
    let mut recents = RecentProjects::load_global(&state.app_data_dir);
    recents.add_recent(file_path);
    let _ = recents.save_global(&state.app_data_dir);
    
    // Clear undo stack on project switch
    state.undo.lock().unwrap().clear();
    
    Ok(config)
}

#[tauri::command]
pub fn create_project(
    intake_dir: String,
    selected_dir: String,
    deleted_dir: String,
    cache_limit_mib: u64,
    project_file_path: String,
    state: State<'_, AppState>,
) -> Result<ProjectConfig, String> {
    let config = ProjectConfig {
        intake_dir,
        selected_dir,
        deleted_dir,
        cache_limit_mib,
    };
    
    let path = PathBuf::from(&project_file_path);
    config.save(&path)?;
    
    // Save to active state
    *state.project_file_path.lock().unwrap() = Some(path.clone());
    *state.config.lock().unwrap() = Some(config.clone());
    
    // Enforce cache limit
    state.cache.set_max_memory_mib(config.cache_limit_mib);
    
    // Update recents
    let mut recents = RecentProjects::load_global(&state.app_data_dir);
    recents.add_recent(project_file_path);
    let _ = recents.save_global(&state.app_data_dir);
    
    // Clear undo stack
    state.undo.lock().unwrap().clear();
    
    Ok(config)
}

#[tauri::command]
pub fn save_project_settings(
    selected_dir: String,
    deleted_dir: String,
    cache_limit_mib: u64,
    state: State<'_, AppState>,
) -> Result<ProjectConfig, String> {
    let mut config_lock = state.config.lock().unwrap();
    let file_path_lock = state.project_file_path.lock().unwrap();
    
    let mut config = config_lock.as_mut()
        .ok_or_else(|| "No active project loaded".to_string())?
        .clone();
    
    config.selected_dir = selected_dir;
    config.deleted_dir = deleted_dir;
    config.cache_limit_mib = cache_limit_mib;
    
    if let Some(ref path) = *file_path_lock {
        config.save(path)?;
    }
    
    *config_lock = Some(config.clone());
    state.cache.set_max_memory_mib(cache_limit_mib);
    
    Ok(config)
}

#[tauri::command]
pub fn get_media(state: State<'_, AppState>) -> Result<Vec<MediaItem>, String> {
    let config_lock = state.config.lock().unwrap();
    let file_path_lock = state.project_file_path.lock().unwrap();
    
    let config = config_lock.as_ref()
        .ok_or_else(|| "No active project".to_string())?;
    let proj_file = file_path_lock.as_ref()
        .ok_or_else(|| "No active project".to_string())?;
    
    let intake_path = config.resolve_path(proj_file, &config.intake_dir);
    discover_media(&intake_path)
}

#[tauri::command]
pub fn get_cache_stats(state: State<'_, AppState>) -> CacheStats {
    state.cache.get_stats()
}

#[tauri::command]
pub async fn set_active_lookahead(
    active_media_items: Vec<MediaItem>,
    lookahead_media_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<CacheStats, String> {
    // 1. Update the cache preloader active window (aborts non-active tasks)
    state.cache.update_lookahead_window(&lookahead_media_ids);
    
    let config_lock = state.config.lock().unwrap();
    let file_path_lock = state.project_file_path.lock().unwrap();
    
    let config = config_lock.as_ref()
        .ok_or_else(|| "No active project".to_string())?
        .clone();
    let proj_file = file_path_lock.as_ref()
        .ok_or_else(|| "No active project".to_string())?
        .clone();
    
    // 2. Spawn preloading tasks for items in lookahead list in background thread pool
    for item in active_media_items {
        if !lookahead_media_ids.contains(&item.id) {
            continue;
        }
        
        // Skip if already in memory cache (both image and video components if present)
        let mut needs_preload = false;
        
        if let Some(ref img_path_str) = item.image_path {
            let ext = Path::new(img_path_str).extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            if ext == "heic" {
                if state.cache.get_l1(&item.id).is_none() {
                    needs_preload = true;
                }
            } else {
                if state.cache.get_l2(img_path_str).is_none() {
                    needs_preload = true;
                }
            }
        }
        
        if let Some(ref vid_path_str) = item.video_path {
            if state.cache.get_l2(vid_path_str).is_none() {
                if let Ok(metadata) = fs::metadata(vid_path_str) {
                    if metadata.len() < 50 * 1024 * 1024 { // Under 50 MiB
                        needs_preload = true;
                    }
                }
            }
        }
        
        if !needs_preload {
            continue;
        }
        
        // Skip if a preloading task is already active for this item
        let mut tasks = state.cache.loading_tasks.lock().unwrap();
        if tasks.contains_key(&item.id) {
            continue;
        }
        
        let cache = state.cache.clone();
        let item_clone = item.clone();
        let cache_clone = state.cache.clone();
        let media_id_clone = item.id.clone();
        
        // Spawn background task
        let handle = tokio::spawn(async move {
            // 1. Preload Image if present
            if let Some(ref img_path_str) = item_clone.image_path {
                let img_path = PathBuf::from(img_path_str);
                let ext = img_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                
                // If it is HEIC, we want to L1 pre-decode it (only on non-macOS platforms)
                if ext == "heic" {
                    #[cfg(target_os = "macos")]
                    {
                        // On macOS, WebKit natively handles HEIC hardware-accelerated. Just pre-read to L2!
                        if cache.get_l2(img_path_str).is_none() {
                            if let Ok(bytes) = fs::read(&img_path) {
                                cache.insert_l2(img_path_str.clone(), Arc::new(bytes));
                            }
                        }
                    }
                    
                    #[cfg(not(target_os = "macos"))]
                    {
                        if cache.get_l1(&item_clone.id).is_none() {
                            // Pre-read into L2 if needed
                            let file_bytes = match cache.get_l2(img_path_str) {
                                Some(bytes) => bytes,
                                None => {
                                    if let Ok(bytes) = fs::read(&img_path) {
                                        let shared_bytes = Arc::new(bytes);
                                        cache.insert_l2(img_path_str.clone(), shared_bytes.clone());
                                        shared_bytes
                                    } else {
                                        Arc::new(Vec::new())
                                    }
                                }
                            };
                            
                            if !file_bytes.is_empty() {
                                let media_id = item_clone.id.clone();
                                let _ = tokio::task::spawn_blocking(move || {
                                    if let Ok(decoded) = CacheManager::decode_heic_to_rgba(&file_bytes) {
                                        let mut jpeg_bytes = Vec::new();
                                        let rgb: Vec<u8> = decoded.rgba
                                            .chunks_exact(4)
                                            .flat_map(|rgba| [rgba[0], rgba[1], rgba[2]])
                                            .collect();
                                        let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut jpeg_bytes);
                                        if encoder.encode(&rgb, decoded.width, decoded.height, image::ColorType::Rgb8.into()).is_ok() {
                                            cache.insert_l1(media_id, Arc::new(jpeg_bytes));
                                        }
                                    }
                                }).await;
                            }
                        }
                    }
                } else if ext == "jpg" || ext == "jpeg" || ext == "png" {
                    // Standard image: just pre-read into L2 cache
                    if cache.get_l2(img_path_str).is_none() {
                        if let Ok(bytes) = fs::read(&img_path) {
                            cache.insert_l2(img_path_str.clone(), Arc::new(bytes));
                        }
                    }
                }
            }
            
            // 2. Preload Video if present
            if let Some(ref vid_path_str) = item_clone.video_path {
                if cache.get_l2(vid_path_str).is_none() {
                    let vid_path = PathBuf::from(vid_path_str);
                    if let Ok(metadata) = fs::metadata(&vid_path) {
                        if metadata.len() < 50 * 1024 * 1024 { // Under 50 MiB
                            if let Ok(bytes) = fs::read(&vid_path) {
                                cache.insert_l2(vid_path_str.clone(), Arc::new(bytes));
                            }
                        }
                    }
                }
            }
            
            // Clean up: task finished loading successfully, remove self from loading_tasks map
            cache_clone.loading_tasks.lock().unwrap().remove(&media_id_clone);
        });
        
        tasks.insert(item.id.clone(), handle);
    }
    
    Ok(state.cache.get_stats())
}

#[tauri::command]
pub fn move_media_item(
    media_item: MediaItem,
    to_selected: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let config_lock = state.config.lock().unwrap();
    let file_path_lock = state.project_file_path.lock().unwrap();
    
    let config = config_lock.as_ref()
        .ok_or_else(|| "No active project".to_string())?;
    let proj_file = file_path_lock.as_ref()
        .ok_or_else(|| "No active project".to_string())?;
    
    // Resolve selected/deleted target directory
    let target_dir_str = if to_selected {
        &config.selected_dir
    } else {
        &config.deleted_dir
    };
    
    let target_dir = config.resolve_path(proj_file, target_dir_str);
    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create destination folder: {}", e))?;
    
    let mut moved_files = Vec::new();
    
    // Gather files to move (image and/or video)
    let mut paths_to_move = Vec::new();
    if let Some(ref p) = media_item.image_path {
        paths_to_move.push(PathBuf::from(p));
    }
    if let Some(ref p) = media_item.video_path {
        paths_to_move.push(PathBuf::from(p));
    }
    
    for src_path in paths_to_move {
        if !src_path.exists() {
            continue;
        }
        let filename = src_path.file_name().unwrap();
        let dest_path = target_dir.join(filename);
        
        // Move the file
        fs::rename(&src_path, &dest_path)
            .map_err(|e| format!("Failed to move file {:?}: {}", filename, e))?;
            
        moved_files.push((dest_path, src_path));
    }
    
    // Push onto undo stack
    let op_type = if to_selected { OperationType::Select } else { OperationType::Delete };
    state.undo.lock().unwrap().push(UndoTransaction {
        media_id: media_item.id,
        operation: op_type,
        moved_files,
    });
    
    Ok(())
}

#[tauri::command]
pub fn undo_action(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let mut undo_lock = state.undo.lock().unwrap();
    match undo_lock.undo_last()? {
        Some(transaction) => Ok(Some(transaction.media_id)),
        None => Ok(None),
    }
}

#[tauri::command]
pub fn rename_media_item(
    media_item: MediaItem,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Collect all paths to rename
    let mut src_paths = Vec::new();
    if let Some(ref p) = media_item.image_path {
        src_paths.push(PathBuf::from(p));
    }
    if let Some(ref p) = media_item.video_path {
        src_paths.push(PathBuf::from(p));
    }
    
    if src_paths.is_empty() {
        return Err("No files found to rename".to_string());
    }
    
    // 1. Verify collisions first! (No collision allowed, as per user request)
    let mut renames = Vec::new();
    for src_path in &src_paths {
        let parent = src_path.parent().unwrap();
        let ext = src_path.extension().unwrap();
        let new_filename = format!("{}.{}", new_name, ext.to_string_lossy());
        let dest_path = parent.join(new_filename);
        
        if dest_path.exists() {
            return Err(format!("A file named '{}' already exists in the intake folder.", dest_path.file_name().unwrap().to_string_lossy()));
        }
        renames.push((src_path, dest_path));
    }
    
    // 2. Perform the actual renames
    for (src, dest) in renames {
        fs::rename(src, dest)
            .map_err(|e| format!("Failed to rename file: {}", e))?;
    }
    
    Ok(())
}

#[tauri::command]
pub fn write_rotation_to_file(
    media_item: MediaItem,
    rotation: i32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let img_path_str = media_item.image_path
        .ok_or_else(|| "Item has no image".to_string())?;
    
    let path = Path::new(&img_path_str);
    if !path.exists() {
        return Err("Image file not found on disk".to_string());
    }
    
    // Read existing EXIF orientation to make visual rotation accumulative
    let mut existing_deg = 0;
    if let Ok(exif_deg) = crate::media::read_exif_orientation_degrees(path) {
        existing_deg = exif_deg;
    }
    
    // Add visual rotation delta
    let new_deg = (existing_deg + rotation) % 360;
    
    // Convert degrees back to EXIF Orientation tag value (1, 3, 6, 8)
    // Degrees: 0 -> 1, 90 -> 6, 180 -> 3, 270 -> 8
    let orientation_tag_val = match new_deg {
        90 => 6,
        180 => 3,
        270 => 8,
        _ => 1,
    };
    
    // Use little_exif to write the Orientation tag losslessly!
    let mut metadata = little_exif::metadata::Metadata::new_from_path(path)
        .map_err(|e| format!("Failed to read EXIF metadata: {:?}", e))?;
    
    metadata.set_tag(little_exif::exif_tag::ExifTag::Orientation(vec![orientation_tag_val as u16]));
    
    metadata.write_to_file(path)
        .map_err(|e| format!("Failed to write EXIF orientation: {:?}", e))?;
    
    // Evict this image from L1 cache so it will be reloaded and re-rendered with the new rotation!
    let mut cache_state = state.cache.state.lock().unwrap();
    if let Some(img) = cache_state.l1_cache.remove(&media_item.id) {
        cache_state.current_memory_bytes -= img.len();
        cache_state.l1_order.retain(|id| id != &media_item.id);
    }
    
    Ok(())
}
