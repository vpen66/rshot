pub mod app;

pub use app::PinApp;

use egui::ViewportBuilder;
use image::RgbaImage;
use log::info;

/// Launches a pinned, always-on-top, borderless floating sticker window.
pub fn launch_pin(
    image: RgbaImage,
    pos_x: f32,
    pos_y: f32,
    scale_factor: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let (w, h) = image.dimensions();
    let logical_w = w as f32 / scale_factor;
    let logical_h = h as f32 / scale_factor;

    info!(
        "Launching Pin window: {}x{} at ({}, {}) with scale {}",
        logical_w, logical_h, pos_x, pos_y, scale_factor
    );

    let viewport = ViewportBuilder::default()
        .with_title("rshot-pin")
        .with_decorations(false)
        .with_always_on_top()
        .with_transparent(true)
        .with_position([pos_x, pos_y])
        .with_inner_size([logical_w, logical_h]);

    let native_options = eframe::NativeOptions {
        viewport,
        centered: false,
        ..Default::default()
    };

    eframe::run_native(
        "rshot-pin",
        native_options,
        Box::new(move |cc| {
            install_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(PinApp::new(image, scale_factor)))
        }),
    )?;

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn test_pin_image_rotation() {
        let mut img = RgbaImage::new(10, 20);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));

        let rotated = image::imageops::rotate90(&img);
        assert_eq!(rotated.width(), 20);
        assert_eq!(rotated.height(), 10);
        assert_eq!(rotated.get_pixel(19, 0), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn test_pin_state_defaults() {
        let img = RgbaImage::new(50, 50);
        let pin = PinApp::new(img, 2.0);
        assert_eq!(pin.zoom(), 1.0);
        assert_eq!(pin.opacity(), 1.0);
        assert!(!pin.click_through());
    }
}
