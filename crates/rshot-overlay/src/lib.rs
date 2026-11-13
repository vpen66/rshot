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

/// Freezes the primary monitor and launches the interactive screenshot overlay window.
pub fn launch_overlay() -> Result<(), Box<dyn std::error::Error>> {
    info!("Capturing primary monitor for overlay...");
    let (monitor_info, captured_image) = ScreenCapture::capture_primary_monitor()?;

    if is_blank_capture(&captured_image) {
        return Err(
            "Captured a solid black frame: screen recording permission is likely not granted \
             to the terminal/IDE launching rshot (macOS: System Settings -> Privacy & Security \
             -> Screen Recording). Grant it and retry."
                .into(),
        );
    }

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

    eframe::run_native(
        "rshot",
        native_options,
        Box::new(move |cc| {
            install_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(OverlayApp::new(
                captured_image,
                scale_factor,
                monitor_origin,
            )))
        }),
    )?;

    Ok(())
}

/// A uniformly near-black frame means the OS denied screen capture (missing permission).
fn is_blank_capture(img: &RgbaImage) -> bool {
    let first = *img.get_pixel(0, 0);
    first[0] < 40 && first[1] < 40 && first[2] < 40 && img.pixels().all(|p| *p == first)
}

// egui's built-in fonts have no CJK glyphs; without this Chinese text renders as tofu boxes.
fn install_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "noto_sans_sc".to_owned(),
        egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/NotoSansSC-Regular.otf"
        ))
        .into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "noto_sans_sc".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("noto_sans_sc".to_owned());
    ctx.set_fonts(fonts);
}