use ksni::{Icon, ToolTip, TrayMethods};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct ApodWallpaperTray {
    icon_pixmap: Arc<Vec<Icon>>,
    cached_tooltip: Arc<Mutex<Option<CachedTooltip>>>,
}

#[derive(Debug, Clone)]
struct CachedTooltip {
    tooltip: ToolTip,
    wallpaper_path: PathBuf,
    cached_at: Instant,
}

impl ApodWallpaperTray {
    fn new() -> Self {
        let nasa_svg_path = apod_wallpaper::utils::get_nasa_svg_path().unwrap();
        let icon_pixmap = render_svg_to_ksni_icon(&nasa_svg_path, true);
        ApodWallpaperTray {
            icon_pixmap: Arc::new(icon_pixmap),
            cached_tooltip: Arc::new(Mutex::new(None)),
        }
    }

    fn get_cached_tooltip(&self) -> ToolTip {
        let manager = apod_wallpaper::desktop::get_wallpaper_manager().unwrap();
        let current_wallpaper = manager.get_wallpaper(None).unwrap().unwrap();

        let mut cache = self.cached_tooltip.lock().unwrap();

        // Check if we need to refresh the cache
        let should_refresh = match &*cache {
            None => true,
            Some(cached) => {
                // Refresh if wallpaper changed or cache is older than 5 minutes
                cached.wallpaper_path != current_wallpaper
                    || cached.cached_at.elapsed() > Duration::from_secs(300)
            }
        };

        if should_refresh {
            let title = apod_wallpaper::utils::get_metadata_from_image(&current_wallpaper, "Title")
                .unwrap_or_else(|| "Unknown Title".to_string());
            let description =
                apod_wallpaper::utils::get_metadata_from_image(&current_wallpaper, "Description")
                    .unwrap_or_else(|| "Unknown Description".to_string());

            let tooltip = ToolTip {
                title,
                description,
                icon_name: "".to_string(),
                icon_pixmap: vec![],
            };

            *cache = Some(CachedTooltip {
                tooltip: tooltip.clone(),
                wallpaper_path: current_wallpaper,
                cached_at: Instant::now(),
            });

            tooltip
        } else {
            cache.as_ref().unwrap().tooltip.clone()
        }
    }
}

fn render_svg_to_ksni_icon(svg_path: &PathBuf, monochrome: bool) -> Vec<Icon> {
    use resvg::usvg;
    use std::fs;

    // Read SVG file
    let svg_data = match fs::read_to_string(svg_path) {
        Ok(data) => data,
        Err(_) => return vec![],
    };

    // Parse SVG
    let options = usvg::Options::default();
    let tree = match usvg::Tree::from_str(&svg_data, &options) {
        Ok(tree) => tree,
        Err(_) => return vec![],
    };

    // Create 32x32 pixmap
    let size = 32;
    let mut pixmap = match resvg::tiny_skia::Pixmap::new(size, size) {
        Some(pixmap) => pixmap,
        None => return vec![],
    };

    // Calculate transform to fit SVG to 32x32
    let transform = resvg::tiny_skia::Transform::from_scale(
        size as f32 / tree.size().width(),
        size as f32 / tree.size().height(),
    );

    // Render SVG to pixmap
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert RGBA to ARGB format with optional monochrome conversion
    let mut argb_data = Vec::with_capacity(pixmap.data().len());
    for chunk in pixmap.data().chunks_exact(4) {
        let (r, g, b, a) = if monochrome {
            // Convert to grayscale using luminance formula
            let gray =
                (0.299 * chunk[0] as f32 + 0.587 * chunk[1] as f32 + 0.114 * chunk[2] as f32) as u8;
            (gray, gray, gray, chunk[3])
        } else {
            (chunk[0], chunk[1], chunk[2], chunk[3])
        };
        // Convert RGBA to ARGB
        argb_data.extend_from_slice(&[a, r, g, b]);
    }

    vec![Icon {
        width: size as i32,
        height: size as i32,
        data: argb_data,
    }]
}

impl ksni::Tray for ApodWallpaperTray {
    fn id(&self) -> String {
        "apod-wallpaper-applet".to_string()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        if !apod_wallpaper::utils::command_exists("apod-wallpaper-switcher") {
            eprintln!("Error: 'apod-wallpaper-switcher' command not found in PATH.");
            return;
        }
        println!("Activating wallpaper switcher...");
        let output = std::process::Command::new("apod-wallpaper-switcher")
            .output()
            .unwrap();
        if !output.status.success() {
            eprintln!(
                "Failed to switch wallpaper: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    fn title(&self) -> String {
        "Apod Wallpaper Applet".to_string()
    }

    fn icon_name(&self) -> String {
        "".to_string()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        (*self.icon_pixmap).clone()
    }

    fn tool_tip(&self) -> ToolTip {
        self.get_cached_tooltip()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Refresh".to_string(),
                activate: Box::new(|_this: &mut Self| {
                    if !apod_wallpaper::utils::command_exists("apod-wallpaper") {
                        eprintln!("Error: 'apod-wallpaper' command not found in PATH.");
                        return;
                    }
                    println!("Refreshing wallpaper...");
                    let output = std::process::Command::new("apod-wallpaper")
                        .arg("-u")
                        .output()
                        .unwrap();
                    if !output.status.success() {
                        eprintln!(
                            "Failed to refresh wallpaper: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".to_string(),
                icon_name: "application-exit".to_string(),
                activate: Box::new(|_| {
                    println!("Quit selected, shutting down gracefully...");
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Initializing tray...");

    let tray = ApodWallpaperTray::new();

    println!("Creating tray service...");
    tray.spawn().await.unwrap();

    std::future::pending::<()>().await;

    println!("Shutting down tray service...");
    Ok(())
}
