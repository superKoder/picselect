use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaItem {
    pub id: String,
    pub name: String, // Base name without extension (e.g., IMG_1234)
    pub has_image: bool,
    pub has_video: bool,
    pub image_path: Option<String>,
    pub video_path: Option<String>,
    pub is_live: bool,
    pub rotation: i32, // Clockwise rotation in degrees (0, 90, 180, 270)
}

/// Scans the intake directory and groups matching files into MediaItems.
pub fn discover_media(intake_dir: &Path) -> Result<Vec<MediaItem>, String> {
    if !intake_dir.exists() || !intake_dir.is_dir() {
        return Err(format!("Intake directory does not exist or is not a directory: {:?}", intake_dir));
    }

    let entries = fs::read_dir(intake_dir)
        .map_err(|e| format!("Failed to read intake directory: {}", e))?;

    // Map of base_name -> (image_path, video_path)
    let mut groups: BTreeMap<String, (Option<PathBuf>, Option<PathBuf>)> = BTreeMap::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let ext_lower = ext.to_lowercase();
            let is_image = matches!(ext_lower.as_str(), "jpg" | "jpeg" | "heic" | "png");
            let is_video = matches!(ext_lower.as_str(), "mov" | "mp4");

            if !is_image && !is_video {
                continue;
            }

            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                // Group by stem (base name), e.g. "IMG_1234"
                let group = groups.entry(stem.to_string()).or_insert((None, None));
                if is_image {
                    group.0 = Some(path.clone());
                } else if is_video {
                    group.1 = Some(path.clone());
                }
            }
        }
    }

    let mut media_items = Vec::new();
    for (base_name, (image_opt, video_opt)) in groups {
        if image_opt.is_none() && video_opt.is_none() {
            continue;
        }

        let is_live = image_opt.is_some() && video_opt.is_some();
        let image_path = image_opt.map(|p| p.to_string_lossy().to_string());
        let video_path = video_opt.map(|p| p.to_string_lossy().to_string());

        media_items.push(MediaItem {
            id: base_name.clone(),
            name: base_name,
            has_image: image_path.is_some(),
            has_video: video_path.is_some(),
            image_path,
            video_path,
            is_live,
            rotation: 0,
        });
    }

    Ok(media_items)
}

/// Helper to read EXIF orientation and convert it to degrees (0, 90, 180, 270)
pub fn read_exif_orientation_degrees(path: &Path) -> Result<i32, String> {
    if !path.exists() {
        return Err("File not found".to_string());
    }

    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bufreader = std::io::BufReader::new(file);
    let exifreader = exif::Reader::new();
    
    if let Ok(exif) = exifreader.read_from_container(&mut bufreader) {
        if let Some(field) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
            if let Some(val) = field.value.get_uint(0) {
                let deg = match val {
                    3 => 180,
                    6 => 90,  // rotated 90 degrees CCW (so rotate 90 CW to display normal)
                    8 => 270, // rotated 90 degrees CW (so rotate 270 CW to display normal)
                    _ => 0,
                };
                return Ok(deg);
            }
        }
    }
    Ok(0)
}
