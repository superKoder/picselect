mod config;
mod media;
mod cache;
mod undo;
mod commands;

use std::sync::{Arc, Mutex};
use std::path::{Path, PathBuf};
use std::fs;
use tauri::{Manager, AppHandle};
use crate::commands::AppState;
use crate::cache::{CacheManager, DecodedImage};
use crate::undo::UndoManager;

fn encode_rgba_to_jpeg(decoded: &DecodedImage) -> Result<Vec<u8>, String> {
    let mut jpeg_bytes = Vec::new();
    // Convert RGBA to RGB (strip alpha channel for standard JPEG compatibility)
    let rgb: Vec<u8> = decoded.rgba
        .chunks_exact(4)
        .flat_map(|rgba| [rgba[0], rgba[1], rgba[2]])
        .collect();
    
    let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut jpeg_bytes);
    encoder.encode(&rgb, decoded.width, decoded.height, image::ColorType::Rgb8.into())
        .map_err(|e| format!("Failed to encode JPEG: {}", e))?;
    
    Ok(jpeg_bytes)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                PathBuf::from("./.picselect_data")
            });
            
            // Initialize AppState
            app.manage(AppState {
                project_file_path: Mutex::new(None),
                config: Mutex::new(None),
                cache: CacheManager::new(1024), // Default 1 GiB cache limit
                undo: Mutex::new(UndoManager::new()),
                app_data_dir,
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_recent_projects,
            commands::open_project,
            commands::create_project,
            commands::save_project_settings,
            commands::get_media,
            commands::get_cache_stats,
            commands::set_active_lookahead,
            commands::move_media_item,
            commands::undo_action,
            commands::rename_media_item,
            commands::write_rotation_to_file
        ])
        .register_asynchronous_uri_scheme_protocol("picselect-asset", |context, request, responder| {
            let app_handle = context.app_handle().clone();
            let request_uri = request.uri().clone();
            let range_header = request.headers()
                .get("range")
                .or_else(|| request.headers().get("Range"))
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            
            std::thread::spawn(move || {
                let state = app_handle.state::<AppState>();
                let path_str = request_uri.path();
                let decoded_path = percent_encoding::percent_decode_str(path_str)
                    .decode_utf8_lossy()
                    .into_owned();
                
                println!("[picselect-asset] Request: {}", decoded_path);
                let file_path = PathBuf::from(&decoded_path);
                if !file_path.exists() {
                    responder.respond(
                        tauri::http::Response::builder()
                            .status(404)
                            .body(Vec::new())
                            .unwrap()
                    );
                    return;
                }

                let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                
                if ext == "heic" {
                    let media_id = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                    
                    #[cfg(target_os = "macos")]
                    {
                        // On macOS, WKWebView natively supports hardware-accelerated HEIC! Serve raw bytes immediately!
                        println!("[picselect-asset] Native macOS HEIC HIT for {}", media_id);
                        if let Some(bytes) = state.cache.get_l2(&decoded_path) {
                            responder.respond(
                                tauri::http::Response::builder()
                                    .header("Content-Type", "image/heic")
                                    .body(bytes.as_ref().clone())
                                    .unwrap()
                            );
                            return;
                        }
                        if let Ok(bytes) = fs::read(&file_path) {
                            let shared_bytes = Arc::new(bytes);
                            state.cache.insert_l2(decoded_path, shared_bytes.clone());
                            responder.respond(
                                tauri::http::Response::builder()
                                    .header("Content-Type", "image/heic")
                                    .body(shared_bytes.as_ref().clone())
                                    .unwrap()
                            );
                            return;
                        }
                    }

                    #[cfg(not(target_os = "macos"))]
                    {
                        // Cooperatively sleep-wait if there is an in-flight background preloading task for this item
                        let mut retries = 0;
                        while state.cache.get_l1(&media_id).is_none() && retries < 40 {
                            let has_task = {
                                let tasks = state.cache.loading_tasks.lock().unwrap();
                                tasks.contains_key(&media_id)
                            };
                            if !has_task {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(50));
                            retries += 1;
                        }
                        
                        // 1. Check L1 cache for already pre-encoded JPEG
                        if let Some(jpeg_bytes) = state.cache.get_l1(&media_id) {
                            println!("[picselect-asset] L1 HIT for HEIC {}", media_id);
                            responder.respond(
                                tauri::http::Response::builder()
                                    .header("Content-Type", "image/jpeg")
                                    .body(jpeg_bytes.as_ref().clone())
                                    .unwrap()
                            );
                            return;
                        }
                        
                        // 2. Decode from raw bytes (cache miss)
                        println!("[picselect-asset] L1 MISS for HEIC {}", media_id);
                        
                        // Check if this request has already gone stale (user navigated away) before doing expensive decoding
                        {
                            let cache_state = state.cache.state.lock().unwrap();
                            if !cache_state.active_window.is_empty() && !cache_state.active_window.contains(&media_id) {
                                println!("[picselect-asset] DISCARDING stale request for HEIC {}", media_id);
                                responder.respond(
                                    tauri::http::Response::builder()
                                        .status(204) // No Content (tells the browser it was discarded)
                                        .body(Vec::new())
                                        .unwrap()
                                );
                                return;
                            }
                        }

                        let file_bytes = match state.cache.get_l2(&decoded_path) {
                            Some(bytes) => {
                                println!("[picselect-asset] L2 HIT for HEIC raw {}", decoded_path);
                                bytes
                            }
                            None => {
                                println!("[picselect-asset] L2 MISS for HEIC raw {}", decoded_path);
                                if let Ok(bytes) = fs::read(&file_path) {
                                    let shared_bytes = Arc::new(bytes);
                                    state.cache.insert_l2(decoded_path.clone(), shared_bytes.clone());
                                    shared_bytes
                                } else {
                                    responder.respond(
                                        tauri::http::Response::builder()
                                            .status(500)
                                            .body(Vec::new())
                                            .unwrap()
                                    );
                                    return;
                                }
                            }
                        };
                        
                        if let Ok(decoded) = CacheManager::decode_heic_to_rgba(&file_bytes) {
                            if let Ok(jpeg_bytes) = encode_rgba_to_jpeg(&decoded) {
                                let shared_jpeg = Arc::new(jpeg_bytes);
                                state.cache.insert_l1(media_id.clone(), shared_jpeg.clone());
                                println!("[picselect-asset] HEIC {} successfully decoded and inserted into L1", media_id);
                                responder.respond(
                                    tauri::http::Response::builder()
                                        .header("Content-Type", "image/jpeg")
                                        .body(shared_jpeg.as_ref().clone())
                                        .unwrap()
                                );
                                return;
                            }
                        }
                    }
                } else {
                    // Standard JPEG/PNG/MOV/MP4
                    let mime_type = match ext.as_str() {
                        "jpg" | "jpeg" => "image/jpeg",
                        "png" => "image/png",
                        "mp4" => "video/mp4",
                        "mov" => "video/quicktime",
                        _ => "application/octet-stream",
                    };
                    
                    // Check L2 cache
                    let bytes = if let Some(bytes) = state.cache.get_l2(&decoded_path) {
                        println!("[picselect-asset] L2 HIT for path {}", decoded_path);
                        bytes
                    } else {
                        // Read file
                        println!("[picselect-asset] L2 MISS for path {}", decoded_path);
                        if let Ok(file_bytes) = fs::read(&file_path) {
                            let shared_bytes = Arc::new(file_bytes);
                            state.cache.insert_l2(decoded_path, shared_bytes.clone());
                            shared_bytes
                        } else {
                            responder.respond(
                                tauri::http::Response::builder()
                                    .status(500)
                                    .body(Vec::new())
                                    .unwrap()
                            );
                            return;
                        }
                    };

                    let file_len = bytes.len();
                    
                    // Support Range HTTP Requests for streaming
                    let mut parsed_range = None;
                    if let Some(ref range_str) = range_header {
                        if range_str.starts_with("bytes=") {
                            let range_parts: Vec<&str> = range_str["bytes=".len()..].split('-').collect();
                            if range_parts.len() == 2 {
                                let start_str = range_parts[0].trim();
                                let end_str = range_parts[1].trim();
                                
                                if !start_str.is_empty() || !end_str.is_empty() {
                                    if start_str.is_empty() {
                                        if let Ok(suffix_len) = end_str.parse::<usize>() {
                                            let start = if file_len > suffix_len { file_len - suffix_len } else { 0 };
                                            parsed_range = Some((start, file_len - 1));
                                        }
                                    } else if end_str.is_empty() {
                                        if let Ok(start) = start_str.parse::<usize>() {
                                            if start < file_len {
                                                parsed_range = Some((start, file_len - 1));
                                            }
                                        }
                                    } else {
                                        if let (Ok(start), Ok(mut end)) = (start_str.parse::<usize>(), end_str.parse::<usize>()) {
                                            if start < file_len {
                                                if end >= file_len {
                                                    end = file_len - 1;
                                                }
                                                if start <= end {
                                                    parsed_range = Some((start, end));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    if let Some((start, end)) = parsed_range {
                        let chunk = bytes[start..=end].to_vec();
                        let content_range = format!("bytes {}-{}/{}", start, end, file_len);
                        responder.respond(
                            tauri::http::Response::builder()
                                .status(206)
                                .header("Content-Type", mime_type)
                                .header("Accept-Ranges", "bytes")
                                .header("Content-Range", content_range)
                                .header("Content-Length", chunk.len().to_string())
                                .body(chunk)
                                .unwrap()
                        );
                    } else {
                        responder.respond(
                            tauri::http::Response::builder()
                                .status(200)
                                .header("Content-Type", mime_type)
                                .header("Accept-Ranges", "bytes")
                                .header("Content-Length", file_len.to_string())
                                .body(bytes.as_ref().clone())
                                .unwrap()
                        );
                    }
                    return;
                }
                
                // Final error fallback
                println!("[picselect-asset] FAILED request for path: {}", decoded_path);
                responder.respond(
                    tauri::http::Response::builder()
                        .status(500)
                        .body(Vec::new())
                        .unwrap()
                );
            });
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
