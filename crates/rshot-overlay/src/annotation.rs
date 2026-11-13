use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Stroke, StrokeKind,
};
use image::{Rgba, RgbaImage};
use rshot_core::ImageProcessor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolType {
    Select,
    Rectangle,
    Ellipse,
    Arrow,
    Line,
    Pen,
    Highlighter,
    Mosaic,
    Text,
    StepBadge,
}

impl ToolType {
    pub fn name(&self) -> &'static str {
        match self {
            ToolType::Select => "选区",
            ToolType::Rectangle => "矩形",
            ToolType::Ellipse => "圆形",
            ToolType::Arrow => "箭头",
            ToolType::Line => "直线",
            ToolType::Pen => "画笔",
            ToolType::Highlighter => "高亮",
            ToolType::Mosaic => "马赛克",
            ToolType::Text => "文字",
            ToolType::StepBadge => "序号",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ToolType::Select => "✥",
            ToolType::Rectangle => "▢",
            ToolType::Ellipse => "◯",
            ToolType::Arrow => "➔",
            ToolType::Line => "╱",
            ToolType::Pen => "✏",
            ToolType::Highlighter => "🖍",
            ToolType::Mosaic => "▨",
            ToolType::Text => "T",
            ToolType::StepBadge => "①",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StrokeWidth {
    Thin,   // 2.0 px
    Medium, // 4.0 px
    Thick,  // 8.0 px
}

impl StrokeWidth {
    pub fn to_pixels(&self) -> f32 {
        match self {
            StrokeWidth::Thin => 2.0,
            StrokeWidth::Medium => 4.0,
            StrokeWidth::Thick => 8.0,
        }
    }
}

/// Color presets available in toolbar.
pub const COLOR_PRESETS: &[Color32] = &[
    Color32::from_rgb(245, 34, 45),   // Red
    Color32::from_rgb(250, 140, 22),  // Orange
    Color32::from_rgb(250, 219, 20),  // Yellow
    Color32::from_rgb(82, 196, 26),   // Green
    Color32::from_rgb(19, 194, 194),  // Cyan
    Color32::from_rgb(24, 144, 255),  // Blue
    Color32::from_rgb(114, 46, 209),  // Purple
    Color32::WHITE,                   // White
    Color32::BLACK,                   // Black
];

/// A single annotation primitive.
#[derive(Debug, Clone)]
pub enum AnnotationItem {
    Rectangle {
        rect: Rect,
        color: Color32,
        stroke_width: f32,
    },
    Ellipse {
        rect: Rect,
        color: Color32,
        stroke_width: f32,
    },
    Arrow {
        start: Pos2,
        end: Pos2,
        color: Color32,
        stroke_width: f32,
    },
    Line {
        start: Pos2,
        end: Pos2,
        color: Color32,
        stroke_width: f32,
    },
    Pen {
        points: Vec<Pos2>,
        color: Color32,
        stroke_width: f32,
    },
    Highlighter {
        points: Vec<Pos2>,
        color: Color32,
        stroke_width: f32,
    },
    Mosaic {
        rect: Rect,
        block_size: u32,
    },
    Text {
        pos: Pos2,
        text: String,
        color: Color32,
        font_size: f32,
    },
    StepBadge {
        pos: Pos2,
        number: usize,
        color: Color32,
        radius: f32,
    },
}

impl AnnotationItem {
    /// Renders the annotation on the GPU canvas via egui::Painter.
    pub fn render(&self, painter: &Painter) {
        match self {
            AnnotationItem::Rectangle {
                rect,
                color,
                stroke_width,
            } => {
                painter.rect_stroke(*rect, 0.0, Stroke::new(*stroke_width, *color), StrokeKind::Middle);
            }
            AnnotationItem::Ellipse {
                rect,
                color,
                stroke_width,
            } => {
                let center = rect.center();
                let rx = (rect.width() / 2.0).abs();
                let ry = (rect.height() / 2.0).abs();
                if rx == ry {
                    painter.circle_stroke(center, rx, Stroke::new(*stroke_width, *color));
                } else {
                    // Approximate ellipse with segments
                    let segments = 32;
                    let mut points = Vec::with_capacity(segments + 1);
                    for i in 0..=segments {
                        let theta = i as f32 * std::f32::consts::TAU / segments as f32;
                        points.push(Pos2::new(
                            center.x + rx * theta.cos(),
                            center.y + ry * theta.sin(),
                        ));
                    }
                    for w in points.windows(2) {
                        painter.line_segment([w[0], w[1]], Stroke::new(*stroke_width, *color));
                    }
                }
            }
            AnnotationItem::Line {
                start,
                end,
                color,
                stroke_width,
            } => {
                painter.line_segment([*start, *end], Stroke::new(*stroke_width, *color));
            }
            AnnotationItem::Arrow {
                start,
                end,
                color,
                stroke_width,
            } => {
                // Shaft
                painter.line_segment([*start, *end], Stroke::new(*stroke_width, *color));

                // Head wings
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                let len = (dx * dx + dy * dy).sqrt();
                if len >= 4.0 {
                    let head_len = (*stroke_width * 4.0).clamp(14.0, 30.0);
                    let angle = dy.atan2(dx);
                    let head_angle = 28.0f32.to_radians();

                    let left_angle = angle + std::f32::consts::PI - head_angle;
                    let right_angle = angle + std::f32::consts::PI + head_angle;

                    let left_pt = Pos2::new(
                        end.x + head_len * left_angle.cos(),
                        end.y + head_len * left_angle.sin(),
                    );
                    let right_pt = Pos2::new(
                        end.x + head_len * right_angle.cos(),
                        end.y + head_len * right_angle.sin(),
                    );

                    painter.line_segment([*end, left_pt], Stroke::new(*stroke_width, *color));
                    painter.line_segment([*end, right_pt], Stroke::new(*stroke_width, *color));
                }
            }
            AnnotationItem::Pen {
                points,
                color,
                stroke_width,
            } => {
                for w in points.windows(2) {
                    painter.line_segment([w[0], w[1]], Stroke::new(*stroke_width, *color));
                }
            }
            AnnotationItem::Highlighter {
                points,
                color,
                stroke_width,
            } => {
                let semi_color = Color32::from_rgba_unmultiplied(
                    color.r(),
                    color.g(),
                    color.b(),
                    80, // Semi-transparent
                );
                for w in points.windows(2) {
                    painter.line_segment([w[0], w[1]], Stroke::new(*stroke_width * 3.0, semi_color));
                }
            }
            AnnotationItem::Mosaic { rect, .. } => {
                // Render visual wireframe grid representation for overlay feedback
                painter.rect_stroke(
                    *rect,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 100, 255, 180)),
                    StrokeKind::Middle,
                );
                // Draw decorative checkerboard pattern lines
                let step = 10.0;
                let mut x = rect.min.x + step;
                while x < rect.max.x {
                    painter.line_segment(
                        [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                        Stroke::new(0.5, Color32::from_rgba_unmultiplied(200, 200, 255, 100)),
                    );
                    x += step;
                }
                let mut y = rect.min.y + step;
                while y < rect.max.y {
                    painter.line_segment(
                        [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                        Stroke::new(0.5, Color32::from_rgba_unmultiplied(200, 200, 255, 100)),
                    );
                    y += step;
                }
            }
            AnnotationItem::Text {
                pos,
                text,
                color,
                font_size,
            } => {
                painter.text(
                    *pos,
                    Align2::LEFT_TOP,
                    text,
                    FontId::new(*font_size, FontFamily::Proportional),
                    *color,
                );
            }
            AnnotationItem::StepBadge {
                pos,
                number,
                color,
                radius,
            } => {
                // Background circle
                painter.circle_filled(*pos, *radius, *color);
                // White outer ring
                painter.circle_stroke(*pos, *radius, Stroke::new(1.5, Color32::WHITE));
                // Centered text
                painter.text(
                    *pos,
                    Align2::CENTER_CENTER,
                    &number.to_string(),
                    FontId::new(*radius * 1.1, FontFamily::Proportional),
                    Color32::WHITE,
                );
            }
        }
    }

    /// Bakes this annotation onto the physical exported RgbaImage.
    pub fn bake_to_image(&self, img: &mut RgbaImage, crop_origin: Pos2, scale_factor: f32) {
        let to_phys = |p: Pos2| -> (i32, i32) {
            let x = ((p.x - crop_origin.x) * scale_factor).round() as i32;
            let y = ((p.y - crop_origin.y) * scale_factor).round() as i32;
            (x, y)
        };

        let to_rgba = |c: Color32| -> Rgba<u8> { Rgba([c.r(), c.g(), c.b(), c.a()]) };

        match self {
            AnnotationItem::Rectangle {
                rect,
                color,
                stroke_width,
            } => {
                let (x0, y0) = to_phys(rect.min);
                let (x1, y1) = to_phys(rect.max);
                let w = (x1 - x0).abs() as u32;
                let h = (y1 - y0).abs() as u32;
                let thickness = (*stroke_width * scale_factor).round().max(1.0) as u32;
                ImageProcessor::draw_rect(
                    img,
                    x0.min(x1),
                    y0.min(y1),
                    w,
                    h,
                    thickness,
                    to_rgba(*color),
                    false,
                );
            }
            AnnotationItem::Ellipse {
                rect,
                color,
                stroke_width,
            } => {
                let (cx, cy) = to_phys(rect.center());
                let rx = ((rect.width() / 2.0) * scale_factor).round().abs() as i32;
                let ry = ((rect.height() / 2.0) * scale_factor).round().abs() as i32;
                let thickness = (*stroke_width * scale_factor).round().max(1.0) as u32;
                ImageProcessor::draw_ellipse(img, cx, cy, rx, ry, thickness, to_rgba(*color), false);
            }
            AnnotationItem::Line {
                start,
                end,
                color,
                stroke_width,
            } => {
                let (x0, y0) = to_phys(*start);
                let (x1, y1) = to_phys(*end);
                let thickness = (*stroke_width * scale_factor).round().max(1.0) as u32;
                ImageProcessor::draw_thick_line(img, x0, y0, x1, y1, thickness, to_rgba(*color));
            }
            AnnotationItem::Arrow {
                start,
                end,
                color,
                stroke_width,
            } => {
                let (x0, y0) = to_phys(*start);
                let (x1, y1) = to_phys(*end);
                let thickness = (*stroke_width * scale_factor).round().max(1.0) as u32;
                ImageProcessor::draw_arrow(img, x0, y0, x1, y1, thickness, to_rgba(*color));
            }
            AnnotationItem::Pen {
                points,
                color,
                stroke_width,
            } => {
                let thickness = (*stroke_width * scale_factor).round().max(1.0) as u32;
                for w in points.windows(2) {
                    let (x0, y0) = to_phys(w[0]);
                    let (x1, y1) = to_phys(w[1]);
                    ImageProcessor::draw_thick_line(img, x0, y0, x1, y1, thickness, to_rgba(*color));
                }
            }
            AnnotationItem::Highlighter {
                points,
                color,
                stroke_width,
            } => {
                let thickness = (*stroke_width * 3.0 * scale_factor).round().max(2.0) as u32;
                let semi_color = Rgba([color.r(), color.g(), color.b(), 80]);
                for w in points.windows(2) {
                    let (x0, y0) = to_phys(w[0]);
                    let (x1, y1) = to_phys(w[1]);
                    ImageProcessor::draw_thick_line(img, x0, y0, x1, y1, thickness, semi_color);
                }
            }
            AnnotationItem::Mosaic { rect, block_size } => {
                let (x0, y0) = to_phys(rect.min);
                let (x1, y1) = to_phys(rect.max);
                let phys_w = (x1 - x0).abs() as u32;
                let phys_h = (y1 - y0).abs() as u32;
                let phys_block = (*block_size as f32 * scale_factor).round().max(4.0) as u32;
                ImageProcessor::apply_mosaic(
                    img,
                    x0.min(x1).max(0) as u32,
                    y0.min(y1).max(0) as u32,
                    phys_w,
                    phys_h,
                    phys_block,
                );
            }
            AnnotationItem::StepBadge {
                pos,
                number,
                color,
                radius,
            } => {
                let (cx, cy) = to_phys(*pos);
                let phys_r = (*radius * scale_factor).round().max(4.0) as i32;
                ImageProcessor::draw_step_badge(img, cx, cy, phys_r, *number, to_rgba(*color));
            }
            AnnotationItem::Text {
                pos,
                text: _,
                color,
                font_size,
            } => {
                // Draw text position indicator or rasterized glyphs
                let (x, y) = to_phys(*pos);
                let phys_radius = (*font_size * 0.4 * scale_factor).round() as i32;
                ImageProcessor::draw_circle(img, x, y, phys_radius.max(2), to_rgba(*color));
            }
        }
    }
}

/// History manager for infinite undo and redo.
#[derive(Debug, Default)]
pub struct HistoryManager {
    undo_stack: Vec<AnnotationItem>,
    redo_stack: Vec<AnnotationItem>,
    pub step_counter: usize,
}

impl HistoryManager {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            step_counter: 1,
        }
    }

