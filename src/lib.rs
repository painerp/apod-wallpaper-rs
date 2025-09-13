pub mod desktop;
pub mod utils;

#[cfg(feature = "cli")]
pub mod apod;
#[cfg(any(feature = "cli", feature = "gui"))]
pub mod config;
#[cfg(feature = "gui")]
pub mod gui;

#[cfg(any(feature = "cli"))]
pub use apod::ApodClient;
#[cfg(any(feature = "cli", feature = "gui"))]
pub use config::WallpaperConfig;
pub use desktop::WallpaperManager;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "cli")]
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Desktop environment error: {0}")]
    DesktopEnv(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("API error: {0}")]
    Api(String),

    #[cfg(feature = "gui")]
    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),
}

pub type Result<T> = std::result::Result<T, Error>;
