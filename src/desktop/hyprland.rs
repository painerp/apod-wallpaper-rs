use super::WallpaperManager;
use crate::utils::command_exists;
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[cfg(any(feature = "cli", feature = "gui"))]
use crate::utils::send_notification;

static WALLPAPER_TOOLS: OnceLock<WallpaperTools> = OnceLock::new();

struct WallpaperTools {
    has_hyprpaper: bool,
    has_swww: bool,
    has_swaybg: bool,
}

impl WallpaperTools {
    fn new() -> Self {
        let has_hyprpaper = command_exists("hyprpaper");
        let has_swww = command_exists("swww");
        let has_swaybg = command_exists("swaybg");

        if !has_hyprpaper && !has_swww && !has_swaybg {
            panic!(
                "No supported wallpaper tool found. Please install one of hyprpaper, swww, or swaybg."
            );
        }

        Self {
            has_hyprpaper,
            has_swww,
            has_swaybg,
        }
    }

    fn has_any(&self) -> bool {
        self.has_hyprpaper || self.has_swww || self.has_swaybg
    }
}

pub struct HyprlandManager {}

impl HyprlandManager {
    pub fn new() -> Self {
        WALLPAPER_TOOLS.get_or_init(WallpaperTools::new);
        if !WALLPAPER_TOOLS.get().unwrap().has_any() {
            panic!(
                "No supported wallpaper tool found. Please install one of hyprpaper, swww, or swaybg."
            );
        }
        Self {}
    }

    pub fn is_available() -> bool {
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
            || Command::new("hyprctl").arg("version").output().is_ok()
    }
}

impl WallpaperManager for HyprlandManager {
    fn get_screens(&self) -> Vec<String> {
        let output = Command::new("hyprctl").args(&["monitors", "-j"]).output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(monitors) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    if let Some(array) = monitors.as_array() {
                        return array
                            .iter()
                            .filter_map(|m| m["name"].as_str().map(String::from))
                            .collect();
                    }
                }
                vec!["default".to_string()]
            }
            Err(_) => vec!["default".to_string()],
        }
    }

    fn set_wallpaper(&self, path: &Path, screen: Option<&str>) -> Result<()> {
        let path_str = path.to_string_lossy();
        let tools = WALLPAPER_TOOLS.get().unwrap();

        if tools.has_hyprpaper {
            let command = match screen {
                Some(screen) => format!(
                    "hyprpaper preload {} && hyprctl hyprpaper wallpaper \"{},{}\"",
                    path_str, screen, path_str
                ),
                None => format!(
                    "hyprpaper preload {} && hyprctl hyprpaper wallpaper {}",
                    path_str, path_str
                ),
            };

            let output = Command::new("sh").arg("-c").arg(command).output()?;

            if output.status.success() {
                return Ok(());
            } else {
                eprintln!(
                    "hyprpaper command failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        if tools.has_swww {
            let command = match screen {
                Some(screen) => format!("swww img {} -o {} -t grow", path_str, screen),
                None => format!("swww img {} -t grow", path_str),
            };
            let output = Command::new("sh").arg("-c").arg(command).output()?;

            if output.status.success() {
                return Ok(());
            } else {
                eprintln!(
                    "swww command failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        if tools.has_swaybg {
            let output = Command::new("swaybg")
                .args(["-i", &path_str.to_string()])
                .spawn();

            if output.is_ok() {
                return Ok(());
            } else {
                eprintln!("swaybg command failed to start");
            }
        }

        Err(Error::DesktopEnv(
            "Failed to set wallpaper. No supported wallpaper tool (hyprpaper, swww, or swaybg) is available".to_string()
        ))
    }

    fn get_wallpaper(&self, screen: Option<&str>) -> Result<Option<PathBuf>> {
        let tools = WALLPAPER_TOOLS.get().unwrap();

        if tools.has_hyprpaper {
            let output = Command::new("hyprctl")
                .args(&["hyprpaper", "wallpaper"])
                .output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let wallpaper_path = stdout.trim();
                if !wallpaper_path.is_empty() {
                    return Ok(Some(PathBuf::from(wallpaper_path)));
                }
            } else {
                eprintln!(
                    "hyprctl hyprpaper wallpaper command failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        if tools.has_swww {
            let output = Command::new("swww").arg("query").output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let nl = match screen {
                        Some(screen) => {
                            if line
                                .to_lowercase()
                                .starts_with(screen.to_lowercase().as_str())
                            {
                                line
                            } else {
                                continue;
                            }
                        }
                        None => line,
                    };
                    if let Some(idx) = nl.find("image: ") {
                        let path = nl[idx + 7..].trim();
                        if !path.is_empty() {
                            return Ok(Some(PathBuf::from(path)));
                        }
                    }
                }
            } else {
                eprintln!(
                    "swww query command failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        if tools.has_swaybg {
            return Ok(None);
        }

        Err(Error::DesktopEnv(
            "Failed to get wallpaper. No supported wallpaper tool (hyprpaper, swww, or swaybg) is available".to_string(),
        ))
    }

    fn notify(&self, title: &str, message: &str, image: Option<&Path>) -> Result<()> {
        #[cfg(any(feature = "cli", feature = "gui"))]
        {
            send_notification(title, message, image)?;
        }
        Ok(())
    }
}
