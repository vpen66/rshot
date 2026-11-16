use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2,
};
use image::RgbaImage;

/// Magnifier loupe rendering service.
pub struct Magnifier;

impl Magnifier {
    /// Renders the pixel loupe at the specified cursor position.
    pub fn render(
        painter: &Painter,
        cursor_pos: Pos2,
        screen_size: Vec2,
        image: &RgbaImage,
        scale_factor: f32,
    ) {
        // Physical image coordinates
        let img_x = (cursor_pos.x * scale_factor).round() as i32;
        let img_y = (cursor_pos.y * scale_factor).round() as i32;

        let (img_w, img_h) = image.dimensions();
        if img_x < 0 || img_y < 0 || img_x >= img_w as i32 || img_y >= img_h as i32 {
            return;
        }

        let center_pixel = image.get_pixel(img_x as u32, img_y as u32);
        let hex_color = format!(
            "#{:02X}{:02X}{:02X}",
            center_pixel[0], center_pixel[1], center_pixel[2]
        );
        let rgb_str = format!(
            "RGB({}, {}, {})",
            center_pixel[0], center_pixel[1], center_pixel[2]
        );
        let pos_str = format!("POS({}, {})", cursor_pos.x as i32, cursor_pos.y as i32);

        // Magnifier dimensions
        let grid_size = 9; // 9x9 pixel grid
        let half_grid = grid_size / 2;
        let cell_size = 12.0; // Display size for each pixel
        let loupe_w = grid_size as f32 * cell_size;
        let loupe_h = grid_size as f32 * cell_size + 42.0; // Space for text info

        // Offset loupe away from cursor
        let mut loupe_x = cursor_pos.x + 20.0;
        let mut loupe_y = cursor_pos.y + 20.0;

        // Keep loupe within screen bounds
        if loupe_x + loupe_w + 10.0 > screen_size.x {
            loupe_x = cursor_pos.x - loupe_w - 20.0;
        }
        if loupe_y + loupe_h + 10.0 > screen_size.y {
            loupe_y = cursor_pos.y - loupe_h - 20.0;
        }

        let loupe_rect = Rect::from_min_size(Pos2::new(loupe_x, loupe_y), Vec2::new(loupe_w, loupe_h));

        // Draw shadow and background container
        painter.rect(
            loupe_rect.expand(2.0),
            4.0,
            Color32::from_black_alpha(220),
            Stroke::new(1.5, Color32::from_rgb(40, 150, 255)),
            StrokeKind::Middle,
        );

        // Draw pixel grid
        let grid_origin = Pos2::new(loupe_x, loupe_y);
        for dy in -half_grid..=half_grid {
            for dx in -half_grid..=half_grid {
                let px = img_x + dx;
                let py = img_y + dy;

                let color = if px >= 0 && px < img_w as i32 && py >= 0 && py < img_h as i32 {
                    let p = image.get_pixel(px as u32, py as u32);
                    Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])
                } else {
                    Color32::BLACK
                };

                let cell_rect = Rect::from_min_size(
                    Pos2::new(
                        grid_origin.x + (dx + half_grid) as f32 * cell_size,
                        grid_origin.y + (dy + half_grid) as f32 * cell_size,
                    ),
                    Vec2::splat(cell_size),
                );

                painter.rect_filled(cell_rect, 0.0, color);
                // Grid borders
                painter.rect_stroke(
                    cell_rect,
                    0.0,
                    Stroke::new(0.5, Color32::from_black_alpha(40)),
                    StrokeKind::Middle,
                );
            }
        }

        // Highlight center pixel
        let center_rect = Rect::from_min_size(
            Pos2::new(
                grid_origin.x + half_grid as f32 * cell_size,
                grid_origin.y + half_grid as f32 * cell_size,
            ),
            Vec2::splat(cell_size),
        );
        painter.rect_stroke(
            center_rect,
            0.0,
            Stroke::new(2.0, Color32::from_rgb(255, 50, 50)),
            StrokeKind::Middle,
        );

        // Text labels below pixel grid
        let text_y = loupe_y + (grid_size as f32 * cell_size) + 4.0;
        let font = FontId::new(11.0, FontFamily::Monospace);

        // RGB / Hex row
        let color_preview_rect = Rect::from_min_size(
            Pos2::new(loupe_x + 6.0, text_y + 3.0),
            Vec2::new(12.0, 12.0),
        );
        painter.rect_filled(
            color_preview_rect,
            2.0,
            Color32::from_rgb(center_pixel[0], center_pixel[1], center_pixel[2]),
        );
        painter.rect_stroke(color_preview_rect, 2.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Middle);

        painter.text(
            Pos2::new(loupe_x + 24.0, text_y + 2.0),
            Align2::LEFT_TOP,
            &format!("{hex_color}  {rgb_str}"),
            font.clone(),
            Color32::WHITE,
        );

        painter.text(
            Pos2::new(loupe_x + 6.0, text_y + 20.0),
            Align2::LEFT_TOP,
            &pos_str,
            font,
            Color32::LIGHT_GRAY,
        );
    }
}
