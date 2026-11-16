//! Vector toolbar icons drawn with the egui painter in a 16x16 design space.
//! Stroke-based, single-color, consistent optical weight across the whole set.

use crate::annotation::COLOR_PRESETS;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Select,
    Rect,
    Ellipse,
    Arrow,
    Line,
    Pen,
    Highlighter,
    Mosaic,
    Blur,
    Text,
    StepBadge,
    RoundedRect,
    Diamond,
    Polygon,
    DoubleArrow,
    Polyline,
    BadgeSquare,
    Fill,
    Undo,
    Redo,
    Trash,
    Pin,
    Copy,
    Save,
    Close,
    Minus,
    Plus,
    ColorWheel,
    Grip,
    Caret,
}

/// Draws `icon` centered in `area` (a square). `bg` is the panel color, used to
/// mask overlapping strokes (e.g. the front sheet of the copy icon).
pub fn draw(painter: &Painter, icon: Icon, area: Rect, color: Color32, bg: Color32) {
    let s = area.width() / 16.0;
    let p = |x: f32, y: f32| Pos2::new(area.min.x + x * s, area.min.y + y * s);
    let w = 1.5 * s;
    let line = |a: Pos2, b: Pos2, width: f32| {
        painter.line_segment([a, b], Stroke::new(width, color));
    };
    let poly = |pts: Vec<Pos2>, width: f32| {
        painter.add(Shape::closed_line(pts, Stroke::new(width, color)));
    };

    match icon {
        Icon::Select => {
            poly(
                vec![
                    p(4.0, 1.6),
                    p(4.0, 12.6),
                    p(7.1, 9.9),
                    p(9.2, 14.2),
                    p(11.2, 13.2),
                    p(9.1, 9.1),
                    p(13.2, 9.1),
                ],
                w,
            );
        }
        Icon::Rect => {
            painter.rect_stroke(
                Rect::from_min_max(p(2.5, 3.5), p(13.5, 12.5)),
                1.0 * s,
                Stroke::new(w, color),
                StrokeKind::Middle,
            );
        }
        Icon::Ellipse => {
            painter.circle_stroke(p(8.0, 8.0), 5.6 * s, Stroke::new(w, color));
        }
        Icon::Arrow => {
            line(p(3.0, 13.0), p(12.6, 3.4), w);
            line(p(12.6, 3.4), p(7.4, 3.9), w);
            line(p(12.6, 3.4), p(12.1, 8.6), w);
        }
        Icon::Line => {
            line(p(3.0, 13.0), p(13.0, 3.0), w);
        }
        Icon::Pen => {
            poly(
                vec![
                    p(2.8, 13.2),
                    p(4.0, 10.0),
                    p(10.6, 3.4),
                    p(12.6, 5.4),
                    p(6.0, 12.0),
                ],
                w * 0.95,
            );
            line(p(4.0, 10.0), p(6.0, 12.0), w * 0.95);
        }
        Icon::Highlighter => {
            line(p(4.6, 9.8), p(10.6, 3.8), 3.2 * s);
            line(p(3.0, 11.4), p(4.6, 9.8), 3.2 * s);
            line(p(3.0, 13.0), p(13.0, 13.0), w);
        }
        Icon::Mosaic => {
            let origin = p(2.5, 2.5);
            let cell = 11.0 * s / 3.0;
            // Checker cells suggest pixelation.
            for (cx, cy) in [(0, 0), (2, 0), (1, 1), (0, 2), (2, 2)] {
                let cell_rect = Rect::from_min_size(
                    Pos2::new(origin.x + cx as f32 * cell, origin.y + cy as f32 * cell),
                    Vec2::splat(cell),
                );
                painter.rect_filled(cell_rect, 0.0, color);
            }
            painter.rect_stroke(
                Rect::from_min_size(origin, Vec2::splat(11.0 * s)),
                0.0,
                Stroke::new(w * 0.9, color),
                StrokeKind::Middle,
            );
            for i in 1..3 {
                let d = i as f32 * cell;
                line(
                    Pos2::new(origin.x + d, origin.y),
                    Pos2::new(origin.x + d, origin.y + 11.0 * s),
                    w * 0.7,
                );
                line(
                    Pos2::new(origin.x, origin.y + d),
                    Pos2::new(origin.x + 11.0 * s, origin.y + d),
                    w * 0.7,
                );
            }
        }
        Icon::Blur => {
            // Concentric rings fading outward read as a soft focus / blur.
            for (r, alpha) in [(2.2, 255u8), (4.4, 150), (6.4, 75)] {
                let c = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);
                painter.circle_stroke(p(8.0, 8.0), r * s, Stroke::new(w * 0.9, c));
            }
        }
        Icon::Text => {
            line(p(4.0, 3.8), p(12.0, 3.8), w * 1.15);
            line(p(8.0, 3.8), p(8.0, 12.8), w * 1.15);
            line(p(5.8, 12.8), p(10.2, 12.8), w);
        }
        Icon::StepBadge => {
            painter.circle_stroke(p(8.0, 8.0), 5.8 * s, Stroke::new(w, color));
            painter.text(
                p(8.0, 8.2),
                Align2::CENTER_CENTER,
                "1",
                FontId::proportional(8.5 * s),
                color,
            );
        }
        Icon::RoundedRect => {
            painter.rect_stroke(
                Rect::from_min_max(p(2.5, 3.5), p(13.5, 12.5)),
                3.0 * s,
                Stroke::new(w, color),
                StrokeKind::Middle,
            );
        }
        Icon::Diamond => {
            poly(
                vec![p(8.0, 2.2), p(13.8, 8.0), p(8.0, 13.8), p(2.2, 8.0)],
                w,
            );
        }
        Icon::Polygon => {
            poly(
                vec![p(3.0, 12.5), p(4.5, 4.5), p(9.5, 2.8), p(13.2, 7.5), p(9.0, 13.2)],
                w,
            );
        }
        Icon::DoubleArrow => {
            line(p(3.5, 12.5), p(12.5, 3.5), w);
            line(p(12.5, 3.5), p(8.6, 3.9), w);
            line(p(12.5, 3.5), p(12.1, 7.4), w);
            line(p(3.5, 12.5), p(7.4, 12.1), w);
            line(p(3.5, 12.5), p(3.9, 8.6), w);
        }
        Icon::Polyline => {
            painter.add(Shape::line(
                vec![p(2.5, 12.5), p(6.0, 4.5), p(10.0, 10.5), p(13.5, 3.5)],
                Stroke::new(w, color),
            ));
        }
        Icon::BadgeSquare => {
            painter.rect_stroke(
                Rect::from_min_max(p(2.6, 2.6), p(13.4, 13.4)),
                3.0 * s,
                Stroke::new(w, color),
                StrokeKind::Middle,
            );
            painter.text(
                p(8.0, 8.2),
                Align2::CENTER_CENTER,
                "1",
                FontId::proportional(8.5 * s),
                color,
            );
        }
        Icon::Fill => {
            painter.add(Shape::convex_polygon(
                vec![p(3.4, 12.6), p(12.6, 12.6), p(12.6, 4.6)],
                color,
                Stroke::NONE,
            ));
            painter.rect_stroke(
                Rect::from_min_max(p(3.0, 3.0), p(13.0, 13.0)),
                1.5 * s,
                Stroke::new(w, color),
                StrokeKind::Middle,
            );
        }
        Icon::Undo => {
            arc_arrow(painter, p(8.8, 8.8), 4.7 * s, 60.0, -180.0, w, color);
        }
        Icon::Redo => {
            arc_arrow(painter, p(7.2, 8.8), 4.7 * s, 120.0, 360.0, w, color);
        }
        Icon::Trash => {
            line(p(3.0, 4.3), p(13.0, 4.3), w);
            poly(
                vec![p(6.3, 4.3), p(6.3, 2.6), p(9.7, 2.6), p(9.7, 4.3)],
                w * 0.95,
            );
            painter.add(Shape::closed_line(
                vec![p(4.6, 4.3), p(5.2, 13.6), p(10.8, 13.6), p(11.4, 4.3)],
                Stroke::new(w * 0.95, color),
            ));
            line(p(6.9, 6.8), p(7.1, 11.2), w * 0.85);
            line(p(9.1, 6.8), p(8.9, 11.2), w * 0.85);
        }
        Icon::Pin => {
            // Tilted pushpin: capped head at top-right, collar notches, and a
            // needle pointing to bottom-left (matches the Tabler "pin" glyph).
            poly(
                vec![
                    p(9.7, 2.7),
                    p(7.33, 5.67),
                    p(4.67, 6.67),
                    p(3.67, 7.67),
                    p(8.33, 12.33),
                    p(9.33, 11.33),
                    p(10.33, 8.67),
                    p(13.3, 6.3),
                ],
                w,
            );
            line(p(6.0, 10.0), p(3.0, 13.0), w);
        }
        Icon::Copy => {
            painter.rect_stroke(
                Rect::from_min_max(p(5.5, 2.5), p(13.5, 10.5)),
                1.5 * s,
                Stroke::new(w * 0.95, color),
                StrokeKind::Middle,
            );
            let front = Rect::from_min_max(p(2.5, 5.5), p(10.5, 13.5));
            painter.rect_filled(front, 1.5 * s, bg);
            painter.rect_stroke(front, 1.5 * s, Stroke::new(w * 0.95, color), StrokeKind::Middle);
        }
        Icon::Save => {
            poly(
                vec![
                    p(2.8, 2.8),
                    p(10.8, 2.8),
                    p(13.2, 5.2),
                    p(13.2, 13.2),
                    p(2.8, 13.2),
                ],
                w,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(5.2, 2.8), p(9.8, 6.2)),
                0.0,
                Stroke::new(w * 0.85, color),
                StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(4.8, 9.2), p(11.2, 13.2)),
                0.0,
                Stroke::new(w * 0.85, color),
                StrokeKind::Middle,
            );
        }
        Icon::Close => {
            line(p(4.0, 4.0), p(12.0, 12.0), w * 1.15);
            line(p(12.0, 4.0), p(4.0, 12.0), w * 1.15);
        }
        Icon::Minus => {
            line(p(4.0, 8.0), p(12.0, 8.0), w * 1.2);
        }
        Icon::Plus => {
            line(p(8.0, 4.0), p(8.0, 12.0), w * 1.2);
            line(p(4.0, 8.0), p(12.0, 8.0), w * 1.2);
        }
        Icon::ColorWheel => {
            let center = p(8.0, 8.0);
            let radius = 5.4 * s;
            let segments = 6;
            for (i, &preset) in COLOR_PRESETS.iter().take(segments).enumerate() {
                let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU + 0.06;
                let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU - 0.06;
                let steps = 6;
                let pts: Vec<Pos2> = (0..=steps)
                    .map(|k| {
                        let t = a0 + (a1 - a0) * k as f32 / steps as f32;
                        Pos2::new(center.x + radius * t.cos(), center.y + radius * t.sin())
                    })
                    .collect();
                for pair in pts.windows(2) {
                    painter.line_segment([pair[0], pair[1]], Stroke::new(2.6 * s, preset));
                }
            }
            painter.circle_filled(center, 1.6 * s, Color32::from_white_alpha(220));
        }
        Icon::Grip => {
            // Six-dot drag handle (2 columns x 3 rows).
            for gx in [6.0, 10.0] {
                for gy in [4.0, 8.0, 12.0] {
                    painter.circle_filled(p(gx, gy), 1.15 * s, color);
                }
            }
        }
        Icon::Caret => {
            line(p(5.0, 6.5), p(8.0, 9.5), w * 1.1);
            line(p(8.0, 9.5), p(11.0, 6.5), w * 1.1);
        }
    }
}

