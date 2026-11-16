use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2,
};
use image::{Rgba, RgbaImage};
use rshot_core::ImageProcessor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolType {
    Select,
    Rectangle,
    RoundedRect,
    Ellipse,
    Diamond,
    Polygon,
    Arrow,
    DoubleArrow,
    Line,
    Polyline,
    Pen,
    Highlighter,
    Mosaic,
    Blur,
    Text,
    StepBadge,
}

/// Top-level toolbar groups. Each group (except Select) opens a popover whose
/// first row offers the group's shape/form variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolFamily {
    Select,
    Shapes,
    Lines,
    Brush,
    Mosaic,
    Blur,
    Text,
    Badge,
}

pub const FAMILIES: [ToolFamily; 8] = [
    ToolFamily::Select,
    ToolFamily::Shapes,
    ToolFamily::Lines,
    ToolFamily::Brush,
    ToolFamily::Mosaic,
    ToolFamily::Blur,
    ToolFamily::Text,
    ToolFamily::Badge,
];

impl ToolFamily {
    pub fn name(&self) -> &'static str {
        match self {
            ToolFamily::Select => "选区",
            ToolFamily::Shapes => "形状",
            ToolFamily::Lines => "线条",
            ToolFamily::Brush => "画笔",
            ToolFamily::Mosaic => "马赛克",
            ToolFamily::Blur => "模糊",
            ToolFamily::Text => "文字",
            ToolFamily::Badge => "序号",
        }
    }

    pub fn variants(&self) -> &'static [ToolType] {
        match self {
            ToolFamily::Select => &[ToolType::Select],
            ToolFamily::Shapes => &[
                ToolType::Rectangle,
                ToolType::RoundedRect,
                ToolType::Ellipse,
                ToolType::Diamond,
                ToolType::Polygon,
            ],
            ToolFamily::Lines => &[
                ToolType::Line,
                ToolType::Arrow,
                ToolType::DoubleArrow,
                ToolType::Polyline,
            ],
            ToolFamily::Brush => &[ToolType::Pen, ToolType::Highlighter],
            ToolFamily::Mosaic => &[ToolType::Mosaic],
            ToolFamily::Blur => &[ToolType::Blur],
            ToolFamily::Text => &[ToolType::Text],
            ToolFamily::Badge => &[ToolType::StepBadge],
        }
    }

    pub fn default_tool(&self) -> ToolType {
        self.variants()[0]
    }

    pub fn index(&self) -> usize {
        FAMILIES.iter().position(|f| f == self).unwrap()
    }
}

