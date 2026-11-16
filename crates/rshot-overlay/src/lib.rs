pub mod annotation;
pub mod app;
mod icons;
pub mod magnifier;
pub mod state;
mod toast;
pub mod toolbar;

pub use app::OverlayApp;
pub use state::Selection;
pub use toast::show_toast;

use eframe::egui::ViewportBuilder;
use image::RgbaImage;
use log::info;
use rshot_core::ScreenCapture;

/// Freezes the primary monitor and launches the interactive screenshot overlay window.
pub fn launch_overlay() -> Result<(), Box<dyn std::error::Error>> {
    if !ScreenCapture::has_screen_recording_permission() {
        let target = match ScreenCapture::responsible_app_name() {
            Some(app) => format!("\"{app}\""),
            None => "the app that launches rshot".to_string(),
        };
        return Err(format!(
            "Screen recording permission is not granted to this process, so macOS hands back a \
             blank frame. rshot has no permission entry of its own: macOS files this under the app \
             that launched it, which is {target}. Enable that entry in System Settings -> Privacy \
             & Security -> Screen & System Audio Recording, then quit and reopen it before \
             launching rshot again."
        )
        .into());
    }

    info!("Capturing primary monitor for overlay...");
    let (monitor_info, captured_image) = ScreenCapture::capture_primary_monitor()?;

    if is_blank_capture(&captured_image) {
        return Err(
            "Captured a flat placeholder frame: macOS refused the capture even though the \
             permission check passed. Quit and reopen the app that launched rshot and retry."
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

    let mut native_options = eframe::NativeOptions {
        viewport,
        centered: false,
        ..Default::default()
    };
    // Accessory app: no Dock icon. winit still activates the window, so keyboard input works.
    #[cfg(target_os = "macos")]
    {
        native_options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory);
        }));
    }

    eframe::run_native(
        "rshot",
        native_options,
        Box::new(move |cc| {
            install_cjk_fonts(&cc.egui_ctx);
            // Dark tooltip / popup chrome matching the toolbar panel.
            let mut style = (*cc.egui_ctx.style()).clone();
            style.visuals.window_fill = egui::Color32::from_rgba_unmultiplied(24, 26, 31, 250);
            style.visuals.window_stroke =
                egui::Stroke::new(1.0, egui::Color32::from_white_alpha(40));
            style.visuals.window_corner_radius = egui::CornerRadius::same(7);
            cc.egui_ctx.set_style(style);
            Ok(Box::new(OverlayApp::new(
                captured_image,
                scale_factor,
                monitor_origin,
            )))
        }),
    )?;

    Ok(())
}

/// A perfectly uniform frame means the OS denied screen capture and substituted
/// a flat placeholder (e.g. #28282C) instead of reporting an error. Judging by
/// color consistency rather than brightness survives whatever fill color macOS
/// picks; a real desktop frame is never completely uniform.
fn is_blank_capture(img: &RgbaImage) -> bool {
    let mut pixels = img.pixels();
    match pixels.next() {
        Some(first) => pixels.all(|p| p == first),
        None => true,
    }
}

// egui's built-in fonts have no CJK glyphs; without this Chinese text renders as tofu boxes.
pub(crate) fn install_cjk_fonts(ctx: &egui::Context) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn blank_placeholder_is_detected() {
        // The exact frame macOS returned while the screen-recording grant was not
        // yet in effect: uniform #28282C, brightness above the old < 40 threshold.
        let img = RgbaImage::from_pixel(64, 64, Rgba([40, 40, 44, 255]));
        assert!(is_blank_capture(&img));
    }

    #[test]
    fn real_capture_is_not_blank() {
        let mut img = RgbaImage::from_pixel(64, 64, Rgba([40, 40, 44, 255]));
        img.put_pixel(10, 10, Rgba([118, 157, 185, 255]));
        assert!(!is_blank_capture(&img));
    }
}