/// Curved arrow used by undo/redo: an arc swept from `start_deg` to `end_deg`
/// (screen coordinates, y down) with a two-wing head at the end point.
fn arc_arrow(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    start_deg: f32,
    end_deg: f32,
    width: f32,
    color: Color32,
) {
    let steps = 20;
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let t = (start_deg + (end_deg - start_deg) * i as f32 / steps as f32).to_radians();
            Pos2::new(center.x + radius * t.cos(), center.y + radius * t.sin())
        })
        .collect();
    for pair in pts.windows(2) {
        painter.line_segment([pair[0], pair[1]], Stroke::new(width, color));
    }

    let end = *pts.last().unwrap();
    let t_end = end_deg.to_radians();
    // Tangent along the sweep direction at the end of the arc.
    let dir = if end_deg > start_deg {
        Vec2::new(-t_end.sin(), t_end.cos())
    } else {
        Vec2::new(t_end.sin(), -t_end.cos())
    };
    let base_angle = dir.y.atan2(dir.x);
    let head_len = radius * 0.62;
    for spread in [std::f32::consts::PI - 0.55, -(std::f32::consts::PI - 0.55)] {
        let a = base_angle + spread;
        let wing = Pos2::new(end.x + head_len * a.cos(), end.y + head_len * a.sin());
        painter.line_segment([end, wing], Stroke::new(width, color));
    }
}