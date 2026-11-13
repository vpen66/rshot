pub mod clipboard;
pub mod error;
pub mod image_processor;
pub mod screenshot;

pub use clipboard::ClipboardService;
pub use error::{CoreError, Result};
pub use image_processor::ImageProcessor;
pub use screenshot::{MonitorInfo, ScreenCapture, WindowInfo};
