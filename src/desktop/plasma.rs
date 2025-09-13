use super::WallpaperManager;
use crate::utils::command_exists;
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(any(feature = "cli", feature = "gui"))]
use crate::utils::send_notification;

pub struct PlasmaManager;

impl PlasmaManager {
    pub fn new() -> Self {
        if !command_exists("qdbus") {
            panic!("QDBus command not found. Please install qdbus.");
        }
        Self
    }

    pub fn is_available() -> bool {
        std::env::var("KDE_SESSION_VERSION").is_ok()
    }
}

impl WallpaperManager for PlasmaManager {
    fn get_screens(&self) -> Vec<String> {
        let output = Command::new("qdbus")
            .args(&["org.kde.KWin", "/KWin", "org.kde.KWin.supportInformation"])
            .output();

        match output {
            Ok(_) => {
                // TODO: Parse actual screen names
                vec!["default".to_string()]
            }
            Err(_) => vec!["default".to_string()],
        }
    }

    fn set_wallpaper(&self, path: &Path, _screen: Option<&str>) -> Result<()> {
        let path_str = path.to_string_lossy();

        // TODO: add screen support
        let script = format!(
            r#"
            qdbus org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript '
            var allDesktops = desktops();
            for (i=0;i<allDesktops.length;i++) {{
                d = allDesktops[i];
                d.wallpaperPlugin = "org.kde.image";
                d.currentConfigGroup = Array("Wallpaper", "org.kde.image", "General");
                d.writeConfig("Image", "file://{}");
            }}
            '
            "#,
            path_str
        );

        let output = Command::new("sh").arg("-c").arg(&script).output()?;

        if !output.status.success() {
            return Err(Error::DesktopEnv(format!(
                "Failed to set wallpaper: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    fn get_wallpaper(&self, _screen: Option<&str>) -> Result<Option<PathBuf>> {
        //TODO: add screen support
        let script = r#"
            var allDesktops = desktops();
            if (allDesktops.length > 0) {
                var d = allDesktops[0];
                d.currentConfigGroup = Array("Wallpaper", "org.kde.image", "General");
                print(d.readConfig("Image"));
            }
        "#;

        let output = Command::new("qdbus")
            .args(&[
                "org.kde.plasmashell",
                "/PlasmaShell",
                "org.kde.PlasmaShell.evaluateScript",
                script,
            ])
            .output()?;

        if !output.status.success() {
            return Ok(None);
        }

        let result = String::from_utf8_lossy(&output.stdout);
        let result = result.trim();

        if result.is_empty() || result == "undefined" {
            return Ok(None);
        }

        // Remove "file://" prefix if present
        let path_str = if result.starts_with("file://") {
            &result[7..]
        } else {
            result
        };

        Ok(Some(PathBuf::from(path_str)))
    }

    fn notify(&self, title: &str, message: &str, image: Option<&Path>) -> Result<()> {
        #[cfg(any(feature = "cli", feature = "gui"))]
        {
            if !command_exists("kdialog") {
                // Fallback to notify-rust
                return send_notification(title, message, image);
            }

            let mut cmd = Command::new("kdialog");
            cmd.args(&["--title", title, "--passivepopup", message, "5"]);

            if let Some(image_path) = image {
                cmd.args(&["--icon", &image_path.to_string_lossy()]);
            }

            let output = cmd.output()?;

            if !output.status.success() {
                // Fallback to notify-rust
                return send_notification(title, message, image);
            }

            Ok(())
        }
        #[cfg(not(any(feature = "cli", feature = "gui")))]
        {
            Ok(())
        }
    }
}
