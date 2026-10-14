use crate::utils::command_exists;
use crate::{Error, Result};
use chrono::{Local, NaiveDate, Timelike, Utc};
use rand::seq::IndexedRandom;
use reqwest::Client;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// APOD moved from apod.nasa.gov to science.nasa.gov (Sep 2026); api.nasa.gov/planetary/apod
// still scrapes the old site and now returns the NASA logo. Category 22766 = APOD posts.
const APOD_POSTS_URL: &str = "https://science.nasa.gov/wp-json/wp/v2/image-article?categories=22766";

#[derive(Debug, Deserialize)]
struct Rendered {
    rendered: String,
}

#[derive(Debug, Deserialize)]
struct ApodPost {
    title: Rendered,
    content: Rendered,
    #[serde(default)]
    featured_image: serde_json::Value,
}

fn html_to_text(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }

    let mut decoded = String::new();
    let mut rest = text.as_str();
    while let Some(i) = rest.find('&') {
        decoded.push_str(&rest[..i]);
        rest = &rest[i..];
        let entity = rest.find(';').and_then(|end| {
            let c = match &rest[1..end] {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                "nbsp" => ' ',
                "hellip" => '…',
                num => char::from_u32(num.strip_prefix('#')?.parse().ok()?)?,
            };
            Some((c, end + 1))
        });
        match entity {
            Some((c, len)) => {
                decoded.push(c);
                rest = &rest[len..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);

    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub struct ApodClient {
    client: Client,
}

impl ApodClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn get_image(
        &self,
        folder: &Path,
        random: bool,
        date_offset: Option<usize>,
    ) -> Result<Option<PathBuf>> {
        if !folder.exists() {
            fs::create_dir_all(folder)?;
        }

        let target_date = if random {
            None
        } else {
            let today = Utc::now().naive_utc().date();
            let mut offset = date_offset.unwrap_or(0) as i64;
            if Utc::now().hour() < 5 {
                offset += 1;
            }
            Some(today - chrono::Duration::days(offset))
        };

        if random {
            if let Some(image_path) = self.get_random_local_image(folder)? {
                return Ok(Some(image_path));
            }
        } else if let Some(date) = target_date {
            if let Some(image_path) = self.get_local_image_for_date(folder, date)? {
                return Ok(Some(image_path));
            }
        }

        if let Some(date) = target_date {
            self.download_single_image(folder, Some(date), random).await
        } else {
            self.download_single_image(folder, None, random).await
        }
    }

    pub async fn download_range(&self, folder: &Path, days: usize) -> Result<usize> {
        if !folder.exists() {
            fs::create_dir_all(folder)?;
        }

        let mut downloaded_count = 0;
        let today = Local::now().naive_local().date();
        let start = if Utc::now().hour() < 5 {
            today - chrono::Duration::days(1)
        } else {
            today
        };

        for day_offset in 0..days {
            let target_date = start - chrono::Duration::days(day_offset as i64);

            if self
                .get_local_image_for_date(folder, target_date)?
                .is_some()
            {
                println!("Image for {} already exists, skipping", target_date);
                continue;
            }

            println!("Downloading APOD for {}...", target_date.format("%Y-%m-%d"));

            match self
                .download_single_image(folder, Some(target_date), false)
                .await
            {
                Ok(Some(_)) => {
                    downloaded_count += 1;
                    println!(
                        "Successfully downloaded image for {}",
                        target_date.format("%Y-%m-%d")
                    );
                }
                Ok(None) => {
                    println!(
                        "No image available for {} (might be video content)",
                        target_date.format("%Y-%m-%d")
                    );
                }
                Err(e) => {
                    eprintln!(
                        "Failed to download image for {}: {}",
                        target_date.format("%Y-%m-%d"),
                        e
                    );
                }
            }
        }

        Ok(downloaded_count)
    }

    async fn download_single_image(
        &self,
        folder: &Path,
        target_date: Option<NaiveDate>,
        random: bool,
    ) -> Result<Option<PathBuf>> {
        let date = if random {
            let first = NaiveDate::from_ymd_opt(1995, 6, 16).unwrap();
            let days = (Utc::now().naive_utc().date() - first).num_days();
            first + chrono::Duration::days(rand::random_range(0..=days))
        } else {
            target_date.unwrap_or_else(|| Utc::now().naive_utc().date())
        };
        let date = date.format("%Y-%m-%d").to_string();

        let url = format!(
            "{}&after={}T00:00:00&before={}T23:59:59",
            APOD_POSTS_URL, date, date
        );
        let posts: Vec<ApodPost> = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let Some(apod) = posts.first() else {
            return Ok(None);
        };
        let Some(image_url) = apod.featured_image.get("file").and_then(|f| f.as_str()) else {
            return Ok(None);
        };

        // "APOD: 2026 October 14 – Mars and the Beehive" -> "Mars and the Beehive"
        let title = html_to_text(&apod.title.rendered);
        let title = title.split_once(" – ").map_or(title.as_str(), |(_, t)| t);
        let explanation = apod
            .content
            .rendered
            .split_once("Explanation:")
            .map_or("", |(_, rest)| rest);
        let explanation = explanation.split("<br").next().unwrap_or_default();
        let explanation = html_to_text(explanation.split("</p>").next().unwrap_or_default());

        let image_ext =
            if let Some(ext) = Path::new(image_url).extension().and_then(|e| e.to_str()) {
                ext
            } else {
                "jpg"
            }
            .to_lowercase();

        if image_ext != "jpg" && image_ext != "jpeg" && image_ext != "png" {
            return Err(Error::Api(format!(
                "Unsupported image format: {}",
                image_ext
            )));
        }

        let file_name = format!("{}.{}", date, image_ext);
        let file_path = folder.join(file_name);

        // A cached non-image (e.g. an HTML error page) would block re-downloading that date forever.
        let image_response = self.client.get(image_url).send().await?.error_for_status()?;
        let is_image = image_response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("image/"));
        if !is_image {
            return Err(Error::Api(format!("Not an image: {}", image_url)));
        }
        let image_bytes = image_response.bytes().await?;

        fs::write(&file_path, image_bytes)?;

        if let Err(e) = self.add_exif_metadata(&file_path, &title, &explanation) {
            eprintln!("Warning: Failed to add EXIF metadata: {}", e);
        }

        Ok(Some(file_path))
    }

    pub async fn download_specific_date(&self, folder: &Path, date_str: &str) -> Result<usize> {
        use chrono::NaiveDate;

        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|_| {
            Error::Api(format!("Invalid date format: {}. Use YYYY-MM-DD", date_str))
        })?;

        if !folder.exists() {
            fs::create_dir_all(folder)?;
        }

        if self.get_local_image_for_date(folder, date)?.is_some() {
            println!("Image for {} already exists, skipping", date_str);
            return Ok(0);
        }

        println!("Downloading APOD for {}...", date_str);

        match self.download_single_image(folder, Some(date), false).await {
            Ok(Some(_)) => {
                println!("Successfully downloaded image for {}", date_str);
                Ok(1)
            }
            Ok(None) => {
                println!(
                    "No image available for {} (might be video content)",
                    date_str
                );
                Ok(0)
            }
            Err(e) => {
                eprintln!("Failed to download image for {}: {}", date_str, e);
                Err(e)
            }
        }
    }

    pub async fn download_date_range(
        &self,
        folder: &Path,
        start_str: &str,
        end_str: &str,
    ) -> Result<usize> {
        use chrono::NaiveDate;

        let start_date = NaiveDate::parse_from_str(start_str, "%Y-%m-%d").map_err(|_| {
            Error::Api(format!(
                "Invalid start date format: {}. Use YYYY-MM-DD",
                start_str
            ))
        })?;

        let end_date = NaiveDate::parse_from_str(end_str, "%Y-%m-%d").map_err(|_| {
            Error::Api(format!(
                "Invalid end date format: {}. Use YYYY-MM-DD",
                end_str
            ))
        })?;

        if start_date > end_date {
            return Err(Error::Api(
                "Start date must be before or equal to end date".to_string(),
            ));
        }

        if !folder.exists() {
            fs::create_dir_all(folder)?;
        }

        let mut downloaded_count = 0;
        let mut current_date = start_date;

        while current_date <= end_date {
            if self
                .get_local_image_for_date(folder, current_date)?
                .is_some()
            {
                println!(
                    "Image for {} already exists, skipping",
                    current_date.format("%Y-%m-%d")
                );
                current_date = current_date + chrono::Duration::days(1);
                continue;
            }

            println!(
                "Downloading APOD for {}...",
                current_date.format("%Y-%m-%d")
            );

            match self
                .download_single_image(folder, Some(current_date), false)
                .await
            {
                Ok(Some(_)) => {
                    downloaded_count += 1;
                    println!(
                        "Successfully downloaded image for {}",
                        current_date.format("%Y-%m-%d")
                    );
                }
                Ok(None) => {
                    println!(
                        "No image available for {} (might be video content)",
                        current_date.format("%Y-%m-%d")
                    );
                }
                Err(e) => {
                    eprintln!(
                        "Failed to download image for {}: {}",
                        current_date.format("%Y-%m-%d"),
                        e
                    );
                }
            }

            current_date = current_date + chrono::Duration::days(1);
        }

        Ok(downloaded_count)
    }

    fn add_exif_metadata(&self, file_path: &Path, title: &str, explanation: &str) -> Result<()> {
        if !command_exists("exiftool") {
            eprintln!(
                "exiftool not found. EXIF metadata not added. Install exiftool for full metadata support."
            );
            return Ok(());
        }

        let file_path_str = file_path.to_string_lossy();
        let title_arg = format!("-Title={}", title);
        let description_arg = format!("-Description={}", explanation);

        let result = Command::new("exiftool")
            .arg("-overwrite_original")
            .arg("-ifd0:all=")
            .arg(&title_arg)
            .arg(&description_arg)
            .arg(file_path_str.as_ref())
            .output();

        match result {
            Ok(output) if output.status.success() => Ok(()),
            Ok(_) => Err(Error::DesktopEnv(
                "Failed to add EXIF metadata with exiftool".to_string(),
            )),
            Err(_) => {
                eprintln!(
                    "exiftool not found. EXIF metadata not added. Install exiftool for full metadata support."
                );
                Ok(())
            }
        }
    }

    fn get_local_image_for_date(&self, folder: &Path, date: NaiveDate) -> Result<Option<PathBuf>> {
        let date_ymd = date.format("%Y-%m-%d").to_string();

        if let Ok(entries) = fs::read_dir(folder) {
            for entry in entries.filter_map(|e| e.ok()) {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with(&date_ymd) {
                    return Ok(Some(entry.path()));
                }
            }
        }

        Ok(None)
    }

    fn get_random_local_image(&self, folder: &Path) -> Result<Option<PathBuf>> {
        let mut images = Vec::new();

        if let Ok(entries) = fs::read_dir(folder) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "jpg") {
                    images.push(path);
                }
            }
        }

        if !images.is_empty() {
            let mut rng = rand::rng();
            return Ok(images.choose(&mut rng).cloned());
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::html_to_text;

    #[test]
    fn strips_tags_and_decodes_entities() {
        assert_eq!(
            html_to_text("</strong>In <a href=\"x\">today&#8217;s</a>\n\t image &amp; &lt;M44&gt;&nbsp;R&D &bogus;"),
            "In today’s image & <M44> R&D &bogus;"
        );
    }
}
