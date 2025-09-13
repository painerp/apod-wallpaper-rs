use crate::Result;
use std::path::{Path, PathBuf};

pub mod hyprland;
pub mod plasma;

pub trait WallpaperManager {
    fn get_screens(&self) -> Vec<String>;
    fn set_wallpaper(&self, path: &Path, screen: Option<&str>) -> Result<()>;
    fn get_wallpaper(&self, screen: Option<&str>) -> Result<Option<PathBuf>>;
    fn notify(&self, title: &str, message: &str, image: Option<&Path>) -> Result<()>;
}

pub fn get_wallpaper_manager() -> Result<Box<dyn WallpaperManager>> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();

    match desktop.to_lowercase().as_str() {
        "hyprland" => Ok(Box::new(hyprland::HyprlandManager::new())),
        "kde" | "plasma" => Ok(Box::new(plasma::PlasmaManager::new())),
        _ => {
            if hyprland::HyprlandManager::is_available() {
                Ok(Box::new(hyprland::HyprlandManager::new()))
            } else if plasma::PlasmaManager::is_available() {
                Ok(Box::new(plasma::PlasmaManager::new()))
            } else {
                Err(crate::Error::DesktopEnv(
                    "No supported desktop environment found".to_string(),
                ))
            }
        }
    }
}
