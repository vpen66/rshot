use ab_glyph::{Font, FontRef, PxScale, point};
use image::{Rgba, RgbaImage};
use std::sync::OnceLock;

/// Bundled CJK font so exported text annotations match the overlay's rendering.
const TEXT_FONT_BYTES: &[u8] = include_bytes!("../../../../assets/fonts/NotoSansSC-Regular.otf");

fn text_font() -> &'static FontRef<'static> {
    static FONT: OnceLock<FontRef<'static>> = OnceLock::new();
    FONT.get_or_init(|| {
        FontRef::try_from_slice(TEXT_FONT_BYTES).expect("bundled NotoSansSC font must be valid")
    })
}

/// Low-level image processing operations for annotations and visual effects.
pub struct ImageProcessor;

impl ImageProcessor {
    /// Rasterizes a single line of text at (x, y) top-left with the bundled font.
    pub fn draw_text(
        img: &mut RgbaImage,
        x: i32,
        y: i32,
        text: &str,
        font_size_px: f32,
        color: Rgba<u8>,
    ) {
        let font = text_font();
        let units = font.units_per_em().unwrap_or(1000.0);
        let scale = PxScale::from(font_size_px.max(1.0));
        let ascent = font.ascent_unscaled() * scale.y / units;
        let baseline = y as f32 + ascent;
        let (img_w, img_h) = img.dimensions();
        let mut caret = x as f32;

        for ch in text.chars() {
            let id = font.glyph_id(ch);
            let glyph = id.with_scale_and_position(scale, point(caret, baseline));
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|px, py, coverage| {
                    let tx = bounds.min.x as i32 + px as i32;
                    let ty = bounds.min.y as i32 + py as i32;
                    if tx >= 0 && ty >= 0 && (tx as u32) < img_w && (ty as u32) < img_h {
                        let alpha = (color[3] as f32 * coverage).round() as u8;
                        if alpha > 0 {
                            let p = img.get_pixel_mut(tx as u32, ty as u32);
                            Self::blend_pixel(p, Rgba([color[0], color[1], color[2], alpha]));
                        }
                    }
                });
            }
            caret += font.h_advance_unscaled(id) * scale.x / units;
        }
    }

    /// Applies a pixelated mosaic effect to a rectangular region.
    pub fn apply_mosaic(
        img: &mut RgbaImage,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        block_size: u32,
    ) {
        if block_size <= 1 || width == 0 || height == 0 {
            return;
        }

        let (img_w, img_h) = img.dimensions();
        let end_x = (x + width).min(img_w);
        let end_y = (y + height).min(img_h);

        let mut by = y;
        while by < end_y {
            let bh = block_size.min(end_y - by);
            let mut bx = x;
            while bx < end_x {
                let bw = block_size.min(end_x - bx);

                // Compute average color in this block
                let mut sum_r: u64 = 0;
                let mut sum_g: u64 = 0;
                let mut sum_b: u64 = 0;
                let mut sum_a: u64 = 0;
                let count = (bw * bh) as u64;

                for py in by..(by + bh) {
                    for px in bx..(bx + bw) {
                        let p = img.get_pixel(px, py);
                        sum_r += p[0] as u64;
                        sum_g += p[1] as u64;
                        sum_b += p[2] as u64;
                        sum_a += p[3] as u64;
                    }
                }

                let avg_color = Rgba([
                    (sum_r / count) as u8,
                    (sum_g / count) as u8,
                    (sum_b / count) as u8,
                    (sum_a / count) as u8,
                ]);

                // Fill block with average color
                for py in by..(by + bh) {
                    for px in bx..(bx + bw) {
                        img.put_pixel(px, py, avg_color);
                    }
                }

                bx += block_size;
            }
            by += block_size;
        }
    }

    /// Applies a fast box blur effect to a rectangular region.
    pub fn apply_blur(
        img: &mut RgbaImage,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        radius: u32,
    ) {
        if radius == 0 || width == 0 || height == 0 {
            return;
        }

        let (img_w, img_h) = img.dimensions();
        let x0 = x.min(img_w);
        let y0 = y.min(img_h);
        let x1 = (x + width).min(img_w);
        let y1 = (y + height).min(img_h);
        let w = x1 - x0;
        let h = y1 - y0;

        if w == 0 || h == 0 {
            return;
        }

        let r = radius as i32;
        let mut temp = img.clone();

        // Horizontal pass
        for cy in y0..y1 {
            for cx in x0..x1 {
                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut sum_a = 0u32;
                let mut count = 0u32;

                let min_k = (cx as i32 - r).max(x0 as i32);
                let max_k = (cx as i32 + r).min(x1 as i32 - 1);

                for kx in min_k..=max_k {
                    let p = img.get_pixel(kx as u32, cy);
                    sum_r += p[0] as u32;
                    sum_g += p[1] as u32;
                    sum_b += p[2] as u32;
                    sum_a += p[3] as u32;
                    count += 1;
                }

                if count > 0 {
                    temp.put_pixel(
                        cx,
                        cy,
                        Rgba([
                            (sum_r / count) as u8,
                            (sum_g / count) as u8,
                            (sum_b / count) as u8,
                            (sum_a / count) as u8,
                        ]),
                    );
                }
            }
        }

        // Vertical pass
        for cx in x0..x1 {
            for cy in y0..y1 {
                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut sum_a = 0u32;
                let mut count = 0u32;

                let min_k = (cy as i32 - r).max(y0 as i32);
                let max_k = (cy as i32 + r).min(y1 as i32 - 1);

                for ky in min_k..=max_k {
                    let p = temp.get_pixel(cx, ky as u32);
                    sum_r += p[0] as u32;
                    sum_g += p[1] as u32;
                    sum_b += p[2] as u32;
                    sum_a += p[3] as u32;
                    count += 1;
                }

                if count > 0 {
                    img.put_pixel(
                        cx,
                        cy,
                        Rgba([
                            (sum_r / count) as u8,
                            (sum_g / count) as u8,
                            (sum_b / count) as u8,
                            (sum_a / count) as u8,
                        ]),
                    );
                }
            }
        }
    }

    /// Blends a pixel with alpha compositing over target pixel.
    #[inline]
    fn blend_pixel(target: &mut Rgba<u8>, source: Rgba<u8>) {
        let src_a = source[3] as f32 / 255.0;
        let inv_a = 1.0 - src_a;

        target[0] = ((source[0] as f32 * src_a) + (target[0] as f32 * inv_a)).round() as u8;
        target[1] = ((source[1] as f32 * src_a) + (target[1] as f32 * inv_a)).round() as u8;
        target[2] = ((source[2] as f32 * src_a) + (target[2] as f32 * inv_a)).round() as u8;
        target[3] = ((source[3] as f32 * src_a) + (target[3] as f32 * inv_a)).round() as u8;
    }

    /// Draws a solid disk/circle at (cx, cy) with given radius.
    pub fn draw_circle(img: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: Rgba<u8>) {
        let (w, h) = img.dimensions();
        let r2 = radius * radius;

        let min_x = (cx - radius).max(0);
        let max_x = (cx + radius).min(w as i32 - 1);
        let min_y = (cy - radius).max(0);
        let max_y = (cy + radius).min(h as i32 - 1);

        for y in min_y..=max_y {
            let dy = y - cy;
            for x in min_x..=max_x {
                let dx = x - cx;
                if dx * dx + dy * dy <= r2 {
                    let p = img.get_pixel_mut(x as u32, y as u32);
                    Self::blend_pixel(p, color);
                }
            }
        }
    }

    /// Draws a line with thickness between two points using Bresenham / stepped circles.
    pub fn draw_thick_line(
        img: &mut RgbaImage,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        thickness: u32,
        color: Rgba<u8>,
    ) {
        let radius = (thickness / 2).max(1) as i32;
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let steps = dx.max(dy).max(1);

        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let cx = (x0 as f32 + (x1 - x0) as f32 * t).round() as i32;
            let cy = (y0 as f32 + (y1 - y0) as f32 * t).round() as i32;
            Self::draw_circle(img, cx, cy, radius, color);
        }
    }

    /// Draws an arrow with a pointy head from (x0, y0) to (x1, y1).
    pub fn draw_arrow(
        img: &mut RgbaImage,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        thickness: u32,
        color: Rgba<u8>,
    ) {
        // Draw the main shaft
        Self::draw_thick_line(img, x0, y0, x1, y1, thickness, color);

        // Arrow head calculations
        let dx = (x1 - x0) as f32;
        let dy = (y1 - y0) as f32;
        let length = (dx * dx + dy * dy).sqrt();
        if length < 5.0 {
            return;
        }

        let head_length = (thickness as f32 * 4.5).clamp(16.0, 36.0);
        let head_angle = 28.0f32.to_radians();

        let angle = dy.atan2(dx);
        let left_angle = angle + std::f32::consts::PI - head_angle;
        let right_angle = angle + std::f32::consts::PI + head_angle;

        let left_x = (x1 as f32 + head_length * left_angle.cos()).round() as i32;
        let left_y = (y1 as f32 + head_length * left_angle.sin()).round() as i32;

        let right_x = (x1 as f32 + head_length * right_angle.cos()).round() as i32;
        let right_y = (y1 as f32 + head_length * right_angle.sin()).round() as i32;

        // Draw arrow wings
        Self::draw_thick_line(img, x1, y1, left_x, left_y, thickness, color);
        Self::draw_thick_line(img, x1, y1, right_x, right_y, thickness, color);
    }

    /// Draws a rectangle outline or filled rectangle.
    pub fn draw_rect(
        img: &mut RgbaImage,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        thickness: u32,
        color: Rgba<u8>,
        filled: bool,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        let right = x + width as i32;
        let bottom = y + height as i32;

        if filled {
            let (img_w, img_h) = img.dimensions();
            let min_x = x.max(0) as u32;
            let max_x = (right).min(img_w as i32) as u32;
            let min_y = y.max(0) as u32;
            let max_y = (bottom).min(img_h as i32) as u32;

            for cy in min_y..max_y {
                for cx in min_x..max_x {
                    let p = img.get_pixel_mut(cx, cy);
                    Self::blend_pixel(p, color);
                }
            }
        } else {
            Self::draw_thick_line(img, x, y, right, y, thickness, color);
            Self::draw_thick_line(img, right, y, right, bottom, thickness, color);
            Self::draw_thick_line(img, right, bottom, x, bottom, thickness, color);
            Self::draw_thick_line(img, x, bottom, x, y, thickness, color);
        }
    }

    /// Draws an ellipse outline or filled ellipse.
    pub fn draw_ellipse(
        img: &mut RgbaImage,
        cx: i32,
        cy: i32,
        rx: i32,
        ry: i32,
        thickness: u32,
        color: Rgba<u8>,
        filled: bool,
    ) {
        if rx <= 0 || ry <= 0 {
            return;
        }

        let (w, h) = img.dimensions();
        let min_x = (cx - rx - thickness as i32).max(0);
        let max_x = (cx + rx + thickness as i32).min(w as i32 - 1);
        let min_y = (cy - ry - thickness as i32).max(0);
        let max_y = (cy + ry + thickness as i32).min(h as i32 - 1);

        let rx_f = rx as f32;
        let ry_f = ry as f32;
        let half_t = (thickness as f32 / 2.0).max(0.5);

        for py in min_y..=max_y {
            let dy = (py - cy) as f32;
            for px in min_x..=max_x {
                let dx = (px - cx) as f32;
                let norm_dist = (dx / rx_f).powi(2) + (dy / ry_f).powi(2);

                if filled {
                    if norm_dist <= 1.0 {
                        let p = img.get_pixel_mut(px as u32, py as u32);
                        Self::blend_pixel(p, color);
                    }
                } else {
                    // Ring within thickness
                    let dist = norm_dist.sqrt();
                    let target_dist = 1.0;
                    let pixel_dist = (dist - target_dist).abs() * rx_f.min(ry_f);
                    if pixel_dist <= half_t {
                        let p = img.get_pixel_mut(px as u32, py as u32);
                        Self::blend_pixel(p, color);
                    }
                }
            }
        }
    }

    /// Draws a numbered step badge (e.g. ①, ②, ...) at (cx, cy).
    pub fn draw_step_badge(
        img: &mut RgbaImage,
        cx: i32,
        cy: i32,
        radius: i32,
        number: usize,
        bg_color: Rgba<u8>,
    ) {
        // Draw background colored circle
        Self::draw_circle(img, cx, cy, radius, bg_color);
        // Draw white border
        let border_color = Rgba([255, 255, 255, 255]);
        let thickness = (radius / 6).max(2) as u32;
        Self::draw_ellipse(img, cx, cy, radius, radius, thickness, border_color, false);

        Self::draw_badge_number(img, cx, cy, number);
    }

    /// Draws a numbered square step badge at (cx, cy).
    pub fn draw_step_badge_square(
        img: &mut RgbaImage,
        cx: i32,
        cy: i32,
        radius: i32,
        number: usize,
        bg_color: Rgba<u8>,
    ) {
        let side = (radius * 2).max(1) as u32;
        Self::draw_rect(img, cx - radius, cy - radius, side, side, 1, bg_color, true);
        let border = Rgba([255, 255, 255, 255]);
        let thickness = (radius / 6).max(2) as u32;
        Self::draw_rect(img, cx - radius, cy - radius, side, side, thickness, border, false);
        Self::draw_badge_number(img, cx, cy, number);
    }

    fn draw_badge_number(img: &mut RgbaImage, cx: i32, cy: i32, number: usize) {
        let text_color = Rgba([255, 255, 255, 255]);
        let num_str = number.to_string();
        let digit_w = 4;
        let digit_h = 7;
        let total_w = num_str.len() as i32 * (digit_w + 1) - 1;
        let start_x = cx - total_w / 2;
        let start_y = cy - digit_h / 2;
        for (i, ch) in num_str.chars().enumerate() {
            let dx = start_x + i as i32 * (digit_w + 1);
            Self::draw_glyph(img, dx, start_y, ch, text_color);
        }
    }

    /// Fills (even-odd scanline) and/or strokes a polygon path.
    pub fn draw_polygon(
        img: &mut RgbaImage,
        points: &[(i32, i32)],
        thickness: u32,
        color: Rgba<u8>,
        filled: bool,
        closed: bool,
    ) {
        if points.len() < 2 {
            return;
        }
        if filled && closed && points.len() >= 3 {
            let (w, h) = img.dimensions();
            let min_y = points.iter().map(|p| p.1).min().unwrap().max(0);
            let max_y = points.iter().map(|p| p.1).max().unwrap().min(h as i32 - 1);
            for y in min_y..=max_y {
                let mut xs: Vec<f32> = Vec::new();
                for i in 0..points.len() {
                    let (x0, y0) = points[i];
                    let (x1, y1) = points[(i + 1) % points.len()];
                    if (y0 <= y && y1 > y) || (y1 <= y && y0 > y) {
                        let t = (y - y0) as f32 / (y1 - y0) as f32;
                        xs.push(x0 as f32 + t * (x1 - x0) as f32);
                    }
                }
                xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                for pair in xs.chunks(2) {
                    if pair.len() < 2 {
                        continue;
                    }
                    let xa = pair[0].ceil().max(0.0) as u32;
                    let xb = pair[1].floor().min((w - 1) as f32) as u32;
                    for x in xa..=xb {
                        let p = img.get_pixel_mut(x, y as u32);
                        Self::blend_pixel(p, color);
                    }
                }
            }
        }
        let segments = if closed { points.len() } else { points.len() - 1 };
        for i in 0..segments {
            let (x0, y0) = points[i];
            let (x1, y1) = points[(i + 1) % points.len()];
            Self::draw_thick_line(img, x0, y0, x1, y1, thickness, color);
        }
    }

    /// Draws a rounded rectangle outline or fill, approximating corners with arcs.
    pub fn draw_rounded_rect(
        img: &mut RgbaImage,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        radius: i32,
        thickness: u32,
        color: Rgba<u8>,
        filled: bool,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        let r = radius.max(0).min((width.min(height) / 2) as i32);
        if r <= 0 {
            Self::draw_rect(img, x, y, width, height, thickness, color, filled);
            return;
        }
        let (x0, y0) = (x as f32, y as f32);
        let (x1, y1) = (x as f32 + width as f32, y as f32 + height as f32);
        let rf = r as f32;
        let pi = std::f32::consts::PI;
        let corners = [
            (x1 - rf, y0 + rf, -pi / 2.0, 0.0),
            (x1 - rf, y1 - rf, 0.0, pi / 2.0),
            (x0 + rf, y1 - rf, pi / 2.0, pi),
            (x0 + rf, y0 + rf, pi, 1.5 * pi),
        ];
        let steps = 6;
        let mut pts: Vec<(i32, i32)> = Vec::new();
        for (cx, cy, a0, a1) in corners {
            for k in 0..=steps {
                let a = a0 + (a1 - a0) * k as f32 / steps as f32;
                pts.push(((cx + rf * a.cos()).round() as i32, (cy + rf * a.sin()).round() as i32));
            }
        }
        Self::draw_polygon(img, &pts, thickness, color, filled, true);
    }

    /// Renders a basic 3x5/4x7 micro-bitmap glyph for number characters.
    fn draw_glyph(img: &mut RgbaImage, x: i32, y: i32, ch: char, color: Rgba<u8>) {
        let (w, h) = img.dimensions();
        let pattern: [u8; 7] = match ch {
            '0' => [0b1110, 0b1001, 0b1001, 0b1001, 0b1001, 0b1001, 0b1110],
            '1' => [0b0100, 0b1100, 0b0100, 0b0100, 0b0100, 0b0100, 0b1110],
            '2' => [0b1110, 0b0001, 0b0001, 0b1110, 0b1000, 0b1000, 0b1111],
            '3' => [0b1110, 0b0001, 0b0001, 0b0110, 0b0001, 0b0001, 0b1110],
            '4' => [0b1001, 0b1001, 0b1001, 0b1111, 0b0001, 0b0001, 0b0001],
            '5' => [0b1111, 0b1000, 0b1000, 0b1110, 0b0001, 0b0001, 0b1110],
            '6' => [0b1110, 0b1000, 0b1000, 0b1110, 0b1001, 0b1001, 0b1110],
            '7' => [0b1111, 0b0001, 0b0010, 0b0100, 0b0100, 0b0100, 0b0100],
            '8' => [0b1110, 0b1001, 0b1001, 0b1110, 0b1001, 0b1001, 0b1110],
            '9' => [0b1110, 0b1001, 0b1001, 0b1111, 0b0001, 0b0001, 0b1110],
            _ => [0b0000, 0b0000, 0b0000, 0b0000, 0b0000, 0b0000, 0b0000],
        };

        for row in 0..7 {
            let row_bits = pattern[row];
            for col in 0..4 {
                if (row_bits & (1 << (3 - col))) != 0 {
                    let px = x + col;
                    let py = y + row as i32;
                    if px >= 0 && px < w as i32 && py >= 0 && py < h as i32 {
                        let p = img.get_pixel_mut(px as u32, py as u32);
                        Self::blend_pixel(p, color);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mosaic_effect() {
        let mut img = RgbaImage::new(20, 20);
        // Fill with alternating pattern
        for y in 0..20 {
            for x in 0..20 {
                let c = if (x + y) % 2 == 0 { 255 } else { 0 };
                img.put_pixel(x, y, Rgba([c, c, c, 255]));
            }
        }

        ImageProcessor::apply_mosaic(&mut img, 0, 0, 20, 20, 4);

        // Every 4x4 block should have identical pixel values
        let block_color = img.get_pixel(0, 0);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(img.get_pixel(x, y), block_color);
            }
        }
    }

    #[test]
    fn test_blur_effect() {
        let mut img = RgbaImage::new(30, 30);
        img.put_pixel(15, 15, Rgba([255, 255, 255, 255]));

        ImageProcessor::apply_blur(&mut img, 0, 0, 30, 30, 3);

        // Surrounding pixels should now have blurred intensity > 0
        let neighbor = img.get_pixel(14, 15);
        assert!(neighbor[0] > 0);
    }

    #[test]
    fn test_draw_primitives() {
        let mut img = RgbaImage::new(100, 100);
        let red = Rgba([255, 0, 0, 255]);

        ImageProcessor::draw_rect(&mut img, 10, 10, 20, 20, 2, red, false);
        ImageProcessor::draw_circle(&mut img, 50, 50, 10, red);
        ImageProcessor::draw_arrow(&mut img, 10, 10, 80, 80, 3, red);
        ImageProcessor::draw_step_badge(&mut img, 30, 30, 12, 1, red);

        assert_eq!(img.get_pixel(50, 50), &red);
    }
}
