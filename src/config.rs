use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::theme::ThemeConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub editor: EditorConfig,
    #[serde(default)]
    pub file_browser: FileBrowserConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    pub tab_size: usize,
    pub show_line_numbers: bool,
    pub syntax_highlighting: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            tab_size: 4,
            show_line_numbers: true,
            syntax_highlighting: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBrowserConfig {
    pub show_hidden: bool,
    pub confirm_delete: bool,
}

impl Default for FileBrowserConfig {
    fn default() -> Self {
        Self {
            show_hidden: false,
            confirm_delete: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeConfig::default(),
            editor: EditorConfig::default(),
            file_browser: FileBrowserConfig::default(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = get_config_path();
        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        Config::default()
    }

    pub fn create_default_if_not_exists() {
        let config_path = get_config_path();
        if !config_path.exists() {
            if let Some(parent) = config_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let config = Config::default();
            if let Ok(content) = toml::to_string_pretty(&config) {
                let _ = fs::write(&config_path, content);
            }
        }
    }
}

fn get_config_path() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "NeoFM", "nfm") {
        proj_dirs.config_dir().join("config.toml")
    } else {
        PathBuf::from("config.toml")
    }
}
