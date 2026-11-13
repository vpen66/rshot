use chrono::Local;
use image::RgbaImage;
use log::info;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Directory error: {0}")]
    Directory(String),

    #[error("Image encoding error: {0}")]
    ImageEncode(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    Png,
    Jpeg,
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
        }
    }
}

/// Storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub save_directory: PathBuf,
    pub format: OutputFormat,
    pub filename_prefix: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            save_directory: StorageService::default_pictures_dir(),
            format: OutputFormat::Png,
            filename_prefix: "rshot".to_string(),
        }
    }
}

/// Service for managing screenshot file persistence and directories.
pub struct StorageService;

impl StorageService {
    /// Resolves the default pictures directory: `~/Pictures/RustShot/`
    pub fn default_pictures_dir() -> PathBuf {
        if let Some(pictures) = dirs::picture_dir() {
            pictures.join("RustShot")
        } else if let Some(home) = dirs::home_dir() {
            home.join("Pictures").join("RustShot")
        } else {
            PathBuf::from("RustShot")
        }
    }

    /// Generates a timestamp-based filename: `rshot_YYYY-MM-DD_HHMMSS.ext`
    pub fn generate_timestamped_filename(prefix: &str, ext: &str) -> String {
        let timestamp = Local::now().format("%Y-%m-%d_%H%M%S");
        format!("{prefix}_{timestamp}.{ext}")
    }

    /// Saves an image to the specified target path.
    pub fn save_image(img: &RgbaImage, target_path: &Path) -> Result<()> {
        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| StorageError::Directory(format!("Failed to create parent dir: {e}")))?;
            }
        }

        info!("Saving screenshot to: {}", target_path.display());
        img.save(target_path)
            .map_err(|e| StorageError::ImageEncode(format!("Failed to encode/save image: {e}")))?;

        Ok(())
    }

    /// Saves an image into the default `~/Pictures/RustShot/` directory with timestamp name.
    pub fn save_to_default_dir(img: &RgbaImage, format: OutputFormat) -> Result<PathBuf> {
        let dir = Self::default_pictures_dir();
        let filename = Self::generate_timestamped_filename("rshot", format.extension());
        let target_path = dir.join(filename);

        Self::save_image(img, &target_path)?;
        Ok(target_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use std::fs;

    #[test]
    fn test_generate_filename() {
        let filename = StorageService::generate_timestamped_filename("test", "png");
        assert!(filename.starts_with("test_"));
        assert!(filename.ends_with(".png"));
    }

    #[test]
    fn test_save_image() {
        let temp_dir = std::env::temp_dir().join("rshot_test_save");
        let target_path = temp_dir.join("test_img.png");

        let mut img = RgbaImage::new(16, 16);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));

        let save_res = StorageService::save_image(&img, &target_path);
        assert!(save_res.is_ok(), "Saving image should succeed");
        assert!(target_path.exists(), "File should be created on disk");

        // Clean up
        let _ = fs::remove_file(&target_path);
        let _ = fs::remove_dir(&temp_dir);
    }
}