impl ToolType {
    pub fn name(&self) -> &'static str {
        match self {
            ToolType::Select => "选区",
            ToolType::Rectangle => "矩形",
            ToolType::RoundedRect => "圆角矩形",
            ToolType::Ellipse => "椭圆",
            ToolType::Diamond => "菱形",
            ToolType::Polygon => "不规则图形",
            ToolType::Arrow => "箭头",
            ToolType::DoubleArrow => "双向箭头",
            ToolType::Line => "直线",
            ToolType::Polyline => "折线",
            ToolType::Pen => "画笔",
            ToolType::Highlighter => "高亮",
            ToolType::Mosaic => "马赛克",
            ToolType::Blur => "模糊",
            ToolType::Text => "文字",
            ToolType::StepBadge => "序号",
        }
    }

    pub fn hotkey(&self) -> &'static str {
        match self {
            ToolType::Select => "V",
            ToolType::Rectangle => "R",
            ToolType::RoundedRect => "U",
            ToolType::Ellipse => "O",
            ToolType::Diamond => "G",
            ToolType::Polygon => "Y",
            ToolType::Arrow => "A",
            ToolType::DoubleArrow => "J",
            ToolType::Line => "L",
            ToolType::Polyline => "K",
            ToolType::Pen => "D",
            ToolType::Highlighter => "H",
            ToolType::Mosaic => "M",
            ToolType::Blur => "B",
            ToolType::Text => "T",
            ToolType::StepBadge => "N",
        }
    }

    pub fn can_fill(&self) -> bool {
        matches!(
            self,
            ToolType::Rectangle | ToolType::RoundedRect | ToolType::Ellipse | ToolType::Diamond | ToolType::Polygon
        )
    }

    pub fn family(&self) -> ToolFamily {
        match self {
            ToolType::Select => ToolFamily::Select,
            ToolType::Rectangle | ToolType::RoundedRect | ToolType::Ellipse | ToolType::Diamond | ToolType::Polygon => {
                ToolFamily::Shapes
            }
            ToolType::Arrow | ToolType::DoubleArrow | ToolType::Line | ToolType::Polyline => ToolFamily::Lines,
            ToolType::Pen | ToolType::Highlighter => ToolFamily::Brush,
            ToolType::Mosaic => ToolFamily::Mosaic,
            ToolType::Blur => ToolFamily::Blur,
            ToolType::Text => ToolFamily::Text,
            ToolType::StepBadge => ToolFamily::Badge,
        }
    }

    /// Multi-click tools accumulate vertices instead of dragging.
    pub fn is_multi_point(&self) -> bool {
        matches!(self, ToolType::Polygon | ToolType::Polyline)
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

    /// Nearest preset for a pixel width (used when resuming an existing stroke).
    pub fn from_pixels(px: f32) -> Self {
        if px <= 3.0 {
            StrokeWidth::Thin
        } else if px <= 6.0 {
            StrokeWidth::Medium
        } else {
            StrokeWidth::Thick
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
        filled: bool,
    },
    Ellipse {
        rect: Rect,
        color: Color32,
        stroke_width: f32,
        filled: bool,
    },
    RoundedRect {
        rect: Rect,
        color: Color32,
        stroke_width: f32,
        filled: bool,
    },
    Diamond {
        rect: Rect,
        color: Color32,
        stroke_width: f32,
        filled: bool,
    },
    Polygon {
        points: Vec<Pos2>,
        color: Color32,
        stroke_width: f32,
        filled: bool,
    },
    Arrow {
        start: Pos2,
        end: Pos2,
        color: Color32,
        stroke_width: f32,
    },
    DoubleArrow {
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
    Polyline {
        points: Vec<Pos2>,
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
    Blur {
        rect: Rect,
        radius: u32,
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
        square: bool,
    },
    /// An annotation turned by `angle` radians around `center` (edit-mode rotation).
    Rotated {
        item: Box<AnnotationItem>,
        angle: f32,
        center: Pos2,
    },
}

/// Corner radius for rounded rectangles, proportional to the shorter side.
pub fn corner_radius(rect: Rect) -> f32 {
    (rect.width().min(rect.height()) * 0.2).clamp(2.0, 28.0)
}

/// Diamond vertices (top, right, bottom, left) of a rect.
fn diamond_points(rect: Rect) -> Vec<Pos2> {
    let c = rect.center();
    vec![
        Pos2::new(c.x, rect.top()),
        Pos2::new(rect.right(), c.y),
        Pos2::new(c.x, rect.bottom()),
        Pos2::new(rect.left(), c.y),
    ]
}

/// Two wing strokes forming an arrow head at `tip`, pointing away from `from`.
fn arrow_head(painter: &Painter, from: Pos2, tip: Pos2, stroke_width: f32, color: Color32) {
    let dx = tip.x - from.x;
    let dy = tip.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 4.0 {
        return;
    }
    let head_len = (stroke_width * 4.0).clamp(14.0, 30.0);
    let angle = dy.atan2(dx);
    let head_angle = 28.0f32.to_radians();
    for side in [std::f32::consts::PI - head_angle, std::f32::consts::PI + head_angle] {
        let a = angle + side;
        let wing = Pos2::new(tip.x + head_len * a.cos(), tip.y + head_len * a.sin());
        painter.line_segment([tip, wing], Stroke::new(stroke_width, color));
    }
}

/// Rotates `p` around `center` by the angle whose sin/cos are given.
fn rot_pos(p: Pos2, center: Pos2, sin: f32, cos: f32) -> Pos2 {
    let d = p - center;
    center + Vec2::new(cos * d.x - sin * d.y, sin * d.x + cos * d.y)
}

fn path_shape(points: Vec<Pos2>, closed: bool, fill: Color32, stroke: Stroke) -> egui::Shape {
    egui::Shape::Path(egui::epaint::PathShape {
        points,
        closed,
        fill,
        stroke: stroke.into(),
    })
}

/// Outline of a rounded rectangle sampled as a polygon (for rotated rendering).
fn rounded_rect_points(rect: Rect, radius: f32) -> Vec<Pos2> {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let mut pts = Vec::new();
    let corners = [
        (Pos2::new(rect.right() - r, rect.top() + r), -std::f32::consts::FRAC_PI_2, 0.0),
        (Pos2::new(rect.right() - r, rect.bottom() - r), 0.0, std::f32::consts::FRAC_PI_2),
        (Pos2::new(rect.left() + r, rect.bottom() - r), std::f32::consts::FRAC_PI_2, std::f32::consts::PI),
        (Pos2::new(rect.left() + r, rect.top() + r), std::f32::consts::PI, std::f32::consts::PI * 1.5),
    ];
    for (c, a0, a1) in corners {
        for i in 0..=6 {
            let a = a0 + (a1 - a0) * i as f32 / 6.0;
            pts.push(Pos2::new(c.x + r * a.cos(), c.y + r * a.sin()));
        }
    }
    pts
}

fn ellipse_points(rect: Rect) -> Vec<Pos2> {
    let c = rect.center();
    let (rx, ry) = (rect.width() / 2.0, rect.height() / 2.0);
    (0..48)
        .map(|i| {
            let a = i as f32 / 48.0 * std::f32::consts::TAU;
            Pos2::new(c.x + rx * a.cos(), c.y + ry * a.sin())
        })
        .collect()
}

/// Rough text extent used for hit testing and rotation bounds (CJK glyphs are square).
pub fn text_size_est(text: &str, font_size: f32) -> Vec2 {
    let w: f32 = text
        .chars()
        .map(|ch| if ch as u32 > 0x2e80 { font_size } else { font_size * 0.55 })
        .sum();
    Vec2::new(w.max(font_size * 0.5), font_size * 1.25)
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    if len2 < 1e-6 {
        return p.distance(a);
    }
    let t = (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn point_in_polygon(p: Pos2, pts: &[Pos2]) -> bool {
    let mut inside = false;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        if (pts[i].y > p.y) != (pts[j].y > p.y) {
            let x = pts[j].x + (p.y - pts[j].y) / (pts[i].y - pts[j].y) * (pts[i].x - pts[j].x);
            if p.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

fn points_bounds(pts: &[Pos2]) -> Rect {
    let mut r = Rect::from_min_max(pts[0], pts[0]);
    for p in pts {
        r.extend_with(*p);
    }
    r
}

/// Bilinear sample of a premultiplied RGBA image at fractional pixel coords.
fn sample_premul(img: &RgbaImage, x: f32, y: f32) -> Option<[f32; 4]> {
    let (w, h) = (img.width() as f32, img.height() as f32);
    if x < 0.0 || y < 0.0 || x > w - 1.0 || y > h - 1.0 {
        return None;
    }
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(img.width() - 1);
    let y1 = (y0 + 1).min(img.height() - 1);
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let mut out = [0.0f32; 4];
    for (c, o) in out.iter_mut().enumerate() {
        let p00 = img.get_pixel(x0, y0)[c] as f32;
        let p10 = img.get_pixel(x1, y0)[c] as f32;
        let p01 = img.get_pixel(x0, y1)[c] as f32;
        let p11 = img.get_pixel(x1, y1)[c] as f32;
        *o = (p00 * (1.0 - fx) + p10 * fx) * (1.0 - fy) + (p01 * (1.0 - fx) + p11 * fx) * fy;
    }
    Some(out)
}

/// Selection gizmo: bounding box, corner dots and a rotation handle above it.
pub fn paint_gizmo(painter: &Painter, bounds: Rect, handle: Pos2, handle_hot: bool) {
    let accent = Color32::from_rgb(24, 144, 255);
    painter.rect_stroke(bounds, 2.0, Stroke::new(1.5, accent), StrokeKind::Outside);
    for c in [bounds.left_top(), bounds.right_top(), bounds.left_bottom(), bounds.right_bottom()] {
        painter.rect_filled(Rect::from_center_size(c, Vec2::splat(6.0)), 1.0, Color32::WHITE);
        painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(6.0)), 1.0, Stroke::new(1.0, accent), StrokeKind::Inside);
    }
    painter.line_segment([bounds.center_top(), handle], Stroke::new(1.0, accent));
    let (fill, ring) = if handle_hot { (accent, Color32::WHITE) } else { (Color32::WHITE, accent) };
    painter.circle_filled(handle, 5.0, fill);
    painter.circle_stroke(handle, 5.0, Stroke::new(1.5, ring));
}

/// Position of the rotation handle for a selected annotation.
pub fn rotation_handle_pos(bounds: Rect) -> Pos2 {
    Pos2::new(bounds.center().x, bounds.top() - 26.0)
}

/// Gizmo for a selected polyline/polygon: bounding box, rotation handle and a
/// draggable handle on every vertex. The active vertex is filled with accent.
pub fn paint_multi_gizmo(
    painter: &Painter,
    bounds: Rect,
    handle: Pos2,
    handle_hot: bool,
    vertices: &[Pos2],
    active: Option<usize>,
    hover: Option<usize>,
) {
    let accent = Color32::from_rgb(24, 144, 255);
    painter.rect_stroke(bounds, 2.0, Stroke::new(1.0, accent.gamma_multiply(0.65)), StrokeKind::Outside);
    painter.line_segment([bounds.center_top(), handle], Stroke::new(1.0, accent));
    let (fill, ring) = if handle_hot { (accent, Color32::WHITE) } else { (Color32::WHITE, accent) };
    painter.circle_filled(handle, 5.0, fill);
    painter.circle_stroke(handle, 5.0, Stroke::new(1.5, ring));

    for (i, v) in vertices.iter().enumerate() {
        let is_active = active == Some(i);
        let is_hover = hover == Some(i);
        let r = if is_active { 6.5 } else if is_hover { 6.0 } else { 5.0 };
        let (fill, ring) = if is_active { (accent, Color32::WHITE) } else { (Color32::WHITE, accent) };
        painter.circle_filled(*v, r, fill);
        painter.circle_stroke(*v, r, Stroke::new(1.5, ring));
        if is_active {
            painter.circle_stroke(*v, r + 3.0, Stroke::new(1.0, accent.gamma_multiply(0.6)));
        }
    }
}

impl AnnotationItem {
    /// Axis-aligned extent in logical coordinates, including stroke and arrow heads.
    pub fn bounds(&self) -> Rect {
        match self {
            AnnotationItem::Rectangle { rect, stroke_width, .. }
            | AnnotationItem::RoundedRect { rect, stroke_width, .. }
            | AnnotationItem::Ellipse { rect, stroke_width, .. }
            | AnnotationItem::Diamond { rect, stroke_width, .. } => rect.expand(stroke_width / 2.0 + 1.0),
            AnnotationItem::Mosaic { rect, .. } | AnnotationItem::Blur { rect, .. } => rect.expand(1.0),
            AnnotationItem::Arrow { start, end, stroke_width, .. }
            | AnnotationItem::DoubleArrow { start, end, stroke_width, .. }
            | AnnotationItem::Line { start, end, stroke_width, .. } => {
                let head = (stroke_width * 4.0).clamp(14.0, 30.0);
                Rect::from_two_pos(*start, *end).expand(stroke_width / 2.0 + head)
            }
            AnnotationItem::Pen { points, stroke_width, .. } | AnnotationItem::Polyline { points, stroke_width, .. } => {
                points_bounds(points).expand(stroke_width / 2.0 + 1.0)
            }
            AnnotationItem::Highlighter { points, stroke_width, .. } => {
                points_bounds(points).expand(stroke_width * 1.5 + 1.0)
            }
            AnnotationItem::Polygon { points, stroke_width, .. } => {
                points_bounds(points).expand(stroke_width / 2.0 + 1.0)
            }
            AnnotationItem::Text { pos, text, font_size, .. } => {
                Rect::from_min_size(*pos, text_size_est(text, *font_size)).expand(2.0)
            }
            AnnotationItem::StepBadge { pos, radius, .. } => {
                Rect::from_center_size(*pos, Vec2::splat(radius * 2.0 + 4.0))
            }
            AnnotationItem::Rotated { item, angle, center } => {
                let (sin, cos) = angle.sin_cos();
                let b = item.bounds();
                let corners = [b.left_top(), b.right_top(), b.left_bottom(), b.right_bottom()];
                let mut r = Rect::from_min_max(
                    rot_pos(corners[0], *center, sin, cos),
                    rot_pos(corners[0], *center, sin, cos),
                );
                for c in corners {
                    r.extend_with(rot_pos(c, *center, sin, cos));
                }
                r
            }
        }
    }

    /// Shifts the whole annotation by `delta` (edit-mode move).
    pub fn translate(&mut self, delta: Vec2) {
        match self {
            AnnotationItem::Rectangle { rect, .. }
            | AnnotationItem::RoundedRect { rect, .. }
            | AnnotationItem::Ellipse { rect, .. }
            | AnnotationItem::Diamond { rect, .. }
            | AnnotationItem::Mosaic { rect, .. }
            | AnnotationItem::Blur { rect, .. } => *rect = rect.translate(delta),
            AnnotationItem::Arrow { start, end, .. }
            | AnnotationItem::DoubleArrow { start, end, .. }
            | AnnotationItem::Line { start, end, .. } => {
                *start += delta;
                *end += delta;
            }
            AnnotationItem::Pen { points, .. }
            | AnnotationItem::Highlighter { points, .. }
            | AnnotationItem::Polygon { points, .. }
            | AnnotationItem::Polyline { points, .. } => {
                for p in points.iter_mut() {
                    *p += delta;
                }
            }
            AnnotationItem::Text { pos, .. } | AnnotationItem::StepBadge { pos, .. } => *pos += delta,
            AnnotationItem::Rotated { item, center, .. } => {
                item.translate(delta);
                *center += delta;
            }
        }
    }

    /// Whether `p` grabs this annotation; `tol` is extra grab tolerance in points.
    pub fn hit_test(&self, p: Pos2, tol: f32) -> bool {
        match self {
            AnnotationItem::Rectangle { rect, stroke_width, filled, .. }
            | AnnotationItem::RoundedRect { rect, stroke_width, filled, .. }
            | AnnotationItem::Ellipse { rect, stroke_width, filled, .. }
            | AnnotationItem::Diamond { rect, stroke_width, filled, .. } => {
                if *filled {
                    rect.expand(tol).contains(p)
                } else {
                    let outer = rect.expand(stroke_width / 2.0 + tol);
                    let inner = rect.shrink(stroke_width / 2.0 + tol);
                    outer.contains(p) && !(inner.width() > 0.0 && inner.height() > 0.0 && inner.contains(p))
                }
            }
            AnnotationItem::Mosaic { rect, .. } | AnnotationItem::Blur { rect, .. } => rect.expand(tol).contains(p),
            AnnotationItem::Arrow { start, end, stroke_width, .. }
            | AnnotationItem::DoubleArrow { start, end, stroke_width, .. }
            | AnnotationItem::Line { start, end, stroke_width, .. } => {
                dist_to_segment(p, *start, *end) <= stroke_width / 2.0 + tol
            }
            AnnotationItem::Pen { points, stroke_width, .. } | AnnotationItem::Polyline { points, stroke_width, .. } => {
                points.windows(2).any(|w| dist_to_segment(p, w[0], w[1]) <= stroke_width / 2.0 + tol)
            }
            AnnotationItem::Highlighter { points, stroke_width, .. } => {
                points.windows(2).any(|w| dist_to_segment(p, w[0], w[1]) <= stroke_width * 1.5 + tol)
            }
            AnnotationItem::Polygon { points, stroke_width, filled, .. } => {
                if *filled && point_in_polygon(p, points) {
                    return true;
                }
                let n = points.len();
                (0..n).any(|i| dist_to_segment(p, points[i], points[(i + 1) % n]) <= stroke_width / 2.0 + tol)
            }
            AnnotationItem::Text { pos, text, font_size, .. } => {
                Rect::from_min_size(*pos, text_size_est(text, *font_size)).expand(tol).contains(p)
            }
            AnnotationItem::StepBadge { pos, radius, .. } => p.distance(*pos) <= radius + tol,
            AnnotationItem::Rotated { item, angle, center } => {
                let (sin, cos) = (-angle).sin_cos();
                item.hit_test(rot_pos(p, *center, sin, cos), tol)
            }
        }
    }

    /// Current edit-mode rotation angle in radians (0 when never rotated).
    pub fn rotation(&self) -> f32 {
        match self {
            AnnotationItem::Rotated { angle, .. } => *angle,
            _ => 0.0,
        }
    }

    /// The multi-point tool this item belongs to, unwrapping any rotation.
    pub fn multi_point_kind(&self) -> Option<ToolType> {
        self.multi_point_parts().map(|(kind, ..)| kind)
    }

    /// On-screen vertex positions (rotation applied) for vertex editing.
    pub fn editable_vertices(&self) -> Option<Vec<Pos2>> {
        match self {
            AnnotationItem::Rotated { item, angle, center } => {
                let inner = item.editable_vertices()?;
                let (sin, cos) = angle.sin_cos();
                Some(inner.into_iter().map(|p| rot_pos(p, *center, sin, cos)).collect())
            }
            _ => self.multi_point_parts().map(|(_, pts, ..)| pts.to_vec()),
        }
    }

    /// Moves vertex `v` to an on-screen position (inverse-rotated when wrapped).
    pub fn move_vertex(&mut self, v: usize, pos: Pos2) {
        match self {
            AnnotationItem::Rotated { item, angle, center } => {
                let (sin, cos) = (-*angle).sin_cos();
                item.move_vertex(v, rot_pos(pos, *center, sin, cos));
            }
            AnnotationItem::Polyline { points, .. } | AnnotationItem::Polygon { points, .. } => {
                if let Some(p) = points.get_mut(v) {
                    *p = pos;
                }
            }
            _ => {}
        }
    }

    /// Removes vertex `v`, keeping the minimum that defines the shape
    /// (2 for a polyline, 3 for a polygon). Returns whether it removed one.
    pub fn remove_vertex(&mut self, v: usize) -> bool {
        match self {
            AnnotationItem::Rotated { item, .. } => item.remove_vertex(v),
            AnnotationItem::Polyline { points, .. } => {
                if points.len() > 2 && v < points.len() {
                    points.remove(v);
                    true
                } else {
                    false
                }
            }
            AnnotationItem::Polygon { points, .. } => {
                if points.len() > 3 && v < points.len() {
                    points.remove(v);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Color, stroke width and fill of a multi-point item (for resuming it).
    pub fn multi_point_style(&self) -> Option<(Color32, f32, bool)> {
        self.multi_point_parts().map(|(_, _, color, stroke, filled)| (color, stroke, filled))
    }

    fn multi_point_parts(&self) -> Option<(ToolType, &[Pos2], Color32, f32, bool)> {
        match self {
            AnnotationItem::Polyline { points, color, stroke_width } => {
                Some((ToolType::Polyline, points, *color, *stroke_width, false))
            }
            AnnotationItem::Polygon { points, color, stroke_width, filled } => {
                Some((ToolType::Polygon, points, *color, *stroke_width, *filled))
            }
            AnnotationItem::Rotated { item, .. } => item.multi_point_parts(),
            _ => None,
        }
    }

    /// Renders the annotation on the GPU canvas via egui::Painter.
    pub fn render(&self, painter: &Painter) {
        match self {
            AnnotationItem::Rectangle {
                rect,
                color,
                stroke_width,
                filled,
            } => {
                if *filled {
                    painter.rect_filled(*rect, 0.0, *color);
                }
                painter.rect_stroke(*rect, 0.0, Stroke::new(*stroke_width, *color), StrokeKind::Middle);
            }
            AnnotationItem::Ellipse {
                rect,
                color,
                stroke_width,
                filled,
            } => {
                let fill = if *filled { *color } else { Color32::TRANSPARENT };
                painter.add(egui::Shape::Ellipse(egui::epaint::EllipseShape {
                    center: rect.center(),
                    radius: rect.size() / 2.0,
                    fill,
                    stroke: Stroke::new(*stroke_width, *color),
                }));
            }
            AnnotationItem::RoundedRect {
                rect,
                color,
                stroke_width,
                filled,
            } => {
                let r = corner_radius(*rect);
                if *filled {
                    painter.rect_filled(*rect, r, *color);
                }
                painter.rect_stroke(*rect, r, Stroke::new(*stroke_width, *color), StrokeKind::Middle);
            }
            AnnotationItem::Diamond {
                rect,
                color,
                stroke_width,
                filled,
            } => {
                let pts = diamond_points(*rect);
                if *filled {
                    painter.add(egui::Shape::convex_polygon(pts.clone(), *color, Stroke::NONE));
                }
                painter.add(egui::Shape::closed_line(pts, Stroke::new(*stroke_width, *color)));
            }
            AnnotationItem::Polygon {
                points,
                color,
                stroke_width,
                filled,
            } => {
                let fill = if *filled { *color } else { Color32::TRANSPARENT };
                painter.add(egui::Shape::Path(egui::epaint::PathShape {
                    points: points.clone(),
                    closed: true,
                    fill,
                    stroke: Stroke::new(*stroke_width, *color).into(),
                }));
            }
            AnnotationItem::Line {
                start,
                end,
                color,
                stroke_width,
            } => {
                painter.line_segment([*start, *end], Stroke::new(*stroke_width, *color));
            }
            AnnotationItem::Polyline {
                points,
                color,
                stroke_width,
            } => {
                painter.add(egui::Shape::Path(egui::epaint::PathShape {
                    points: points.clone(),
                    closed: false,
                    fill: Color32::TRANSPARENT,
                    stroke: Stroke::new(*stroke_width, *color).into(),
                }));
            }
            AnnotationItem::DoubleArrow {
                start,
                end,
                color,
                stroke_width,
            } => {
                painter.line_segment([*start, *end], Stroke::new(*stroke_width, *color));
                arrow_head(painter, *start, *end, *stroke_width, *color);
                arrow_head(painter, *end, *start, *stroke_width, *color);
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
            AnnotationItem::Blur { rect, .. } => {
                // Frosted preview while dragging; the real blur is baked on export.
                painter.rect_filled(*rect, 0.0, Color32::from_white_alpha(36));
                painter.rect_stroke(
                    *rect,
                    0.0,
                    Stroke::new(1.0, Color32::from_white_alpha(140)),
                    StrokeKind::Middle,
                );
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
                square,
            } => {
                if *square {
                    let r = Rect::from_center_size(*pos, Vec2::splat(*radius * 2.0));
                    painter.rect_filled(r, radius * 0.3, *color);
                    painter.rect_stroke(r, radius * 0.3, Stroke::new(1.5, Color32::WHITE), StrokeKind::Inside);
                } else {
                    painter.circle_filled(*pos, *radius, *color);
                    painter.circle_stroke(*pos, *radius, Stroke::new(1.5, Color32::WHITE));
                }
                painter.text(
                    *pos,
                    Align2::CENTER_CENTER,
                    &number.to_string(),
                    FontId::new(*radius * 1.1, FontFamily::Proportional),
                    Color32::WHITE,
                );
            }
            AnnotationItem::Rotated { item, angle, center } => {
                let (sin, cos) = angle.sin_cos();
                let rp = |p: Pos2| rot_pos(p, *center, sin, cos);
                let stroke_of = |sw: f32, c: Color32| Stroke::new(sw, c);
                match item.as_ref() {
                    AnnotationItem::Rectangle { rect, color, stroke_width, filled } => {
                        let pts = vec![rp(rect.left_top()), rp(rect.right_top()), rp(rect.right_bottom()), rp(rect.left_bottom())];
                        let fill = if *filled { *color } else { Color32::TRANSPARENT };
                        painter.add(path_shape(pts, true, fill, stroke_of(*stroke_width, *color)));
                    }
                    AnnotationItem::RoundedRect { rect, color, stroke_width, filled } => {
                        let pts = rounded_rect_points(*rect, corner_radius(*rect)).into_iter().map(rp).collect();
                        let fill = if *filled { *color } else { Color32::TRANSPARENT };
                        painter.add(path_shape(pts, true, fill, stroke_of(*stroke_width, *color)));
                    }
                    AnnotationItem::Ellipse { rect, color, stroke_width, filled } => {
                        let pts = ellipse_points(*rect).into_iter().map(rp).collect();
                        let fill = if *filled { *color } else { Color32::TRANSPARENT };
                        painter.add(path_shape(pts, true, fill, stroke_of(*stroke_width, *color)));
                    }
                    AnnotationItem::Diamond { rect, color, stroke_width, filled } => {
                        let pts: Vec<Pos2> = diamond_points(*rect).into_iter().map(rp).collect();
                        let fill = if *filled { *color } else { Color32::TRANSPARENT };
                        painter.add(path_shape(pts, true, fill, stroke_of(*stroke_width, *color)));
                    }
                    AnnotationItem::Polygon { points, color, stroke_width, filled } => {
                        let pts = points.iter().map(|p| rp(*p)).collect();
                        let fill = if *filled { *color } else { Color32::TRANSPARENT };
                        painter.add(path_shape(pts, true, fill, stroke_of(*stroke_width, *color)));
                    }
                    AnnotationItem::Line { start, end, color, stroke_width } => {
                        painter.line_segment([rp(*start), rp(*end)], stroke_of(*stroke_width, *color));
                    }
                    AnnotationItem::Arrow { start, end, color, stroke_width } => {
                        let (s, e) = (rp(*start), rp(*end));
                        painter.line_segment([s, e], stroke_of(*stroke_width, *color));
                        arrow_head(painter, s, e, *stroke_width, *color);
                    }
                    AnnotationItem::DoubleArrow { start, end, color, stroke_width } => {
                        let (s, e) = (rp(*start), rp(*end));
                        painter.line_segment([s, e], stroke_of(*stroke_width, *color));
                        arrow_head(painter, s, e, *stroke_width, *color);
                        arrow_head(painter, e, s, *stroke_width, *color);
                    }
                    AnnotationItem::Polyline { points, color, stroke_width } => {
                        let pts = points.iter().map(|p| rp(*p)).collect();
                        painter.add(path_shape(pts, false, Color32::TRANSPARENT, stroke_of(*stroke_width, *color)));
                    }
                    AnnotationItem::Pen { points, color, stroke_width } => {
                        for w in points.windows(2) {
                            painter.line_segment([rp(w[0]), rp(w[1])], stroke_of(*stroke_width, *color));
                        }
                    }
                    AnnotationItem::Highlighter { points, color, stroke_width } => {
                        let semi = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 80);
                        for w in points.windows(2) {
                            painter.line_segment([rp(w[0]), rp(w[1])], stroke_of(*stroke_width * 3.0, semi));
                        }
                    }
                    AnnotationItem::Mosaic { rect, .. } => {
                        let pts = vec![rp(rect.left_top()), rp(rect.right_top()), rp(rect.right_bottom()), rp(rect.left_bottom())];
                        painter.add(path_shape(pts, true, Color32::TRANSPARENT, stroke_of(1.0, Color32::from_rgba_unmultiplied(100, 100, 255, 180))));
                    }
                    AnnotationItem::Blur { rect, .. } => {
                        let pts = vec![rp(rect.left_top()), rp(rect.right_top()), rp(rect.right_bottom()), rp(rect.left_bottom())];
                        painter.add(path_shape(pts, true, Color32::from_white_alpha(36), stroke_of(1.0, Color32::from_white_alpha(140))));
                    }
                    AnnotationItem::Text { text, color, font_size, .. } => {
                        let galley = painter.layout_no_wrap(text.clone(), FontId::new(*font_size, FontFamily::Proportional), *color);
                        // Pivot is the galley's top-left; place it so the rotated center lands on `center`.
                        let pos = rp(*center - galley.size() / 2.0);
                        painter.add(egui::epaint::TextShape::new(pos, galley, *color).with_angle(*angle));
                    }
                    AnnotationItem::StepBadge { pos, number, color, radius, square } => {
                        let c = rp(*pos);
                        if *square {
                            let r = Rect::from_center_size(*pos, Vec2::splat(*radius * 2.0));
                            let pts = rounded_rect_points(r, radius * 0.3).into_iter().map(rp).collect();
                            painter.add(path_shape(pts, true, *color, stroke_of(1.5, Color32::WHITE)));
                        } else {
                            painter.circle_filled(c, *radius, *color);
                            painter.circle_stroke(c, *radius, stroke_of(1.5, Color32::WHITE));
                        }
                        let galley = painter.layout_no_wrap(number.to_string(), FontId::new(*radius * 1.1, FontFamily::Proportional), Color32::WHITE);
                        let tpos = rp(*pos - galley.size() / 2.0);
                        painter.add(egui::epaint::TextShape::new(tpos, galley, Color32::WHITE).with_angle(*angle));
                    }
                    AnnotationItem::Rotated { .. } => unreachable!("rotation wrappers are never nested"),
                }
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
                filled,
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
                    *filled,
                );
            }
            AnnotationItem::Ellipse {
                rect,
                color,
                stroke_width,
                filled,
            } => {
                let (cx, cy) = to_phys(rect.center());
                let rx = ((rect.width() / 2.0) * scale_factor).round().abs() as i32;
                let ry = ((rect.height() / 2.0) * scale_factor).round().abs() as i32;
                let thickness = (*stroke_width * scale_factor).round().max(1.0) as u32;
                ImageProcessor::draw_ellipse(img, cx, cy, rx, ry, thickness, to_rgba(*color), *filled);
            }
            AnnotationItem::RoundedRect {
                rect,
                color,
                stroke_width,
                filled,
            } => {
                let (x0, y0) = to_phys(rect.min);
                let (x1, y1) = to_phys(rect.max);
                let w = (x1 - x0).abs() as u32;
                let h = (y1 - y0).abs() as u32;
                let thickness = (*stroke_width * scale_factor).round().max(1.0) as u32;
                let r = (corner_radius(*rect) * scale_factor).round() as i32;
                ImageProcessor::draw_rounded_rect(
                    img,
                    x0.min(x1),
                    y0.min(y1),
                    w,
                    h,
                    r,
                    thickness,
                    to_rgba(*color),
                    *filled,
                );
            }
            AnnotationItem::Diamond {
                rect,
                color,
                stroke_width,
                filled,
            } => {
                let pts: Vec<(i32, i32)> = diamond_points(*rect).iter().map(|p| to_phys(*p)).collect();
                let thickness = (*stroke_width * scale_factor).round().max(1.0) as u32;
                ImageProcessor::draw_polygon(img, &pts, thickness, to_rgba(*color), *filled, true);
            }
            AnnotationItem::Polygon {
                points,
                color,
                stroke_width,
                filled,
            } => {
                let pts: Vec<(i32, i32)> = points.iter().map(|p| to_phys(*p)).collect();
                let thickness = (*stroke_width * scale_factor).round().max(1.0) as u32;
                ImageProcessor::draw_polygon(img, &pts, thickness, to_rgba(*color), *filled, true);
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
            AnnotationItem::Polyline {
                points,
                color,
                stroke_width,
            } => {
                let pts: Vec<(i32, i32)> = points.iter().map(|p| to_phys(*p)).collect();
                let thickness = (*stroke_width * scale_factor).round().max(1.0) as u32;
                ImageProcessor::draw_polygon(img, &pts, thickness, to_rgba(*color), false, false);
            }
            AnnotationItem::DoubleArrow {
                start,
                end,
                color,
                stroke_width,
            } => {
                let (x0, y0) = to_phys(*start);
                let (x1, y1) = to_phys(*end);
                let thickness = (*stroke_width * scale_factor).round().max(1.0) as u32;
                ImageProcessor::draw_arrow(img, x0, y0, x1, y1, thickness, to_rgba(*color));
                ImageProcessor::draw_arrow(img, x1, y1, x0, y0, thickness, to_rgba(*color));
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
            AnnotationItem::Blur { rect, radius } => {
                let (x0, y0) = to_phys(rect.min);
                let (x1, y1) = to_phys(rect.max);
                let phys_w = (x1 - x0).abs() as u32;
                let phys_h = (y1 - y0).abs() as u32;
                let phys_radius = (*radius as f32 * scale_factor).round().max(1.0) as u32;
                ImageProcessor::apply_blur(
                    img,
                    x0.min(x1).max(0) as u32,
                    y0.min(y1).max(0) as u32,
                    phys_w,
                    phys_h,
                    phys_radius,
                );
            }
            AnnotationItem::StepBadge {
                pos,
                number,
                color,
                radius,
                square,
            } => {
                let (cx, cy) = to_phys(*pos);
                let phys_r = (*radius * scale_factor).round().max(4.0) as i32;
                if *square {
                    ImageProcessor::draw_step_badge_square(img, cx, cy, phys_r, *number, to_rgba(*color));
                } else {
                    ImageProcessor::draw_step_badge(img, cx, cy, phys_r, *number, to_rgba(*color));
                }
            }
            AnnotationItem::Text {
                pos,
                text,
                color,
                font_size,
            } => {
                let (x, y) = to_phys(*pos);
                ImageProcessor::draw_text(img, x, y, text, *font_size * scale_factor, to_rgba(*color));
            }
            AnnotationItem::Rotated { item, angle, center } => {
                // Mosaic/blur are pixel effects on the underlying image; apply them
                // over the rotated axis-aligned extent instead of blitting transparency.
                match item.as_ref() {
                    AnnotationItem::Mosaic { block_size, .. } => {
                        let b = self.bounds();
                        let (x0, y0) = to_phys(b.min);
                        let (x1, y1) = to_phys(b.max);
                        let phys_block = (*block_size as f32 * scale_factor).round().max(4.0) as u32;
                        ImageProcessor::apply_mosaic(
                            img,
                            x0.max(0) as u32,
                            y0.max(0) as u32,
                            (x1 - x0).abs() as u32,
                            (y1 - y0).abs() as u32,
                            phys_block,
                        );
                        return;
                    }
                    AnnotationItem::Blur { radius, .. } => {
                        let b = self.bounds();
                        let (x0, y0) = to_phys(b.min);
                        let (x1, y1) = to_phys(b.max);
                        let phys_radius = (*radius as f32 * scale_factor).round().max(1.0) as u32;
                        ImageProcessor::apply_blur(
                            img,
                            x0.max(0) as u32,
                            y0.max(0) as u32,
                            (x1 - x0).abs() as u32,
                            (y1 - y0).abs() as u32,
                            phys_radius,
                        );
                        return;
                    }
                    _ => {}
                }

                // Render the inner item upright into a temp image, then rotate-blit it.
                let b = item.bounds().expand(4.0);
                let tw = (b.width() * scale_factor).ceil().max(1.0) as u32;
                let th = (b.height() * scale_factor).ceil().max(1.0) as u32;
                let mut tmp = RgbaImage::new(tw, th);
                item.bake_to_image(&mut tmp, b.min, scale_factor);

                let cx = (center.x - b.min.x) * scale_factor;
                let cy = (center.y - b.min.y) * scale_factor;
                let ox = (b.min.x - crop_origin.x) * scale_factor;
                let oy = (b.min.y - crop_origin.y) * scale_factor;
                let (sin, cos) = angle.sin_cos();

                let mut min_x = f32::MAX;
                let mut min_y = f32::MAX;
                let mut max_x = f32::MIN;
                let mut max_y = f32::MIN;
                for (px, py) in [(0.0, 0.0), (tw as f32, 0.0), (0.0, th as f32), (tw as f32, th as f32)] {
                    let rx = ox + cx + cos * (px - cx) - sin * (py - cy);
                    let ry = oy + cy + sin * (px - cx) + cos * (py - cy);
                    min_x = min_x.min(rx);
                    min_y = min_y.min(ry);
                    max_x = max_x.max(rx);
                    max_y = max_y.max(ry);
                }
                let x0 = min_x.floor().max(0.0) as u32;
                let y0 = min_y.floor().max(0.0) as u32;
                let x1 = (max_x.ceil() as u32).min(img.width());
                let y1 = (max_y.ceil() as u32).min(img.height());

                for dy in y0..y1 {
                    for dx in x0..x1 {
                        let lx = dx as f32 + 0.5 - ox;
                        let ly = dy as f32 + 0.5 - oy;
                        let sx = cx + cos * (lx - cx) + sin * (ly - cy) - 0.5;
                        let sy = cy - sin * (lx - cx) + cos * (ly - cy) - 0.5;
                        let Some(s) = sample_premul(&tmp, sx, sy) else { continue };
                        let ia = 1.0 - s[3] / 255.0;
                        let d = img.get_pixel_mut(dx, dy);
                        for c in 0..4 {
                            d[c] = (s[c] + d[c] as f32 * ia).round().min(255.0) as u8;
                        }
                    }
                }
            }
        }
    }
}

/// Whether an item (possibly rotation-wrapped) is a step badge.
fn is_badge(item: &AnnotationItem) -> bool {
    match item {
        AnnotationItem::StepBadge { .. } => true,
        AnnotationItem::Rotated { item, .. } => is_badge(item),
        _ => false,
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
        if is_badge(&item) {
            self.step_counter += 1;
        }
        self.undo_stack.push(item);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(item) = self.undo_stack.pop() {
            if is_badge(&item) {
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
            if is_badge(&item) {
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

    pub fn len(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn item_mut(&mut self, index: usize) -> Option<&mut AnnotationItem> {
        self.undo_stack.get_mut(index)
    }

    /// Replaces one annotation in place (finishing a resumed multi-point edit);
    /// clears the redo branch like any other mutation.
    pub fn replace(&mut self, index: usize, item: AnnotationItem) -> bool {
        if let Some(slot) = self.undo_stack.get_mut(index) {
            *slot = item;
            self.redo_stack.clear();
            true
        } else {
            false
        }
    }

    /// Removes one annotation (edit-mode delete); clears the redo branch.
    pub fn remove(&mut self, index: usize) -> Option<AnnotationItem> {
        if index >= self.undo_stack.len() {
            return None;
        }
        let item = self.undo_stack.remove(index);
        if is_badge(&item) {
            self.step_counter = self.step_counter.saturating_sub(1).max(1);
        }
        self.redo_stack.clear();
        Some(item)
    }

    pub fn is_empty(&self) -> bool {
        self.undo_stack.is_empty()
    }

    /// Removes every annotation and resets the step badge counter.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.step_counter = 1;
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
            filled: false,
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
            square: false,
        });
        assert_eq!(history.step_counter, 2);

        history.undo();
        assert_eq!(history.step_counter, 1);
    }

    #[test]
    fn test_clear_resets_history_and_counter() {
        let mut history = HistoryManager::new();
        history.push(AnnotationItem::StepBadge {
            pos: Pos2::new(10.0, 10.0),
            number: 1,
            color: Color32::BLUE,
            radius: 12.0,
            square: false,
        });
        history.undo();
        assert!(history.can_redo());

        history.clear();
        assert!(history.is_empty());
        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.step_counter, 1);
    }

    #[test]
    fn test_filled_rectangle_bake_covers_interior() {
        let mut img = RgbaImage::new(40, 40);
        let item = AnnotationItem::Rectangle {
            rect: Rect::from_min_max(Pos2::new(5.0, 5.0), Pos2::new(35.0, 35.0)),
            color: Color32::from_rgb(255, 0, 0),
            stroke_width: 2.0,
            filled: true,
        };
        item.bake_to_image(&mut img, Pos2::ZERO, 1.0);
        // Interior pixel must be painted red by the fill, not just the outline.
        assert_eq!(img.get_pixel(20, 20), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn test_blur_bake_spreads_isolated_pixel() {
        let mut img = RgbaImage::new(30, 30);
        img.put_pixel(15, 15, Rgba([255, 255, 255, 255]));
        let item = AnnotationItem::Blur {
            rect: Rect::from_min_max(Pos2::ZERO, Pos2::new(30.0, 30.0)),
            radius: 3,
        };
        item.bake_to_image(&mut img, Pos2::ZERO, 1.0);
        assert!(img.get_pixel(14, 15)[0] > 0, "blur should spread the bright pixel");
    }

    #[test]
    fn test_text_bake_rasterizes_glyphs() {
        let mut img = RgbaImage::new(240, 60);
        let item = AnnotationItem::Text {
            pos: Pos2::new(5.0, 5.0),
            text: "你好 rshot".to_string(),
            color: Color32::from_rgb(255, 60, 60),
            font_size: 24.0,
        };
        item.bake_to_image(&mut img, Pos2::ZERO, 1.0);
        let ink = img.pixels().filter(|p| p[0] > 100 && p[3] > 100).count();
        assert!(ink > 100, "text should rasterize visible glyphs, got {ink} ink pixels");
    }

    #[test]
    fn test_polygon_fill_covers_interior() {
        let mut img = RgbaImage::new(40, 40);
        let item = AnnotationItem::Polygon {
            points: vec![Pos2::new(5.0, 5.0), Pos2::new(35.0, 5.0), Pos2::new(20.0, 35.0)],
            color: Color32::from_rgb(255, 0, 0),
            stroke_width: 2.0,
            filled: true,
        };
        item.bake_to_image(&mut img, Pos2::ZERO, 1.0);
        assert_eq!(img.get_pixel(20, 15), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn test_rounded_rect_bake_rounds_corners() {
        let mut img = RgbaImage::new(40, 40);
        let item = AnnotationItem::RoundedRect {
            rect: Rect::from_min_max(Pos2::new(5.0, 5.0), Pos2::new(35.0, 35.0)),
            color: Color32::from_rgb(255, 0, 0),
            stroke_width: 2.0,
            filled: false,
        };
        item.bake_to_image(&mut img, Pos2::ZERO, 1.0);
        assert_eq!(img.get_pixel(20, 5), &Rgba([255, 0, 0, 255]), "top edge painted");
        assert_ne!(img.get_pixel(6, 6), &Rgba([255, 0, 0, 255]), "corner rounded away");
        assert_ne!(img.get_pixel(20, 20), &Rgba([255, 0, 0, 255]), "outline only");
    }

    #[test]
    fn test_polyline_bake_is_open() {
        let mut img = RgbaImage::new(40, 40);
        let item = AnnotationItem::Polyline {
            points: vec![Pos2::new(5.0, 5.0), Pos2::new(20.0, 20.0), Pos2::new(35.0, 5.0)],
            color: Color32::from_rgb(255, 0, 0),
            stroke_width: 2.0,
        };
        item.bake_to_image(&mut img, Pos2::ZERO, 1.0);
        assert_eq!(img.get_pixel(20, 20), &Rgba([255, 0, 0, 255]), "vertex painted");
        assert_ne!(img.get_pixel(20, 5), &Rgba([255, 0, 0, 255]), "no closing edge");
    }

    #[test]
    fn test_double_arrow_bake_heads_both_ends() {
        let mut img = RgbaImage::new(50, 40);
        let item = AnnotationItem::DoubleArrow {
            start: Pos2::new(10.0, 20.0),
            end: Pos2::new(40.0, 20.0),
            color: Color32::from_rgb(255, 0, 0),
            stroke_width: 2.0,
        };
        item.bake_to_image(&mut img, Pos2::ZERO, 1.0);
        let red = Rgba([255, 0, 0, 255]);
        assert_eq!(img.get_pixel(28, 13), &red, "head wing at end");
        assert_eq!(img.get_pixel(22, 13), &red, "head wing at start");
    }

    #[test]
    fn test_square_badge_bake_covers_corner() {
        let mut img = RgbaImage::new(40, 40);
        let item = AnnotationItem::StepBadge {
            pos: Pos2::new(20.0, 20.0),
            number: 1,
            color: Color32::from_rgb(255, 0, 0),
            radius: 10.0,
            square: true,
        };
        item.bake_to_image(&mut img, Pos2::ZERO, 1.0);
        // (12,12) lies outside the circle's radius but inside the square fill,
        // clear of the 2px white border that hugs the edges.
        assert_eq!(img.get_pixel(12, 12), &Rgba([255, 0, 0, 255]), "square badge fills corner");
    }

    fn rotated_rect() -> AnnotationItem {
        AnnotationItem::Rotated {
            item: Box::new(AnnotationItem::Rectangle {
                rect: Rect::from_min_max(Pos2::new(10.0, 5.0), Pos2::new(30.0, 15.0)),
                color: Color32::from_rgb(255, 0, 0),
                stroke_width: 2.0,
                filled: true,
            }),
            angle: std::f32::consts::FRAC_PI_2,
            center: Pos2::new(20.0, 10.0),
        }
    }

    #[test]
    fn rotated_bounds_swap_axes_at_90_degrees() {
        let b = rotated_rect().bounds();
        // Original 20x10 rect becomes a 10x20 vertical bar around (20,10), plus padding.
        assert!(b.width() < b.height());
        assert!(b.contains(Pos2::new(20.0, 2.0)));
        assert!(!b.contains(Pos2::new(11.0, 10.0)));
    }

    #[test]
    fn rotated_hit_test_follows_the_shape() {
        let item = rotated_rect();
        assert!(item.hit_test(Pos2::new(20.0, 3.0), 1.0), "inside rotated bar");
        assert!(!item.hit_test(Pos2::new(12.0, 10.0), 1.0), "original footprint is now empty");
        assert_eq!(item.rotation(), std::f32::consts::FRAC_PI_2);
    }

    #[test]
    fn translate_moves_rotated_item_and_pivot() {
        let mut item = rotated_rect();
        item.translate(Vec2::new(5.0, -3.0));
        let AnnotationItem::Rotated { center, .. } = &item else { panic!() };
        assert_eq!(*center, Pos2::new(25.0, 7.0));
        assert!(item.hit_test(Pos2::new(25.0, 0.0), 1.0));
    }

    #[test]
    fn rotated_bake_paints_turned_pixels() {
        let mut img = RgbaImage::new(40, 40);
        rotated_rect().bake_to_image(&mut img, Pos2::ZERO, 1.0);
        let red = Rgba([255, 0, 0, 255]);
        assert_eq!(img.get_pixel(20, 10), &red, "center stays painted");
        assert_eq!(img.get_pixel(20, 3), &red, "vertical bar after 90 deg turn");
        assert_ne!(img.get_pixel(12, 10), &red, "old horizontal footprint cleared");
        assert_ne!(img.get_pixel(28, 10), &red);
    }

    #[test]
    fn rotated_line_bake_turns_vertical() {
        let mut img = RgbaImage::new(40, 40);
        let item = AnnotationItem::Rotated {
            item: Box::new(AnnotationItem::Line {
                start: Pos2::new(5.0, 20.0),
                end: Pos2::new(35.0, 20.0),
                color: Color32::from_rgb(255, 0, 0),
                stroke_width: 2.0,
            }),
            angle: std::f32::consts::FRAC_PI_2,
            center: Pos2::new(20.0, 20.0),
        };
        item.bake_to_image(&mut img, Pos2::ZERO, 1.0);
        assert!(img.get_pixel(20, 10)[0] > 200, "vertical line painted");
        assert_eq!(img.get_pixel(10, 20)[3], 0, "horizontal footprint empty");
    }

    #[test]
    fn polygon_hit_test_edges_and_fill() {
        let outline = AnnotationItem::Polygon {
            points: vec![Pos2::new(0.0, 0.0), Pos2::new(40.0, 0.0), Pos2::new(20.0, 40.0)],
            color: Color32::RED,
            stroke_width: 2.0,
            filled: false,
        };
        assert!(outline.hit_test(Pos2::new(20.0, 0.0), 2.0), "top edge");
        assert!(!outline.hit_test(Pos2::new(20.0, 15.0), 2.0), "hollow interior");
        let filled = AnnotationItem::Polygon {
            points: vec![Pos2::new(0.0, 0.0), Pos2::new(40.0, 0.0), Pos2::new(20.0, 40.0)],
            color: Color32::RED,
            stroke_width: 2.0,
            filled: true,
        };
        assert!(filled.hit_test(Pos2::new(20.0, 15.0), 2.0), "filled interior");
    }

    #[test]
    fn history_remove_deletes_one_and_adjusts_badge_counter() {
        let mut history = HistoryManager::new();
        history.push(AnnotationItem::StepBadge {
            pos: Pos2::new(10.0, 10.0),
            number: 1,
            color: Color32::BLUE,
            radius: 12.0,
            square: false,
        });
        history.push(AnnotationItem::Line {
            start: Pos2::ZERO,
            end: Pos2::new(10.0, 10.0),
            color: Color32::RED,
            stroke_width: 2.0,
        });
        assert_eq!(history.len(), 2);
        assert_eq!(history.step_counter, 2);

        let removed = history.remove(0).expect("badge removed");
        assert!(matches!(removed, AnnotationItem::StepBadge { .. }));
        assert_eq!(history.len(), 1);
        assert_eq!(history.step_counter, 1);
        assert!(!history.can_redo(), "delete clears the redo branch");
        assert!(history.remove(5).is_none());
    }

    #[test]
    fn polyline_vertex_move_and_remove_respect_minimum() {
        let mut item = AnnotationItem::Polyline {
            points: vec![Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0), Pos2::new(20.0, 0.0)],
            color: Color32::RED,
            stroke_width: 4.0,
        };
        assert_eq!(item.multi_point_kind(), Some(ToolType::Polyline));
        assert_eq!(item.multi_point_style(), Some((Color32::RED, 4.0, false)));
        item.move_vertex(1, Pos2::new(10.0, 5.0));
        assert_eq!(item.editable_vertices().unwrap()[1], Pos2::new(10.0, 5.0));
        assert!(item.remove_vertex(2), "3 -> 2 vertices allowed");
        assert!(!item.remove_vertex(0), "a polyline keeps at least 2 vertices");
        assert_eq!(item.editable_vertices().unwrap().len(), 2);
    }

    #[test]
    fn polygon_keeps_at_least_three_vertices() {
        let mut item = AnnotationItem::Polygon {
            points: vec![Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0), Pos2::new(5.0, 10.0)],
            color: Color32::RED,
            stroke_width: 4.0,
            filled: true,
        };
        assert_eq!(item.multi_point_kind(), Some(ToolType::Polygon));
        assert!(!item.remove_vertex(0), "a polygon keeps at least 3 vertices");
        assert_eq!(item.editable_vertices().unwrap().len(), 3);
    }

    #[test]
    fn rotated_polyline_vertices_transform_both_ways() {
        let inner = AnnotationItem::Polyline {
            points: vec![Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0)],
            color: Color32::RED,
            stroke_width: 4.0,
        };
        let mut item = AnnotationItem::Rotated {
            item: Box::new(inner),
            angle: std::f32::consts::FRAC_PI_2,
            center: Pos2::ZERO,
        };
        let verts = item.editable_vertices().unwrap();
        assert!(verts[1].distance(Pos2::new(0.0, 10.0)) < 1e-3, "90 deg turns (10,0) into (0,10)");
        // Dragging the on-screen vertex back out inverse-rotates into local space.
        item.move_vertex(1, Pos2::new(0.0, 20.0));
        let verts = item.editable_vertices().unwrap();
        assert!(verts[1].distance(Pos2::new(0.0, 20.0)) < 1e-3);
        assert_eq!(item.multi_point_kind(), Some(ToolType::Polyline));
    }

    #[test]
    fn history_replace_swaps_in_place_and_clears_redo() {
        let mut history = HistoryManager::new();
        history.push(AnnotationItem::Line {
            start: Pos2::ZERO,
            end: Pos2::new(10.0, 10.0),
            color: Color32::RED,
            stroke_width: 2.0,
        });
        history.undo();
        assert!(history.can_redo());
        history.redo();
        let replacement = AnnotationItem::Polyline {
            points: vec![Pos2::ZERO, Pos2::new(5.0, 5.0)],
            color: Color32::BLUE,
            stroke_width: 2.0,
        };
        assert!(history.replace(0, replacement));
        assert_eq!(history.len(), 1);
        assert!(matches!(history.items()[0], AnnotationItem::Polyline { .. }));
        assert!(!history.can_redo(), "replace clears the redo branch");
        assert!(!history.replace(9, AnnotationItem::Line {
            start: Pos2::ZERO,
            end: Pos2::new(1.0, 1.0),
            color: Color32::RED,
            stroke_width: 2.0,
        }));
    }
}
