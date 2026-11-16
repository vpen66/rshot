//! Menu-bar / tray glyph: a viewfinder drawn from signed distance fields so
//! edges stay smooth at any scale, unlike a hard-pixel bitmap.

// Canvas matches the macOS menu bar cap; artwork sits in the middle 16pt so it
// carries the same visual weight as system menu bar extras.
const CANVAS_PT: f32 = 22.0;
const INSET: f32 = 3.0;
const STROKE: f32 = 1.8;
const ARM: f32 = 5.4;
const DOT_R: f32 = 2.2;

fn sd_round_box(px: f32, py: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - hx + r;
    let qy = (py - cy).abs() - hy + r;
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.min(0.0).max(qy.min(0.0)) - r
}

fn glyph_distance(px: f32, py: f32) -> f32 {
    // Fold into the top-left quadrant: all four brackets are mirror images.
    let qx = px.min(CANVAS_PT - px);
    let qy = py.min(CANVAS_PT - py);
    let r = STROKE / 2.0;
    let horizontal = sd_round_box(
        qx,
        qy,
        INSET + ARM / 2.0,
        INSET + STROKE / 2.0,
        ARM / 2.0,
        STROKE / 2.0,
        r,
    );
    let vertical = sd_round_box(
        qx,
        qy,
        INSET + STROKE / 2.0,
        INSET + ARM / 2.0,
        STROKE / 2.0,
        ARM / 2.0,
        r,
    );
    let dot = ((px - CANVAS_PT / 2.0).powi(2) + (py - CANVAS_PT / 2.0).powi(2)).sqrt() - DOT_R;
    horizontal.min(vertical).min(dot)
}

/// Straight-alpha RGBA of the glyph at `size` x `size` pixels, tinted `rgb`.
pub fn rgba(size: u32, rgb: [u8; 3]) -> Vec<u8> {
    let scale = size as f32 / CANVAS_PT;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let d = glyph_distance((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale) * scale;
            let coverage = (0.5 - d).clamp(0.0, 1.0);
            out.extend_from_slice(&rgb);
            out.push((coverage * 255.0).round() as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(buf: &[u8], size: u32, x: u32, y: u32) -> u8 {
        buf[((y * size + x) * 4 + 3) as usize]
    }

    #[test]
    fn glyph_is_four_fold_symmetric() {
        const S: u32 = 44;
        let buf = rgba(S, [0, 0, 0]);
        for y in 0..S {
            for x in 0..S {
                let a = alpha_at(&buf, S, x, y);
                assert_eq!(a, alpha_at(&buf, S, S - 1 - x, y));
                assert_eq!(a, alpha_at(&buf, S, x, S - 1 - y));
            }
        }
    }

    #[test]
    fn solid_inside_transparent_outside() {
        const S: u32 = 44;
        let buf = rgba(S, [0, 0, 0]);
        let px = |pt: f32| (pt * 2.0) as u32;
        assert_eq!(alpha_at(&buf, S, px(11.0), px(11.0)), 255); // dot
        assert_eq!(alpha_at(&buf, S, px(4.0), px(3.8)), 255); // top-left bracket
        assert_eq!(alpha_at(&buf, S, px(11.0), px(3.8)), 0); // gap between brackets
        assert_eq!(alpha_at(&buf, S, px(0.5), px(0.5)), 0); // outside artwork
        assert_eq!(alpha_at(&buf, S, px(11.0), px(8.0)), 0); // between dot and bracket
    }

    #[test]
    fn edges_are_antialiased() {
        let buf = rgba(44, [0, 0, 0]);
        assert!(buf.chunks_exact(4).any(|p| p[3] > 0 && p[3] < 255));
    }

    #[test]
    #[ignore = "writes /tmp/rshot-tray-icon-preview.png for visual inspection"]
    fn dump_preview_sheet() {
        const CELL: u32 = 176;
        let bgs = [[245u8, 245, 247], [28, 28, 30], [28, 28, 30]];
        let fgs = [[0u8, 0, 0], [255, 255, 255], [24, 144, 255]];
        let mut sheet = image::RgbaImage::new(CELL * 3, CELL * 3);
        for row in 0..3u32 {
            for col in 0..3u32 {
                let px = [44u32, 22, 88][col as usize];
                let up = CELL / px;
                let glyph = rgba(px, fgs[row as usize]);
                for y in 0..CELL {
                    for x in 0..CELL {
                        let i = (((y / up) * px + x / up) * 4) as usize;
                        let a = glyph[i + 3] as u32;
                        let mut out = [0u8; 4];
                        for c in 0..3 {
                            out[c] = ((fgs[row as usize][c] as u32 * a
                                + bgs[row as usize][c] as u32 * (255 - a))
                                / 255) as u8;
                        }
                        out[3] = 255;
                        sheet.put_pixel(col * CELL + x, row * CELL + y, image::Rgba(out));
                    }
                }
            }
        }
        sheet.save("/tmp/rshot-tray-icon-preview.png").unwrap();
    }
}
