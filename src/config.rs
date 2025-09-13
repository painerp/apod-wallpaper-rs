use crate::utils::{get_cache_dir, get_config_dir};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, read_to_string, write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct WallpaperConfig {
    #[serde(default = "default_save_folder")]
    pub save_folder: PathBuf,
    #[serde(default)]
    pub multi_monitor: bool,
    #[serde(default)]
    pub random: bool,
    #[serde(default)]
    pub pywal: bool,
    #[serde(default)]
    pub wallust: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "Dark".to_string()
}

fn default_save_folder() -> PathBuf {
    dirs::picture_dir()
        .unwrap_or_else(|| {
            get_cache_dir().unwrap_or_else(|_| {
                panic!(
                    "{}",
                    Error::Config("Could not find picture directory".to_string())
                )
            })
        })
        .join(PathBuf::from("apod"))
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            save_folder: default_save_folder(),
            multi_monitor: false,
            random: false,
            pywal: false,
            wallust: false,
            theme: default_theme(),
        }
    }
}

impl WallpaperConfig {
    pub fn load_or_default() -> Result<Self> {
        let config_dir = get_config_dir()?;

        create_dir_all(&config_dir)?;
        let config_path = config_dir.join(PathBuf::from("config.json"));

        if config_path.exists() {
            let content = read_to_string(&config_path)?;
            let config: Self =
                serde_json::from_str(&content).map_err(|e| Error::Config(e.to_string()))?;
            config.save()?;
            Ok(config)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| Error::Config("Could not find config directory".to_string()))?
            .join(PathBuf::from("apodwallpaper"));

        create_dir_all(&config_dir)?;
        let config_path = config_dir.join(PathBuf::from("config.json"));
        let content =
            serde_json::to_string_pretty(self).map_err(|e| Error::Config(e.to_string()))?;
        write(&config_path, content)?;
        Ok(())
    }
}
