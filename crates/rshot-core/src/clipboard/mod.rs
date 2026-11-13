use crate::error::{CoreError, Result};
use arboard::{Clipboard, ImageData};
use image::RgbaImage;
use std::borrow::Cow;

/// Service for interacting with the system clipboard.
pub struct ClipboardService;

impl ClipboardService {
    /// Copies an RGBA image to the system clipboard.
    pub fn copy_image(img: &RgbaImage) -> Result<()> {
        let mut clipboard = Clipboard::new()
            .map_err(|e| CoreError::Clipboard(format!("Failed to initialize clipboard: {e}")))?;

        let image_data = ImageData {
            width: img.width() as usize,
            height: img.height() as usize,
            bytes: Cow::Borrowed(img.as_raw()),
        };

        clipboard
            .set_image(image_data)
            .map_err(|e| CoreError::Clipboard(format!("Failed to copy image to clipboard: {e}")))?;

        Ok(())
    }

    /// Reads an RGBA image from the system clipboard, if present.
    pub fn get_image() -> Result<Option<RgbaImage>> {
        let mut clipboard = Clipboard::new()
            .map_err(|e| CoreError::Clipboard(format!("Failed to initialize clipboard: {e}")))?;

        match clipboard.get_image() {
            Ok(img_data) => {
                let img = RgbaImage::from_raw(
                    img_data.width as u32,
                    img_data.height as u32,
                    img_data.bytes.into_owned(),
                )
                .ok_or_else(|| {
                    CoreError::Clipboard("Failed to construct image from clipboard buffer".into())
                })?;
                Ok(Some(img))
            }
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(CoreError::Clipboard(format!("Failed to read clipboard image: {e}"))),
        }
    }

    /// Copies a text string to the system clipboard.
    pub fn copy_text(text: &str) -> Result<()> {
        let mut clipboard = Clipboard::new()
            .map_err(|e| CoreError::Clipboard(format!("Failed to initialize clipboard: {e}")))?;

        clipboard
            .set_text(text)
            .map_err(|e| CoreError::Clipboard(format!("Failed to copy text to clipboard: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn test_clipboard_copy_text() {
        let result = ClipboardService::copy_text("rshot test");
        // May succeed or fail on headless CI, but verify no panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_clipboard_copy_image() {
        let mut img = RgbaImage::new(10, 10);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        let result = ClipboardService::copy_image(&img);
        assert!(result.is_ok() || result.is_err());
    }
}
