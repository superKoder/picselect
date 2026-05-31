use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    // These paths can be absolute or relative to the directory containing the config file.
    pub intake_dir: String,
    pub selected_dir: String,
    pub deleted_dir: String,
    pub cache_limit_mib: u64,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            intake_dir: ".".to_string(),
            selected_dir: "./selected".to_string(),
            deleted_dir: "./deleted".to_string(),
            cache_limit_mib: 1024, // Default 1 GiB
        }
    }
}

impl ProjectConfig {
    /// Loads a ProjectConfig from a given path.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read project file: {}", e))?;
        let config: ProjectConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse project file: {}", e))?;
        Ok(config)
    }

    /// Saves the ProjectConfig to a given path.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(path, content)
            .map_err(|e| format!("Failed to write project file: {}", e))?;
        Ok(())
    }

    /// Resolves a path relative to the directory containing this config file, if it is relative.
    pub fn resolve_path<P: AsRef<Path>>(&self, config_file_path: P, path_str: &str) -> PathBuf {
        let path = Path::new(path_str);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            if let Some(parent) = config_file_path.as_ref().parent() {
                parent.join(path)
            } else {
                path.to_path_buf()
            }
        }
    }
}

/// Helper to manage recent projects in the app's global data dir.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RecentProjects {
    pub last_project_path: Option<String>,
    pub all_recent_paths: Vec<String>,
}

impl RecentProjects {
    pub fn load_global(app_data_dir: &Path) -> Self {
        let path = app_data_dir.join("recent_projects.json");
        if !path.exists() {
            return Self::default();
        }
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(recents) = serde_json::from_str::<RecentProjects>(&content) {
                return recents;
            }
        }
        Self::default()
    }

    pub fn save_global(&self, app_data_dir: &Path) -> Result<(), String> {
        let _ = fs::create_dir_all(app_data_dir);
        let path = app_data_dir.join("recent_projects.json");
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize recents: {}", e))?;
        fs::write(path, content)
            .map_err(|e| format!("Failed to write recents file: {}", e))?;
        Ok(())
    }

    pub fn add_recent(&mut self, path: String) {
        self.last_project_path = Some(path.clone());
        self.all_recent_paths.retain(|p| p != &path);
        self.all_recent_paths.insert(0, path);
        if self.all_recent_paths.len() > 10 {
            self.all_recent_paths.truncate(10);
        }
    }
}
