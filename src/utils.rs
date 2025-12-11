use crate::{Error, Result};
use std::fs::{create_dir, write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(feature = "gui")]
use tokio::fs;

const NASA_SVG: &[u8] = include_bytes!("../assets/nasa.svg");

pub fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn get_nasa_svg_path() -> Result<PathBuf> {
    let mut location = get_cache_dir()?;
    location.push(PathBuf::from("assets"));
    if !location.exists() {
        create_dir(&location)?;
    }
    location.push("nasa.svg");

    if !location.exists() {
        write(&location.as_path(), NASA_SVG)?;
    }
    Ok(location)
}

pub fn get_cache_dir() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .map(|dir| dir.join(PathBuf::from("apodwallpaper")))
        .ok_or_else(|| Error::DesktopEnv("Could not find cache directory".to_string()));
    if let Ok(ref dir) = cache_dir {
        if !dir.exists() {
            create_dir(dir)?;
        }
    }
    cache_dir
}

#[cfg(any(feature = "cli", feature = "gui"))]
pub fn get_config_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .or_else(|| {
            std::env::var("HOME").ok().map(|home| {
                PathBuf::from(home).join(".config")
            })
        })
        .map(|dir| dir.join(PathBuf::from("apodwallpaper")))
        .ok_or_else(|| Error::DesktopEnv(
            "Could not find config directory. Please set HOME or XDG_CONFIG_HOME environment variable.".to_string()
        ))?;

    if !config_dir.exists() {
        create_dir(&config_dir)?;
    }

    Ok(config_dir)
}

#[cfg(any(feature = "cli", feature = "gui"))]
pub fn generate_pywal_colors(image_path: &Path) -> Result<()> {
    if !command_exists("wal") {
        return Err(Error::DesktopEnv(
            "pywal (wal) command not found in PATH".to_string(),
        ));
    }

    let output = Command::new("wal")
        .args(["-i", &image_path.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        return Err(Error::DesktopEnv(format!(
            "Failed to generate pywal colors: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(())
}

#[cfg(any(feature = "cli", feature = "gui"))]
pub fn generate_wallust_colors(image_path: &Path) -> Result<()> {
    if !command_exists("wallust") {
        return Err(Error::DesktopEnv("wallust not found in PATH".to_string()));
    }

    let output = Command::new("wallust")
        .args(["run", &image_path.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        return Err(Error::DesktopEnv(format!(
            "Failed to generate wallust colors: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(())
}

#[cfg(feature = "gui")]
pub async fn get_image_files(
    directory: &Path,
) -> std::result::Result<Vec<PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    let mut images = Vec::new();

    if !directory.exists() {
        println!("Directory doesn't exist: {}", directory.display());
        return Ok(images);
    }

    let mut entries = fs::read_dir(directory).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if let Some(extension) = path.extension() {
            let ext_str = extension.to_string_lossy().to_lowercase();
            if matches!(ext_str.as_str(), "jpg" | "jpeg" | "png") {
                images.push(path);
            }
        }
    }

    images.sort();
    Ok(images)
}

#[cfg(feature = "gui")]
pub fn generate_thumbnail(image_path: &Path, thumbnail_path: &Path, size: u32) -> Result<()> {
    let img = image::open(image_path)?;
    let thumbnail = img.thumbnail(size, size);

    if let Some(parent) = thumbnail_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    thumbnail.save(thumbnail_path)?;
    Ok(())
}

#[cfg(any(feature = "cli", feature = "gui"))]
pub fn send_notification(title: &str, message: &str, image: Option<&Path>) -> Result<()> {
    let mut notification = notify_rust::Notification::new();
    notification.summary(title).body(message);

    if let Some(image_path) = image {
        notification.image_path(image_path.to_string_lossy().as_ref());
    }

    notification
        .show()
        .map_err(|e| Error::DesktopEnv(e.to_string()))?;
    Ok(())
}

#[cfg(feature = "applet")]
pub fn get_metadata_from_image(image_path: &PathBuf, key: &str) -> Option<String> {
    if !command_exists("exiftool") {
        println!("Couldn't find exiftool (needed for metadata extraction)");
        return None;
    }

    let output = match Command::new("exiftool")
        .args(&[
            "-s",
            "-s",
            "-s",
            &format!("-{}", key),
            &image_path.to_string_lossy(),
        ])
        .output()
    {
        Ok(output) => output,
        Err(_) => return None,
    };

    if !output.status.success() {
        eprintln!("Failed to get metadata from exiftool ({})", output.status);
        return None;
    }

    let result = String::from_utf8_lossy(&output.stdout);
    let result = result.trim();
    if result.is_empty() {
        eprintln!(
            "Metadata key '{}' not found in image: {}",
            key,
            image_path.display()
        );
        None
    } else {
        Some(result.to_string())
    }
}
