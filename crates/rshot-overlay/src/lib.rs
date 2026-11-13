pub mod annotation;
pub mod app;
pub mod magnifier;
pub mod state;
pub mod toolbar;

pub use app::OverlayApp;
pub use state::Selection;

use eframe::egui::ViewportBuilder;
use image::RgbaImage;
use log::info;
use rshot_core::ScreenCapture;
use std::sync::{Arc, Mutex};

/// Request payload emitted when user pins a screenshot.
#[derive(Debug, Clone)]
pub struct PinRequest {
    pub image: RgbaImage,
    pub x: f32,
    pub y: f32,
    pub scale_factor: f32,
}

/// Freezes the primary monitor and launches the interactive screenshot overlay window.
/// Returns Some(PinRequest) if user opted to pin the screenshot, or None on exit/copy/save/cancel.
pub fn launch_overlay() -> Result<Option<PinRequest>, Box<dyn std::error::Error>> {
    info!("Capturing primary monitor for overlay...");
    let (monitor_info, captured_image) = ScreenCapture::capture_primary_monitor()?;

    let scale_factor = monitor_info.scale_factor;
    let logical_width = monitor_info.width as f32 / scale_factor;
    let logical_height = monitor_info.height as f32 / scale_factor;
    let monitor_origin = (monitor_info.x as f32, monitor_info.y as f32);

    info!(
        "Display: {}x{} (Logical: {}x{}, DPI scale: {})",
        monitor_info.width, monitor_info.height, logical_width, logical_height, scale_factor
    );

    let viewport = ViewportBuilder::default()
        .with_title("rshot-overlay")
        .with_decorations(false)
        .with_always_on_top()
        .with_inner_size([logical_width, logical_height])
        .with_position([monitor_origin.0, monitor_origin.1])
        .with_transparent(true);

    let native_options = eframe::NativeOptions {
        viewport,
        centered: false,
        ..Default::default()
    };

    let pin_slot = Arc::new(Mutex::new(None));
    let pin_slot_clone = pin_slot.clone();

    eframe::run_native(
        "rshot",
        native_options,
        Box::new(move |_cc| {
            Ok(Box::new(OverlayApp::new(
                captured_image,
                scale_factor,
                monitor_origin,
                pin_slot_clone,
            )))
        }),
    )?;

    let result = pin_slot.lock().unwrap().take();
    Ok(result)
}