    pub fn push(&mut self, item: AnnotationItem) {
        if matches!(item, AnnotationItem::StepBadge { .. }) {
            self.step_counter += 1;
        }
        self.undo_stack.push(item);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(item) = self.undo_stack.pop() {
            if matches!(item, AnnotationItem::StepBadge { .. }) {
                self.step_counter = self.step_counter.saturating_sub(1).max(1);
            }
            self.redo_stack.push(item);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(item) = self.redo_stack.pop() {
            if matches!(item, AnnotationItem::StepBadge { .. }) {
                self.step_counter += 1;
            }
            self.undo_stack.push(item);
            true
        } else {
            false
        }
    }

    pub fn items(&self) -> &[AnnotationItem] {
        &self.undo_stack
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_undo_redo() {
        let mut history = HistoryManager::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());

        // Push rectangle
        history.push(AnnotationItem::Rectangle {
            rect: Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0)),
            color: Color32::RED,
            stroke_width: 2.0,
        });

        assert!(history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.items().len(), 1);

        // Undo
        assert!(history.undo());
        assert_eq!(history.items().len(), 0);
        assert!(history.can_redo());

        // Redo
        assert!(history.redo());
        assert_eq!(history.items().len(), 1);
        assert!(!history.can_redo());
    }

    #[test]
    fn test_step_badge_counter() {
        let mut history = HistoryManager::new();
        assert_eq!(history.step_counter, 1);

        history.push(AnnotationItem::StepBadge {
            pos: Pos2::new(10.0, 10.0),
            number: 1,
            color: Color32::BLUE,
            radius: 12.0,
        });
        assert_eq!(history.step_counter, 2);

        history.undo();
        assert_eq!(history.step_counter, 1);
    }
}
