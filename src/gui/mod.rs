pub mod image_grid;

use std::path::PathBuf;

pub fn run_switcher(save_folder: PathBuf) -> crate::Result<()> {
    use iced::Result as IcedResult;

    let result: IcedResult = image_grid::run_wallpaper_switcher(save_folder);

    result.map_err(|e| crate::Error::DesktopEnv(format!("GUI error: {e}")))?;
    Ok(())
}
