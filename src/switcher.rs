use apod_wallpaper::{gui, WallpaperConfig};
use clap::Parser;
use std::fs::create_dir;

#[derive(Parser)]
#[command(name = "apod-wallpaper-switcher")]
#[command(
    version,
    about = "APOD Wallpaper Switcher, a GUI tool to switch between downloaded APOD wallpapers."
)]
struct Args {
    #[arg(
        short,
        long,
        help = "Folder to save wallpapers to (will be saved in config if used once)"
    )]
    folder: Option<std::path::PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = WallpaperConfig::load_or_default()?;
    let save_folder = args.folder.unwrap_or(config.save_folder);

    if !save_folder.exists() {
        create_dir(&save_folder)?;
    }

    gui::run_switcher(save_folder)?;
    Ok(())
}
