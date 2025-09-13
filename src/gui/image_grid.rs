use crate::desktop::get_wallpaper_manager;
use crate::utils::{
    generate_pywal_colors, generate_wallust_colors, get_cache_dir, get_image_files,
    get_nasa_svg_path,
};
use iced::{
    keyboard::{key::Named, Key}, widget::{button, column, container, image, mouse_area, scrollable, stack, text}, Background, Border, Color, Element, Length, Padding, Pixels, Size,
    Task,
    Theme,
};
use std::cell::Cell;
use std::path::PathBuf;

macro_rules! themes {
    ($($variant:ident),*) => {
        fn get_available_themes() -> Vec<String> {
            vec![$(stringify!($variant).to_string()),*]
        }

        fn string_to_theme(theme_str: &str) -> Theme {
            match theme_str {
                $(stringify!($variant) => Theme::$variant,)*
                _ => Theme::Dark,
            }
        }
    };
}

themes!(
    Dark,
    Light,
    Dracula,
    Nord,
    SolarizedLight,
    SolarizedDark,
    GruvboxLight,
    GruvboxDark,
    CatppuccinLatte,
    CatppuccinFrappe,
    CatppuccinMacchiato,
    CatppuccinMocha,
    TokyoNight,
    TokyoNightStorm,
    TokyoNightLight,
    KanagawaWave,
    KanagawaDragon,
    KanagawaLotus,
    Moonfly,
    Nightfly,
    Oxocarbon
);

#[derive(Debug, Clone)]
pub enum Message {
    ImageSelected(PathBuf),
    ImageHovered(usize),
    ImageUnhovered,
    LoadImages,
    ImagesLoaded(Vec<(PathBuf, Option<PathBuf>)>),
    ThumbnailReady(PathBuf, Option<PathBuf>),
    ThemeChanged(String),
    ToggleThemeSelector,
    KeyPressed(Key),
}

pub struct WallpaperSwitcher {
    images: Vec<(PathBuf, Option<PathBuf>)>,
    images_per_row: Cell<usize>,
    save_folder: PathBuf,
    hovered_image: Option<usize>,
    config: crate::config::WallpaperConfig,
    available_themes: Vec<String>,
    show_theme_selector: bool,
    show_top_bar: bool,
}

impl WallpaperSwitcher {
    pub fn new(save_folder: PathBuf) -> (Self, Task<Message>) {
        let config = crate::config::WallpaperConfig::load_or_default().unwrap_or_default();

        let app = Self {
            images: Vec::new(),
            images_per_row: Cell::new(1),
            save_folder: save_folder.clone(),
            hovered_image: None,
            config,
            available_themes: get_available_themes(),
            show_theme_selector: false,
            show_top_bar: false,
        };

        let task = Self::load_folder_task(save_folder);
        (app, task)
    }

    fn load_folder_task(folder: PathBuf) -> Task<Message> {
        Task::future(async move {
            let result = match get_image_files(&folder).await {
                Ok(images) => images.into_iter().rev().map(|path| (path, None)).collect(),
                Err(e) => {
                    println!("Error loading images: {}", e);
                    Vec::new()
                }
            };

            result
        })
        .map(Message::ImagesLoaded)
    }

    fn generate_single_thumbnail(image_path: PathBuf) -> Task<Message> {
        Task::future(async move {
            let cache_dir = get_cache_dir().unwrap().join(PathBuf::from("thumbnails"));

            let hash = {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                image_path.hash(&mut hasher);
                hasher.finish()
            };

            let original_name = image_path.file_stem().unwrap_or_default().to_string_lossy();
            let extension = image_path.extension().unwrap_or_default().to_string_lossy();
            let thumbnail_name = format!("thumb_{}_{}.{}", original_name, hash, extension);
            let thumbnail_path = cache_dir.join(thumbnail_name);

            if thumbnail_path.exists() {
                return (image_path, Some(thumbnail_path));
            }

            let img_path = image_path.clone();
            let thumb_path = thumbnail_path.clone();

            match tokio::task::spawn_blocking(move || {
                match crate::utils::generate_thumbnail(&img_path, &thumb_path, 400) {
                    Ok(_) => Some(thumb_path),
                    Err(e) => {
                        println!(
                            "Failed to generate thumbnail for {}: {}",
                            img_path.display(),
                            e
                        );
                        None
                    }
                }
            })
            .await
            {
                Ok(result) => (image_path, result),
                Err(e) => {
                    println!("Thumbnail generation task failed: {}", e);
                    (image_path, None)
                }
            }
        })
        .map(|(original, thumbnail)| Message::ThumbnailReady(original, thumbnail))
    }

