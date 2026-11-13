use crate::error::{CoreError, Result};
use image::RgbaImage;
use log::{debug, info};
use xcap::{Monitor, Window};

/// Information about a connected display.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub is_primary: bool,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

/// Information about an on-screen window (used for window boundary snapping).
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: u32,
    pub title: String,
    pub app_name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_minimized: bool,
}

/// Screenshot capture service.
pub struct ScreenCapture;

impl ScreenCapture {
    /// Lists all available monitors.
    pub fn get_monitors() -> Result<Vec<MonitorInfo>> {
        let monitors = Monitor::all()
            .map_err(|e| CoreError::Capture(format!("Failed to query monitors: {e}")))?;

        let list = monitors
            .into_iter()
            .map(|m| MonitorInfo {
                id: m.id().unwrap_or(0),
                name: m.name().unwrap_or_else(|_| "Unknown".into()),
                is_primary: m.is_primary().unwrap_or(false),
                x: m.x().unwrap_or(0),
                y: m.y().unwrap_or(0),
                width: m.width().unwrap_or(0),
                height: m.height().unwrap_or(0),
                scale_factor: m.scale_factor().unwrap_or(1.0),
            })
            .collect();

        Ok(list)
    }

    /// Captures the primary display.
    pub fn capture_primary_monitor() -> Result<(MonitorInfo, RgbaImage)> {
        let monitors = Monitor::all()
            .map_err(|e| CoreError::Capture(format!("Failed to query monitors: {e}")))?;

        if monitors.is_empty() {
            return Err(CoreError::MonitorNotFound("No monitors detected".into()));
        }

        let primary_monitor = monitors
            .iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .unwrap_or(&monitors[0]);

        let info = MonitorInfo {
            id: primary_monitor.id().unwrap_or(0),
            name: primary_monitor.name().unwrap_or_else(|_| "Unknown".into()),
            is_primary: primary_monitor.is_primary().unwrap_or(false),
            x: primary_monitor.x().unwrap_or(0),
            y: primary_monitor.y().unwrap_or(0),
            width: primary_monitor.width().unwrap_or(0),
            height: primary_monitor.height().unwrap_or(0),
            scale_factor: primary_monitor.scale_factor().unwrap_or(1.0),
        };

        info!("Capturing primary monitor: {} ({}x{})", info.name, info.width, info.height);
        let raw_img = primary_monitor
            .capture_image()
            .map_err(|e| CoreError::Capture(format!("Failed to capture monitor: {e}")))?;

        // Raw image from xcap is xcap::image::RgbaImage, identical to image::RgbaImage
        let (w, h) = raw_img.dimensions();
        let rgba_img = RgbaImage::from_raw(w, h, raw_img.into_raw())
            .ok_or_else(|| CoreError::Image("Failed to construct RgbaImage buffer".into()))?;

        Ok((info, rgba_img))
    }

    /// Captures a specific monitor by index.
    pub fn capture_monitor_by_index(index: usize) -> Result<(MonitorInfo, RgbaImage)> {
        let monitors = Monitor::all()
            .map_err(|e| CoreError::Capture(format!("Failed to query monitors: {e}")))?;

        let monitor = monitors
            .get(index)
            .ok_or_else(|| CoreError::MonitorNotFound(format!("Monitor index {index} not found")))?;

        let info = MonitorInfo {
            id: monitor.id().unwrap_or(0),
            name: monitor.name().unwrap_or_else(|_| "Unknown".into()),
            is_primary: monitor.is_primary().unwrap_or(false),
            x: monitor.x().unwrap_or(0),
            y: monitor.y().unwrap_or(0),
            width: monitor.width().unwrap_or(0),
            height: monitor.height().unwrap_or(0),
            scale_factor: monitor.scale_factor().unwrap_or(1.0),
        };

        let raw_img = monitor
            .capture_image()
            .map_err(|e| CoreError::Capture(format!("Failed to capture monitor: {e}")))?;

        let (w, h) = raw_img.dimensions();
        let rgba_img = RgbaImage::from_raw(w, h, raw_img.into_raw())
            .ok_or_else(|| CoreError::Image("Failed to construct RgbaImage buffer".into()))?;

        Ok((info, rgba_img))
    }

    /// Queries all visible on-screen windows (useful for snapping).
    pub fn get_windows() -> Result<Vec<WindowInfo>> {
        let windows = Window::all()
            .map_err(|e| CoreError::Capture(format!("Failed to list windows: {e}")))?;

        let list = windows
            .into_iter()
            .filter_map(|w| {
                let is_minimized = w.is_minimized().unwrap_or(false);
                let width = w.width().unwrap_or(0);
                let height = w.height().unwrap_or(0);
                // Filter out zero-size or minimized windows
                if is_minimized || width == 0 || height == 0 {
                    return None;
                }

                Some(WindowInfo {
                    id: w.id().unwrap_or(0),
                    title: w.title().unwrap_or_default(),
                    app_name: w.app_name().unwrap_or_default(),
                    x: w.x().unwrap_or(0),
                    y: w.y().unwrap_or(0),
                    width,
                    height,
                    is_minimized,
                })
            })
            .collect();

        Ok(list)
    }

    /// Crops a sub-region from an image with bounds checking.
    pub fn crop_region(
        img: &RgbaImage,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<RgbaImage> {
        let (img_w, img_h) = img.dimensions();
        if width == 0 || height == 0 {
            return Err(CoreError::Image("Crop width and height must be > 0".into()));
        }

        if x >= img_w || y >= img_h {
            return Err(CoreError::Image(format!(
                "Crop origin ({x}, {y}) out of image bounds ({img_w}x{img_h})"
            )));
        }

        let clamped_w = (width).min(img_w - x);
        let clamped_h = (height).min(img_h - y);

        debug!("Cropping region: x={x}, y={y}, w={clamped_w}, h={clamped_h}");
        let cropped = image::imageops::crop_imm(img, x, y, clamped_w, clamped_h).to_image();
        Ok(cropped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn test_crop_region_valid() {
        let mut img = RgbaImage::new(100, 100);
        img.put_pixel(10, 10, Rgba([255, 0, 0, 255]));

        let cropped = ScreenCapture::crop_region(&img, 10, 10, 20, 20).unwrap();
        assert_eq!(cropped.width(), 20);
        assert_eq!(cropped.height(), 20);
        assert_eq!(cropped.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn test_crop_region_out_of_bounds() {
        let img = RgbaImage::new(50, 50);
        let result = ScreenCapture::crop_region(&img, 100, 100, 10, 10);
        assert!(result.is_err());
    }
}
