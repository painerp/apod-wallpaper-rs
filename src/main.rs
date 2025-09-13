use clap::{Parser, Subcommand};
use std::path::PathBuf;

use apod_wallpaper::{
    desktop::get_wallpaper_manager, utils::{generate_pywal_colors, generate_wallust_colors, get_nasa_svg_path},
    ApodClient,
    WallpaperConfig,
};

#[derive(Parser)]
#[command(name = "apod-wallpaper")]
#[command(
    version,
    about = "A NASA APOD wallpaper manager. Fetches the latest APOD image and sets it as your desktop wallpaper."
)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(
        short,
        long,
        help = "Folder to save wallpapers to (will be saved in config if used once)"
    )]
    folder: Option<PathBuf>,
    #[arg(
        short,
        long,
        help = "Will use options from config file (can be overridden)"
    )]
    use_config: bool,
    #[arg(
        short,
        long,
        help = "Enable multi-monitor support (different wallpaper for each monitor)"
    )]
    multi_monitor: bool,
    #[arg(
        short,
        long,
        help = "Fetch a random wallpaper from all available in the Folder"
    )]
    random: bool,
    #[arg(
        long,
        help = "Generate pywal colors from the wallpaper (requires pywal to be installed)"
    )]
    pywal: bool,
    #[arg(
        long,
        help = "Generate wallust colors from the wallpaper (requires wallust to be installed)"
    )]
    wallust: bool,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Download APOD images")]
    Download {
        #[arg(
            short,
            long,
            help = "Folder to save wallpapers to (will be saved in config if used once)"
        )]
        folder: Option<PathBuf>,
        #[arg(help = "Number of days to download (defaults to 7)", conflicts_with_all = ["date", "start_date"])]
        days: Option<usize>,
        #[arg(
                    long,
                    help = "Download image for specific date (YYYY-MM-DD)",
                    conflicts_with_all = ["days", "start_date"]
                )]
        date: Option<String>,
        #[arg(
            long,
            help = "Start date for date range (YYYY-MM-DD), requires --end-date",
            requires = "end_date"
        )]
        start_date: Option<String>,
        #[arg(
            long,
            help = "End date for date range (YYYY-MM-DD), requires --start-date",
            requires = "start_date"
        )]
        end_date: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut config = WallpaperConfig::load_or_default()?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match args.command {
            Some(Commands::Download {
                days,
                folder,
                date,
                start_date,
                end_date,
            }) => {
                let save_folder =
                    folder.unwrap_or_else(|| args.folder.unwrap_or(config.save_folder.clone()));

                let client = ApodClient::new();
                let downloaded_count = if let Some(date_str) = date {
                    client
                        .download_specific_date(&save_folder, &date_str)
                        .await?
                } else if let (Some(start), Some(end)) = (start_date, end_date) {
                    client
                        .download_date_range(&save_folder, &start, &end)
                        .await?
                } else {
                    let download_days = days.unwrap_or(7);
                    client.download_range(&save_folder, download_days).await?
                };

                println!(
                    "Downloaded {} new APOD images to {}",
                    downloaded_count,
                    save_folder.display()
                );

                if !args.use_config {
                    config.save_folder = save_folder;
                    config.save()?;
                }
                Ok::<(), anyhow::Error>(())
            }
            None => {
                let save_folder = args.folder.unwrap_or(config.save_folder);
                let client = ApodClient::new();
                let manager = get_wallpaper_manager()?;

                let screens = if args.multi_monitor || (args.use_config && config.multi_monitor) {
                    manager.get_screens()
                } else {
                    vec!["default".to_string()]
                };

                let mut image_paths = Vec::new();
                let max_offset = 365;
                let mut offset = 0;

                while image_paths.len() < screens.len() && offset < max_offset {
                    if let Some(image_path) = client
                        .get_image(
                            &save_folder,
                            args.random || (args.use_config && config.random),
                            Some(offset),
                        )
                        .await?
                    {
                        image_paths.push(image_path);
                    }
                    offset += 1;
                }

                for (i, screen) in screens.iter().enumerate() {
                    if i < image_paths.len() {
                        manager.set_wallpaper(&image_paths[i], Some(screen))?;
                    }
                }

                manager.notify(
                    "APOD Wallpaper",
                    "Multiple wallpapers updated successfully",
                    Some(&get_nasa_svg_path().unwrap()),
                )?;

                if !image_paths.is_empty()
                    && (args.pywal
                        || args.wallust
                        || (args.use_config && (config.pywal || config.wallust)))
                {
                    if args.pywal || (args.use_config && config.pywal) {
                        generate_pywal_colors(&image_paths[0])?;
                    }
                    if args.wallust || (args.use_config && config.wallust) {
                        generate_wallust_colors(&image_paths[0])?;
                    }
                }

                if !args.use_config {
                    config.save_folder = save_folder;
                    config.multi_monitor = args.multi_monitor;
                    config.random = args.random;
                    config.pywal = args.pywal;
                    config.wallust = args.wallust;
                    config.save()?;
                }

                Ok::<(), anyhow::Error>(())
            }
        }
    })?;

    Ok(())
}