    fn do_update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LoadImages => Self::load_folder_task(self.save_folder.clone()),
            Message::ImagesLoaded(images) => {
                self.images = images;

                let thumbnail_tasks: Vec<Task<Message>> = self
                    .images
                    .iter()
                    .map(|(path, thumbnail)| {
                        if thumbnail.is_none() {
                            Self::generate_single_thumbnail(path.clone())
                        } else {
                            Task::none()
                        }
                    })
                    .collect();

                Task::batch(thumbnail_tasks)
            }
            Message::ThumbnailReady(original_path, thumbnail_path) => {
                for (path, thumbnail) in &mut self.images {
                    if *path == original_path {
                        *thumbnail = thumbnail_path;
                        break;
                    }
                }
                Task::none()
            }
            Message::ImageSelected(path) => {
                println!("Selected wallpaper: {}", path.display());
                let manager = get_wallpaper_manager().unwrap();
                manager.set_wallpaper(&path, None).unwrap();

                manager
                    .notify(
                        "APOD Wallpaper",
                        "Wallpapers updated successfully",
                        Some(&get_nasa_svg_path().unwrap()),
                    )
                    .unwrap();

                if self.config.pywal || self.config.wallust {
                    if self.config.pywal {
                        generate_pywal_colors(&path).unwrap();
                    }
                    if self.config.wallust {
                        generate_wallust_colors(&path).unwrap();
                    }
                }

                iced::exit()
            }
            Message::ImageHovered(index) => {
                self.hovered_image = Some(index);
                Task::none()
            }
            Message::ImageUnhovered => {
                self.hovered_image = None;
                Task::none()
            }
            Message::ThemeChanged(theme_name) => {
                self.config.theme = theme_name;
                let _ = self.config.save();
                self.show_theme_selector = false;
                Task::none()
            }
            Message::ToggleThemeSelector => {
                self.show_theme_selector = !self.show_theme_selector;
                Task::none()
            }
            Message::KeyPressed(key) => {
                if let Key::Named(Named::Alt) = key {
                    self.show_top_bar = !self.show_top_bar;
                    if !self.show_top_bar {
                        self.show_theme_selector = false;
                    }
                }

                let total = self.images.len();
                if total == 0 {
                    return Task::none();
                }

                let mut idx = self.hovered_image.unwrap_or(0);
                let images_per_row = self.images_per_row.get();

                match key {
                    Key::Named(Named::ArrowRight) => {
                        idx = (idx + 1) % total;
                        self.hovered_image = Some(idx);
                        return self.do_update(Message::ImageHovered(idx));
                    }
                    Key::Named(Named::ArrowLeft) => {
                        idx = if idx == 0 { total - 1 } else { idx - 1 };
                        self.hovered_image = Some(idx);
                        return self.do_update(Message::ImageHovered(idx));
                    }
                    Key::Named(Named::ArrowDown) => {
                        idx = if idx + images_per_row >= total {
                            total - 1
                        } else {
                            idx + images_per_row
                        };
                        self.hovered_image = Some(idx);
                        return self.do_update(Message::ImageHovered(idx));
                    }
                    Key::Named(Named::ArrowUp) => {
                        idx = if idx < images_per_row {
                            0
                        } else {
                            idx - images_per_row
                        };
                        self.hovered_image = Some(idx);
                        return self.do_update(Message::ImageHovered(idx));
                    }
                    Key::Named(Named::Enter) => {
                        if let Some(idx) = self.hovered_image {
                            if let Some((path, _)) = self.images.get(idx) {
                                return self.do_update(Message::ImageSelected(path.clone()));
                            }
                        }
                    }
                    _ => {}
                }
                Task::none()
            }
        }
    }

    fn create_responsive_view(&self, actual_width: usize) -> Element<'_, Message> {
        if self.images.is_empty() {
            return container(text("Loading images..."))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        }

        let image_width = 200;
        let image_height = 150;
        let spacing = 10;
        let padding = 10;

        let images_per_row = ((actual_width - padding) / (image_width + spacing)).max(1);
        self.images_per_row.set(images_per_row);

        let mut rows = Vec::new();

        for chunk in self.images.chunks(images_per_row) {
            let mut row_elements = Vec::new();

            for (i, (original_path, thumbnail_path)) in chunk.iter().enumerate() {
                let global_index = rows.len() * images_per_row + i;

                let element = if let Some(thumbnail_path) = thumbnail_path {
                    let img = image(thumbnail_path.clone())
                        .width((image_width - 6) as u16)
                        .height((image_height - 6) as u16)
                        .content_fit(iced::ContentFit::Cover);

                    container(img)
                        .width(image_width as u16)
                        .height(image_height as u16)
                        .padding(3)
                        .style({
                            let hovered_image = self.hovered_image;
                            move |_theme| container::Style {
                                border: Border {
                                    width: if hovered_image == Some(global_index) {
                                        3.0
                                    } else {
                                        1.0
                                    },
                                    color: if hovered_image == Some(global_index) {
                                        self.theme().palette().primary
                                    } else {
                                        Color::from_rgb(0.5, 0.5, 0.5)
                                    },
                                    radius: 5.0.into(),
                                },
                                background: Some(Background::Color(Color::from_rgba(
                                    0.3, 0.3, 0.3, 0.5,
                                ))),
                                ..Default::default()
                            }
                        })
                } else {
                    container(
                        container(text("Loading..."))
                            .center_x(Length::Fill)
                            .center_y(Length::Fill)
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .width(image_width as u16)
                    .height(image_height as u16)
                    .style({
                        let hovered_image = self.hovered_image;
                        move |_theme| container::Style {
                            background: Some(Background::Color(Color::from_rgba(
                                0.3, 0.3, 0.3, 0.5,
                            ))),
                            border: Border {
                                width: if hovered_image == Some(global_index) {
                                    3.0
                                } else {
                                    1.0
                                },
                                color: if hovered_image == Some(global_index) {
                                    self.theme().palette().primary
                                } else {
                                    Color::from_rgb(0.5, 0.5, 0.5)
                                },
                                radius: 5.0.into(),
                            },
                            ..Default::default()
                        }
                    })
                };

                let hoverable = mouse_area(element)
                    .on_enter(Message::ImageHovered(global_index))
                    .on_exit(Message::ImageUnhovered)
                    .on_press(Message::ImageSelected(original_path.clone()));

                row_elements.push(hoverable.into());
            }

            let row = iced::widget::row(row_elements).spacing(Pixels(spacing as f32));
            rows.push(row.into());
        }

        let grid = iced::widget::column(rows)
            .spacing(Pixels(spacing as f32))
            .padding(20)
            .width(Length::Shrink);

        let scrollable_content = scrollable(grid).width(Length::Shrink).height(Length::Fill);

        container(scrollable_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn theme(&self) -> Theme {
        string_to_theme(&self.config.theme)
    }
}

pub fn run_wallpaper_switcher(save_folder: PathBuf) -> iced::Result {
    iced::application("APOD Wallpaper Switcher", update, view)
        .theme(|app: &WallpaperSwitcher| app.theme())
        .subscription(subscription)
        .window_size(Size {
            width: 870.0,
            height: 800.0,
        })
        .run_with(|| WallpaperSwitcher::new(save_folder))
}

fn subscription(_app: &WallpaperSwitcher) -> iced::Subscription<Message> {
    iced::keyboard::on_key_press(|key, _modifiers| Some(Message::KeyPressed(key)))
}

fn update(app: &mut WallpaperSwitcher, message: Message) -> Task<Message> {
    app.do_update(message)
}

fn view(app: &WallpaperSwitcher) -> Element<'_, Message> {
    let main_content =
        iced::widget::responsive(move |size| app.create_responsive_view(size.width as usize));

    if app.show_theme_selector {
        let theme_buttons: Vec<Element<Message>> = app
            .available_themes
            .iter()
            .map(|theme| {
                button(text(theme))
                    .on_press(Message::ThemeChanged(theme.clone()))
                    .width(Length::Fill)
                    .into()
            })
            .collect();

        let theme_selector = container(
            column([
                text("Select Theme").size(20).into(),
                column(theme_buttons).spacing(5).into(),
                button("Cancel")
                    .on_press(Message::ToggleThemeSelector)
                    .into(),
            ])
            .spacing(10)
            .padding(20),
        )
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(theme.palette().background)),
            border: Border::default().width(2).color(theme.palette().primary),
            ..Default::default()
        })
        .center_x(Length::Fill)
        .center_y(Length::Fill);

        stack([main_content.into(), theme_selector.into()]).into()
    } else {
        let mut content = vec![main_content.into()];

        if app.show_top_bar {
            let theme_button = button("Theme").on_press(Message::ToggleThemeSelector);
            content.insert(
                0,
                container(theme_button)
                    .padding(Padding {
                        top: 10.0,
                        right: 0.0,
                        bottom: 0.0,
                        left: 10.0,
                    })
                    .into(),
            );
        }

        column(content).into()
    }
}
