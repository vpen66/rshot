use crate::annotation::{
    AnnotationItem, COLOR_PRESETS, FAMILIES, HistoryManager, StrokeWidth, ToolType, paint_gizmo,
    paint_multi_gizmo, rotation_handle_pos,
};
use crate::magnifier::Magnifier;
use crate::state::{DragState, ResizeHandle, Selection};
use crate::toolbar::{FONT_MAX, FONT_MIN, Toolbar, ToolbarAction, ToolbarState};
use eframe::egui::{
    self, Color32, CursorIcon, FontId, Key, Modifiers, Painter, Pos2, Rect, RichText, Stroke,
    StrokeKind, Vec2,
};
use image::RgbaImage;
use log::{error, info};
use rshot_core::{ClipboardService, ScreenCapture};
use rshot_storage::{OutputFormat, StorageService};
use std::sync::Arc;

pub struct OverlayApp {
    captured_image: Arc<RgbaImage>,
    texture: Option<egui::TextureHandle>,
    scale_factor: f32,
    monitor_origin: (f32, f32),
    selection: Option<Selection>,
    drag_state: DragState,
    hovered_handle: Option<ResizeHandle>,
    handle_radius: f32,

    // Phase 2 Annotation State
    current_tool: ToolType,
    current_color: Color32,
    current_stroke: StrokeWidth,
    filled: bool,
    font_size: f32,
    mosaic_block: u32,
    blur_radius: u32,
    toolbar_pos: Option<Pos2>,
    family_tool: [ToolType; FAMILIES.len()],
    badge_square: bool,
    poly_points: Vec<Pos2>,
    poly_last_click: Option<(f64, Pos2)>,
    history: HistoryManager,
    selected: Option<usize>,
    hover_rotation: bool,
    active_vertex: Option<usize>,
    hover_vertex: Option<usize>,
    editing_index: Option<usize>,
    drawing_item: Option<AnnotationItem>,
    draw_start_pos: Option<Pos2>,
    pen_points: Vec<Pos2>,
    active_text_pos: Option<Pos2>,
    active_text_buffer: String,
}

impl OverlayApp {
    pub fn new(captured_image: RgbaImage, scale_factor: f32, monitor_origin: (f32, f32)) -> Self {
        Self {
            captured_image: Arc::new(captured_image),
            texture: None,
            scale_factor,
            monitor_origin,
            selection: None,
            drag_state: DragState::Idle,
            hovered_handle: None,
            handle_radius: 6.0,

            current_tool: ToolType::Select,
            current_color: COLOR_PRESETS[0],
            current_stroke: StrokeWidth::Medium,
            filled: false,
            font_size: 18.0,
            mosaic_block: 10,
            blur_radius: 8,
            toolbar_pos: None,
            family_tool: std::array::from_fn(|i| FAMILIES[i].default_tool()),
            badge_square: false,
            poly_points: Vec::new(),
            poly_last_click: None,
            history: HistoryManager::new(),
            selected: None,
            hover_rotation: false,
            active_vertex: None,
            hover_vertex: None,
            editing_index: None,
            drawing_item: None,
            draw_start_pos: None,
            pen_points: Vec::new(),
            active_text_pos: None,
            active_text_buffer: String::new(),
        }
    }

    /// Crops the selected region from the original full-resolution image and bakes all annotations onto it.
    fn get_cropped_selection(&self) -> Option<RgbaImage> {
        let sel = self.selection?;
        if !sel.is_valid() {
            return None;
        }

        let min_x = (sel.min.x * self.scale_factor).round() as u32;
        let min_y = (sel.min.y * self.scale_factor).round() as u32;
        let width = (sel.width() * self.scale_factor).round() as u32;
        let height = (sel.height() * self.scale_factor).round() as u32;

        let mut cropped =
            ScreenCapture::crop_region(&self.captured_image, min_x, min_y, width, height).ok()?;

        // Bake all completed annotations onto the physical image buffer
        for item in self.history.items() {
            item.bake_to_image(&mut cropped, sel.min, self.scale_factor);
        }

        Some(cropped)
    }

    /// Handles copying cropped selection with baked annotations to clipboard.
    fn handle_copy(&self) {
        if let Some(cropped) = self.get_cropped_selection() {
            if let Err(e) = ClipboardService::copy_image(&cropped) {
                error!("Failed to copy to clipboard: {e}");
            } else {
                info!("Screenshot (with annotations) successfully copied to clipboard");
            }
        }
        std::process::exit(0);
    }

    /// Handles saving cropped selection with baked annotations to disk.
    fn handle_save(&self) {
        if let Some(cropped) = self.get_cropped_selection() {
            match StorageService::save_to_default_dir(&cropped, OutputFormat::Png) {
                Ok(path) => info!(
                    "Screenshot (with annotations) successfully saved to: {}",
                    path.display()
                ),
                Err(e) => error!("Failed to save screenshot: {e}"),
            }
        }
        std::process::exit(0);
    }

    /// Hands the cropped selection to a fresh pin process and exits.
    /// macOS allows only one winit event loop per process, so the pin window
    /// must run in a child process; ViewportCommand::Close also never unwinds
    /// the macOS run loop, hence process::exit on every exit path.
    fn handle_pin(&self) {
        if let (Some(cropped), Some(sel)) = (self.get_cropped_selection(), self.selection) {
            let global_x = self.monitor_origin.0 + sel.min.x;
            let global_y = self.monitor_origin.1 + sel.min.y;
            let path = std::env::temp_dir().join(format!("rshot_pin_{}.png", std::process::id()));
            if let Err(e) = cropped.save(&path) {
                error!("Failed to write pin handoff image: {e}");
            } else {
                let result = std::env::current_exe().and_then(|exe| {
                    std::process::Command::new(exe)
                        .arg("--pin-image")
                        .arg(&path)
                        .arg("--pin-x")
                        .arg(global_x.to_string())
                        .arg("--pin-y")
                        .arg(global_y.to_string())
                        .arg("--pin-scale")
                        .arg(self.scale_factor.to_string())
                        .spawn()
                });
                match result {
                    Ok(_) => info!("Pinning screenshot at ({}, {})", global_x, global_y),
                    Err(e) => {
                        error!("Failed to launch pin process: {e}");
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
        std::process::exit(0);
    }

    fn commit_active_text(&mut self) {
        if let Some(pos) = self.active_text_pos.take() {
            if !self.active_text_buffer.trim().is_empty() {
                self.history.push(AnnotationItem::Text {
                    pos,
                    text: self.active_text_buffer.clone(),
                    color: self.current_color,
                    font_size: self.font_size,
                });
                self.selected = None;
            }
            self.active_text_buffer.clear();
        }
    }

    /// Finalizes the multi-click polygon/polyline built up so far. When resuming
    /// an existing shape (`editing_index`), the result replaces it in place; if
    /// too few points remain, the original is left untouched.
    fn commit_poly(&mut self, tool: ToolType) {
        self.poly_last_click = None;
        let mut points = std::mem::take(&mut self.poly_points);
        points.dedup();
        // Vertices clicked under an older, larger selection must not survive it:
        // they would draw stray lines across the screen after the box shrinks.
        match self.selection {
            Some(sel) => points.retain(|p| sel.to_rect().contains(*p)),
            None => points.clear(),
        }
        let stroke_w = self.current_stroke.to_pixels();
        let item = match tool {
            ToolType::Polygon if points.len() >= 3 => Some(AnnotationItem::Polygon {
                points,
                color: self.current_color,
                stroke_width: stroke_w,
                filled: self.filled,
            }),
            ToolType::Polyline if points.len() >= 2 => Some(AnnotationItem::Polyline {
                points,
                color: self.current_color,
                stroke_width: stroke_w,
            }),
            _ => None,
        };
        let editing = self.editing_index.take();
        if let Some(item) = item {
            match editing {
                Some(index) if index < self.history.len() => {
                    self.history.replace(index, item);
                }
                _ => self.history.push(item),
            }
            self.selected = None;
            self.active_vertex = None;
        }
    }

    /// Keeps each family's toolbar icon showing the variant last used.
    fn remember_family(&mut self) {
        let fi = self.current_tool.family().index();
        self.family_tool[fi] = self.current_tool;
    }

    /// Topmost annotation under `pos`, if any (edit-mode grab).
    fn hit_annotation(&self, pos: Pos2) -> Option<usize> {
        self.history
            .items()
            .iter()
            .enumerate()
            .rev()
            .find(|(_, item)| item.hit_test(pos, 4.0))
            .map(|(i, _)| i)
    }

    /// Sets an annotation's rotation, wrapping plain items around their center.
    fn set_annotation_rotation(&mut self, index: usize, angle: f32) {
        let Some(item) = self.history.item_mut(index) else {
            return;
        };
        match item {
            AnnotationItem::Rotated { angle: a, .. } => *a = angle,
            _ => {
                let center = item.bounds().center();
                let plain = item.clone();
                *item = AnnotationItem::Rotated {
                    item: Box::new(plain),
                    angle,
                    center,
                };
            }
        }
    }

    /// Deletes the selected annotation (Delete key / edit mode).
    fn delete_selected(&mut self) {
        if let Some(index) = self.selected.take() {
            self.history.remove(index);
        }
        self.active_vertex = None;
    }

    /// Vertex of the selected multi-point item nearest to `pos`, if grabbable.
    fn hit_vertex(&self, pos: Pos2) -> Option<usize> {
        let item = self.history.items().get(self.selected?)?;
        let verts = item.editable_vertices()?;
        let mut best = None;
        let mut best_d = 9.0;
        for (i, v) in verts.iter().enumerate() {
            let d = v.distance(pos);
            if d <= best_d {
                best_d = d;
                best = Some(i);
            }
        }
        best
    }

    /// True when no drawing/drag/typing is in progress, so a hover can engage
    /// editing of an existing annotation instead of starting a new stroke.
    fn can_hover_edit(&self) -> bool {
        self.drag_state == DragState::Idle
            && self.draw_start_pos.is_none()
            && self.poly_points.is_empty()
            && self.editing_index.is_none()
            && self.active_text_pos.is_none()
    }

    /// Whether `pos` is over the selected item's rotation handle or a vertex,
    /// which can sit just outside the annotation body itself.
    fn pointer_over_selected_gizmo(&self, pos: Pos2) -> bool {
        let Some(item) = self.selected.and_then(|i| self.history.items().get(i)) else {
            return false;
        };
        if rotation_handle_pos(item.bounds()).distance(pos) <= 9.0 {
            return true;
        }
        item.editable_vertices()
            .map(|vs| vs.iter().any(|v| v.distance(pos) <= 9.0))
            .unwrap_or(false)
    }

    /// Vertex to resume from when Enter is pressed on a selected multi-point
    /// item: the active vertex if one is picked, otherwise the last vertex.
    fn resume_target_vertex(&self) -> Option<usize> {
        let item = self.history.items().get(self.selected?)?;
        let verts = item.editable_vertices()?;
        if verts.is_empty() {
            return None;
        }
        Some(
            self.active_vertex
                .unwrap_or(verts.len() - 1)
                .min(verts.len() - 1),
        )
    }

    /// Resumes drawing a committed polyline/polygon from `vertex`: the path up to
    /// and including that vertex becomes the live preview, and new clicks extend
    /// it. The original stays in history (hidden) until the edit is committed, so
    /// Esc restores it. Double-clicking the last vertex simply appends to it.
    fn continue_from_vertex(&mut self, vertex: usize) {
        let Some(index) = self.selected else { return };
        let Some(item) = self.history.items().get(index) else {
            return;
        };
        let Some(kind) = item.multi_point_kind() else {
            return;
        };
        let Some(verts) = item.editable_vertices() else {
            return;
        };
        let Some((color, stroke, filled)) = item.multi_point_style() else {
            return;
        };
        if vertex >= verts.len() {
            return;
        }
        self.current_color = color;
        self.current_stroke = StrokeWidth::from_pixels(stroke);
        self.filled = filled;
        self.current_tool = kind;
        self.poly_points = verts[..=vertex].to_vec();
        self.poly_last_click = None;
        self.editing_index = Some(index);
        self.selected = None;
        self.active_vertex = None;
        self.hover_vertex = None;
        self.drag_state = DragState::Idle;
        self.remember_family();
    }

    /// Drops any in-progress multi-point draft / resumed edit without committing.
    fn reset_draft(&mut self) {
        self.poly_points.clear();
        self.poly_last_click = None;
        self.editing_index = None;
        self.active_vertex = None;
        self.hover_vertex = None;
    }
}

/// A quick second click at the same spot finishes a polygon/polyline;
/// rapid clicks at *different* spots must keep adding vertices.
fn poly_click_finishes(last: Option<(f64, Pos2)>, now: f64, pos: Pos2) -> bool {
    last.is_some_and(|(t, p)| now - t < 0.4 && p.distance(pos) < 10.0)
}

/// Small dark guidance badge anchored to the top-right of the selection.
fn paint_hint_badge(painter: &Painter, sel: Rect, text: &str) {
    let galley =
        painter.layout_no_wrap(text.to_string(), FontId::proportional(11.0), Color32::WHITE);
    let size = galley.size() + Vec2::new(16.0, 8.0);
    let mut pos = Pos2::new(sel.right() - size.x, sel.top() - size.y - 6.0);
    if pos.y < 4.0 {
        pos.y = sel.top() + 6.0;
    }
    let r = Rect::from_min_size(pos, size);
    painter.rect_filled(r, 5.0, Color32::from_rgba_unmultiplied(26, 28, 33, 242));
    painter.rect_stroke(
        r,
        5.0,
        Stroke::new(1.0, Color32::from_white_alpha(42)),
        StrokeKind::Inside,
    );
    painter.galley(r.min + Vec2::new(8.0, 4.0), galley, Color32::WHITE);
}

impl eframe::App for OverlayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Ensure GPU texture is loaded
        if self.texture.is_none() {
            let (w, h) = self.captured_image.dimensions();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [w as usize, h as usize],
                self.captured_image.as_raw(),
            );
            let tex = ctx.load_texture("screen_freeze", color_image, Default::default());
            self.texture = Some(tex);
        }

        // 2. Setup CentralPanel
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let screen_rect = ui.max_rect();
                let painter = ui.painter();

                // Draw frozen screen image
                if let Some(texture) = &self.texture {
                    painter.image(
                        texture.id(),
                        screen_rect,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }

                // 3. Process pointer inputs & hit testing
                let pointer_pos = ctx.input(|i| i.pointer.interact_pos());
                let pointer_down = ctx.input(|i| i.pointer.primary_down());
                let double_clicked = ctx.input(|i| {
                    i.pointer
                        .button_double_clicked(egui::PointerButton::Primary)
                });
                let pointer_over_widget = ctx.wants_pointer_input();

                // Hover-to-edit: with any drawing tool armed, moving the pointer
                // onto an existing annotation engages editing directly — no need
                // to switch to the arrow tool and click first. Moving off the
                // shape (and its gizmo) releases it again. Select keeps its own
                // click-to-select behaviour below.
                if self.current_tool != ToolType::Select
                    && self.can_hover_edit()
                    && !pointer_over_widget
                {
                    match (self.selection, pointer_pos) {
                        (Some(sel), Some(pos)) if sel.to_rect().contains(pos) => {
                            if let Some(idx) = self.hit_annotation(pos) {
                                self.selected = Some(idx);
                            } else if !self.pointer_over_selected_gizmo(pos) {
                                self.selected = None;
                                self.active_vertex = None;
                            }
                        }
                        _ => {
                            self.selected = None;
                            self.active_vertex = None;
                        }
                    }
                }

                // Effective edit mode: the arrow tool, an in-progress edit drag,
                // or a hover over a selected annotation while idle.
                let editing_drag = matches!(
                    self.drag_state,
                    DragState::MovingAnnotation { .. }
                        | DragState::RotatingAnnotation { .. }
                        | DragState::MovingVertex { .. }
                );
                let editing = self.current_tool == ToolType::Select
                    || editing_drag
                    || (self.selected.is_some() && self.can_hover_edit() && !pointer_over_widget);

                if double_clicked && !pointer_over_widget && self.selection.is_some() && editing {
                    if let Some(pos) = pointer_pos {
                        // Double-clicking a vertex resumes drawing from it.
                        if let Some(v) = self.hit_vertex(pos) {
                            self.continue_from_vertex(v);
                            return;
                        }
                        // Double-clicking empty space copies; an annotation edits.
                        if self.hit_annotation(pos).is_none() {
                            self.handle_copy();
                            return;
                        }
                    }
                }

                // Keep vertex-edit state consistent with the current selection.
                if self.selected.is_none() {
                    self.active_vertex = None;
                }
                if let Some(index) = self.selected {
                    match self
                        .history
                        .items()
                        .get(index)
                        .and_then(|i| i.editable_vertices())
                    {
                        Some(verts) => {
                            if self.active_vertex.is_some_and(|v| v >= verts.len()) {
                                self.active_vertex = None;
                            }
                        }
                        None => self.active_vertex = None,
                    }
                }

                // Hit test resize handles (capture resize stays Select-only).
                self.hover_rotation = false;
                self.hover_vertex = None;
                if self.current_tool == ToolType::Select {
                    if let (Some(sel), Some(pos)) = (self.selection, pointer_pos) {
                        self.hovered_handle = sel.hit_test_handle(pos, self.handle_radius + 4.0);
                    } else {
                        self.hovered_handle = None;
                    }
                } else {
                    self.hovered_handle = None;
                }
                if editing {
                    if let (Some(index), Some(pos)) = (self.selected, pointer_pos) {
                        if let Some(item) = self.history.items().get(index) {
                            let handle = rotation_handle_pos(item.bounds());
                            self.hover_rotation = handle.distance(pos) <= 9.0;
                        }
                        self.hover_vertex = self.hit_vertex(pos);
                    }
                }

                // Set cursor icon
                if let Some(handle) = self.hovered_handle {
                    let cursor = match handle {
                        ResizeHandle::TopLeft | ResizeHandle::BottomRight => CursorIcon::ResizeNwSe,
                        ResizeHandle::TopRight | ResizeHandle::BottomLeft => CursorIcon::ResizeNeSw,
                        ResizeHandle::TopCenter | ResizeHandle::BottomCenter => {
                            CursorIcon::ResizeVertical
                        }
                        ResizeHandle::LeftCenter | ResizeHandle::RightCenter => {
                            CursorIcon::ResizeHorizontal
                        }
                    };
                    ctx.set_cursor_icon(cursor);
                } else if matches!(
                    self.drag_state,
                    DragState::RotatingAnnotation { .. } | DragState::MovingVertex { .. }
                ) {
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                } else if self.hover_rotation {
                    ctx.set_cursor_icon(CursorIcon::Grab);
                } else if self.hover_vertex.is_some() {
                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                } else if let (Some(sel), Some(pos)) = (self.selection, pointer_pos) {
                    if editing {
                        if sel.to_rect().contains(pos) || self.hit_annotation(pos).is_some() {
                            ctx.set_cursor_icon(CursorIcon::Move);
                        } else {
                            ctx.set_cursor_icon(CursorIcon::Crosshair);
                        }
                    } else if sel.to_rect().contains(pos) {
                        match self.current_tool {
                            ToolType::Text => ctx.set_cursor_icon(CursorIcon::Text),
                            _ => ctx.set_cursor_icon(CursorIcon::Crosshair),
                        }
                    } else {
                        ctx.set_cursor_icon(CursorIcon::Default);
                    }
                } else {
                    ctx.set_cursor_icon(CursorIcon::Crosshair);
                }

                // 4. Handle mouse drag state machine
                if pointer_down && !pointer_over_widget {
                    if let Some(pos) = pointer_pos {
                        if editing {
                            // Selection creation, move, or resize
                            match self.drag_state {
                                DragState::Idle => {
                                    if let Some(handle) = self.hovered_handle {
                                        if let Some(sel) = self.selection {
                                            self.drag_state = DragState::Resizing {
                                                handle,
                                                original_rect: sel.to_rect(),
                                                grab_pos: pos,
                                            };
                                        }
                                    } else if self.hover_rotation {
                                        if let Some(index) = self.selected {
                                            if let Some(item) = self.history.items().get(index) {
                                                let center = item.bounds().center();
                                                self.drag_state = DragState::RotatingAnnotation {
                                                    index,
                                                    center,
                                                    base_angle: item.rotation(),
                                                    grab_angle: (pos - center).angle(),
                                                };
                                            }
                                        }
                                    } else if let Some(v) = self.hit_vertex(pos) {
                                        if let Some(index) = self.selected {
                                            self.active_vertex = Some(v);
                                            self.drag_state =
                                                DragState::MovingVertex { index, vertex: v };
                                        }
                                    } else if let Some(index) = self.hit_annotation(pos) {
                                        self.selected = Some(index);
                                        self.active_vertex = None;
                                        self.drag_state = DragState::MovingAnnotation {
                                            index,
                                            last_pos: pos,
                                        };
                                    } else if let Some(sel) = self.selection {
                                        self.selected = None;
                                        self.active_vertex = None;
                                        if sel.to_rect().contains(pos) {
                                            self.drag_state = DragState::Moving {
                                                original_rect: sel.to_rect(),
                                                grab_offset: pos - sel.min,
                                            };
                                        } else {
                                            self.drag_state = DragState::Creating { start: pos };
                                        }
                                    } else {
                                        self.selected = None;
                                        self.active_vertex = None;
                                        self.drag_state = DragState::Creating { start: pos };
                                    }
                                }
                                DragState::Creating { start } => {
                                    self.selection = Some(Selection::new(start, pos));
                                }
                                DragState::Moving {
                                    original_rect,
                                    grab_offset,
                                } => {
                                    let new_min = pos - grab_offset;
                                    let new_max = new_min + original_rect.size();
                                    self.selection = Some(Selection {
                                        min: new_min,
                                        max: new_max,
                                    });
                                }
                                DragState::Resizing {
                                    handle,
                                    original_rect,
                                    ..
                                } => {
                                    let mut min = original_rect.min;
                                    let mut max = original_rect.max;
                                    match handle {
                                        ResizeHandle::TopLeft => {
                                            min.x = pos.x;
                                            min.y = pos.y;
                                        }
                                        ResizeHandle::TopCenter => min.y = pos.y,
                                        ResizeHandle::TopRight => {
                                            max.x = pos.x;
                                            min.y = pos.y;
                                        }
                                        ResizeHandle::RightCenter => max.x = pos.x,
                                        ResizeHandle::BottomRight => {
                                            max.x = pos.x;
                                            max.y = pos.y;
                                        }
                                        ResizeHandle::BottomCenter => max.y = pos.y,
                                        ResizeHandle::BottomLeft => {
                                            min.x = pos.x;
                                            max.y = pos.y;
                                        }
                                        ResizeHandle::LeftCenter => min.x = pos.x,
                                    }
                                    self.selection = Some(Selection::new(min, max));
                                }
                                DragState::MovingAnnotation { index, last_pos } => {
                                    let delta = pos - last_pos;
                                    if let Some(item) = self.history.item_mut(index) {
                                        item.translate(delta);
                                    }
                                    self.drag_state = DragState::MovingAnnotation {
                                        index,
                                        last_pos: pos,
                                    };
                                }
                                DragState::RotatingAnnotation {
                                    index,
                                    center,
                                    base_angle,
                                    grab_angle,
                                } => {
                                    let mut angle =
                                        base_angle + (pos - center).angle() - grab_angle;
                                    if ctx.input(|i| i.modifiers.shift) {
                                        let step = std::f32::consts::PI / 12.0;
                                        angle = (angle / step).round() * step;
                                    }
                                    self.set_annotation_rotation(index, angle);
                                }
                                DragState::MovingVertex { index, vertex } => {
                                    // Keep edited vertices inside the capture region.
                                    let target = match self.selection {
                                        Some(sel) => Pos2::new(
                                            pos.x.clamp(sel.min.x, sel.max.x),
                                            pos.y.clamp(sel.min.y, sel.max.y),
                                        ),
                                        None => pos,
                                    };
                                    if let Some(item) = self.history.item_mut(index) {
                                        item.move_vertex(vertex, target);
                                    }
                                }
                            }
                        } else if let Some(sel) = self.selection {
                            // Annotation drawing inside selection
                            let stroke_w = self.current_stroke.to_pixels();
                            let color = self.current_color;

                            if sel.to_rect().contains(pos) {
                                if self.draw_start_pos.is_none()
                                    && !self.current_tool.is_multi_point()
                                {
                                    self.draw_start_pos = Some(pos);
                                    if self.current_tool == ToolType::Pen
                                        || self.current_tool == ToolType::Highlighter
                                    {
                                        self.pen_points = vec![pos];
                                    }
                                }

                                if let Some(start) = self.draw_start_pos {
                                    match self.current_tool {
                                        ToolType::Rectangle => {
                                            self.drawing_item = Some(AnnotationItem::Rectangle {
                                                rect: Rect::from_two_pos(start, pos),
                                                color,
                                                stroke_width: stroke_w,
                                                filled: self.filled,
                                            });
                                        }
                                        ToolType::Ellipse => {
                                            self.drawing_item = Some(AnnotationItem::Ellipse {
                                                rect: Rect::from_two_pos(start, pos),
                                                color,
                                                stroke_width: stroke_w,
                                                filled: self.filled,
                                            });
                                        }
                                        ToolType::RoundedRect => {
                                            self.drawing_item = Some(AnnotationItem::RoundedRect {
                                                rect: Rect::from_two_pos(start, pos),
                                                color,
                                                stroke_width: stroke_w,
                                                filled: self.filled,
                                            });
                                        }
                                        ToolType::Diamond => {
                                            self.drawing_item = Some(AnnotationItem::Diamond {
                                                rect: Rect::from_two_pos(start, pos),
                                                color,
                                                stroke_width: stroke_w,
                                                filled: self.filled,
                                            });
                                        }
                                        ToolType::Arrow => {
                                            self.drawing_item = Some(AnnotationItem::Arrow {
                                                start,
                                                end: pos,
                                                color,
                                                stroke_width: stroke_w,
                                            });
                                        }
                                        ToolType::DoubleArrow => {
                                            self.drawing_item = Some(AnnotationItem::DoubleArrow {
                                                start,
                                                end: pos,
                                                color,
                                                stroke_width: stroke_w,
                                            });
                                        }
                                        ToolType::Line => {
                                            self.drawing_item = Some(AnnotationItem::Line {
                                                start,
                                                end: pos,
                                                color,
                                                stroke_width: stroke_w,
                                            });
                                        }
                                        ToolType::Pen => {
                                            self.pen_points.push(pos);
                                            self.drawing_item = Some(AnnotationItem::Pen {
                                                points: self.pen_points.clone(),
                                                color,
                                                stroke_width: stroke_w,
                                            });
                                        }
                                        ToolType::Highlighter => {
                                            self.pen_points.push(pos);
                                            self.drawing_item = Some(AnnotationItem::Highlighter {
                                                points: self.pen_points.clone(),
                                                color,
                                                stroke_width: stroke_w,
                                            });
                                        }
                                        ToolType::Mosaic => {
                                            self.drawing_item = Some(AnnotationItem::Mosaic {
                                                rect: Rect::from_two_pos(start, pos),
                                                block_size: self.mosaic_block,
                                            });
                                        }
                                        ToolType::Blur => {
                                            self.drawing_item = Some(AnnotationItem::Blur {
                                                rect: Rect::from_two_pos(start, pos),
                                                radius: self.blur_radius,
                                            });
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Mouse released
                    self.drag_state = DragState::Idle;

                    // Commit in-progress shape drawing
                    if let Some(item) = self.drawing_item.take() {
                        self.history.push(item);
                        self.selected = None;
                    }
                    self.draw_start_pos = None;
                    self.pen_points.clear();
                }

                // Handle single clicks for StepBadge or Text (only when the click
                // is not already editing an existing annotation).
                let pointer_clicked = ctx.input(|i| i.pointer.primary_clicked());
                if pointer_clicked && !pointer_over_widget && !editing {
                    if let (Some(sel), Some(pos)) = (self.selection, pointer_pos) {
                        if sel.to_rect().contains(pos) {
                            match self.current_tool {
                                ToolType::StepBadge => {
                                    self.history.push(AnnotationItem::StepBadge {
                                        pos,
                                        number: self.history.step_counter,
                                        color: self.current_color,
                                        radius: (self.current_stroke.to_pixels() * 3.0).max(12.0),
                                        square: self.badge_square,
                                    });
                                    self.selected = None;
                                }
                                ToolType::Polygon | ToolType::Polyline => {
                                    // egui's double-click is time-only, so rapid vertex clicks
                                    // would misfire; finish only on a quick click at the same spot.
                                    let now = ctx.input(|i| i.time);
                                    if poly_click_finishes(self.poly_last_click, now, pos) {
                                        self.commit_poly(self.current_tool);
                                    } else {
                                        self.poly_points.push(pos);
                                        self.poly_last_click = Some((now, pos));
                                    }
                                }
                                ToolType::Text => {
                                    self.commit_active_text();
                                    self.active_text_pos = Some(pos);
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // 5. Render Mask & Selection box
                if let Some(sel) = self.selection {
                    let rect = sel.to_rect();
                    let mask_color = Color32::from_black_alpha(130);

                    // Dim top
                    painter.rect_filled(
                        Rect::from_min_max(
                            screen_rect.left_top(),
                            Pos2::new(screen_rect.right(), rect.top()),
                        ),
                        0.0,
                        mask_color,
                    );
                    // Dim bottom
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(screen_rect.left(), rect.bottom()),
                            screen_rect.right_bottom(),
                        ),
                        0.0,
                        mask_color,
                    );
                    // Dim left
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(screen_rect.left(), rect.top()),
                            Pos2::new(rect.left(), rect.bottom()),
                        ),
                        0.0,
                        mask_color,
                    );
                    // Dim right
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(rect.right(), rect.top()),
                            Pos2::new(screen_rect.right(), rect.bottom()),
                        ),
                        0.0,
                        mask_color,
                    );

                    // Selection border
                    let border_stroke = Stroke::new(2.0, Color32::from_rgb(24, 144, 255));
                    painter.rect_stroke(rect, 0.0, border_stroke, StrokeKind::Outside);

                    // 8 Resizing handles (only in Select mode)
                    if self.current_tool == ToolType::Select {
                        for (_, handle_pos) in sel.get_handles() {
                            painter.circle_filled(handle_pos, self.handle_radius, Color32::WHITE);
                            painter.circle_stroke(
                                handle_pos,
                                self.handle_radius,
                                Stroke::new(1.5, Color32::from_rgb(24, 144, 255)),
                            );
                        }
                    }

                    // Dimension badge (e.g. "800 x 600"), measured to fit its text.
                    let (phys_w, phys_h) = (
                        (sel.width() * self.scale_factor).round() as u32,
                        (sel.height() * self.scale_factor).round() as u32,
                    );
                    let galley = painter.layout_no_wrap(
                        format!("{phys_w} × {phys_h}"),
                        FontId::monospace(11.0),
                        Color32::WHITE,
                    );
                    let badge_size = galley.size() + Vec2::new(16.0, 8.0);
                    let mut badge_pos = Pos2::new(rect.left(), rect.top() - badge_size.y - 6.0);
                    if badge_pos.y < 4.0 {
                        badge_pos.y = rect.top() + 6.0;
                    }
                    let badge_rect = Rect::from_min_size(badge_pos, badge_size);
                    painter.rect_filled(
                        badge_rect,
                        5.0,
                        Color32::from_rgba_unmultiplied(26, 28, 33, 242),
                    );
                    painter.rect_stroke(
                        badge_rect,
                        5.0,
                        Stroke::new(1.0, Color32::from_white_alpha(42)),
                        StrokeKind::Inside,
                    );
                    painter.galley(badge_rect.min + Vec2::new(8.0, 4.0), galley, Color32::WHITE);

                    // 6. Render All Committed Annotations, clipped to the selection:
                    // the export crops to this rect, so the overlay must match it.
                    // The item being resumed is hidden — its live preview stands in.
                    let clip_painter = painter.with_clip_rect(rect);
                    for (i, item) in self.history.items().iter().enumerate() {
                        if Some(i) == self.editing_index {
                            continue;
                        }
                        item.render(&clip_painter);
                    }

                    // Render currently drawing item
                    if let Some(item) = &self.drawing_item {
                        item.render(&clip_painter);
                    }

                    // Edit-mode gizmo: vertex handles for multi-point items,
                    // bbox + corner dots + rotation handle for everything else.
                    if editing {
                        if let Some(item) = self.selected.and_then(|i| self.history.items().get(i))
                        {
                            let b = item.bounds();
                            let handle = rotation_handle_pos(b);
                            let hot = self.hover_rotation
                                || matches!(self.drag_state, DragState::RotatingAnnotation { .. });
                            if let Some(verts) = item.editable_vertices() {
                                paint_multi_gizmo(
                                    painter,
                                    b,
                                    handle,
                                    hot,
                                    &verts,
                                    self.active_vertex,
                                    self.hover_vertex,
                                );
                            } else {
                                paint_gizmo(painter, b, handle, hot);
                            }
                        }
                    }

                    // Guidance while editing the vertices of a selected shape.
                    if editing && self.editing_index.is_none() {
                        if self
                            .selected
                            .and_then(|i| self.history.items().get(i))
                            .and_then(|i| i.multi_point_kind())
                            .is_some()
                        {
                            paint_hint_badge(
                                painter,
                                rect,
                                "拖顶点变形 · 双击/回车续画 · Del 删点 · 退格删末点",
                            );
                        }
                    }

                    // Guidance while a multi-click tool is armed (and not editing).
                    if self.current_tool.is_multi_point() && !editing {
                        paint_hint_badge(painter, rect, "单击加顶点 · 双击/回车完成 · Esc 取消");
                    }

                    // Live preview of the multi-click polygon/polyline.
                    if self.current_tool.is_multi_point() && !self.poly_points.is_empty() {
                        let mut pts = self.poly_points.clone();
                        if let Some(p) = pointer_pos {
                            if rect.contains(p) {
                                pts.push(p);
                            }
                        }
                        let stroke_w = self.current_stroke.to_pixels();
                        let preview = if self.current_tool == ToolType::Polygon {
                            AnnotationItem::Polygon {
                                points: pts.clone(),
                                color: self.current_color,
                                stroke_width: stroke_w,
                                filled: false,
                            }
                        } else {
                            AnnotationItem::Polyline {
                                points: pts.clone(),
                                color: self.current_color,
                                stroke_width: stroke_w,
                            }
                        };
                        preview.render(&clip_painter);
                        for p in &pts {
                            clip_painter.circle_filled(*p, 3.0, Color32::WHITE);
                            clip_painter.circle_stroke(
                                *p,
                                3.0,
                                Stroke::new(1.0, self.current_color),
                            );
                        }
                    }

                    // Render inline text input if active
                    if let Some(pos) = self.active_text_pos {
                        let box_size = Vec2::new(240.0, self.font_size + 16.0);
                        let text_box_rect = Rect::from_min_size(pos, box_size);
                        let mut text_ui =
                            ui.new_child(egui::UiBuilder::new().max_rect(text_box_rect));
                        let v = &mut text_ui.style_mut().visuals;
                        v.extreme_bg_color = Color32::from_rgba_unmultiplied(26, 28, 33, 242);
                        v.widgets.noninteractive.bg_stroke =
                            Stroke::new(1.0, Color32::from_white_alpha(42));
                        v.widgets.active.bg_stroke =
                            Stroke::new(1.5, Color32::from_rgb(24, 144, 255));
                        v.selection.bg_fill = Color32::from_rgba_unmultiplied(24, 144, 255, 90);
                        let response = text_ui.add(
                            egui::TextEdit::singleline(&mut self.active_text_buffer)
                                .font(FontId::proportional(self.font_size))
                                .margin(Vec2::new(8.0, 6.0))
                                .text_color(self.current_color)
                                .hint_text(
                                    RichText::new("输入文字，回车确认")
                                        .size((self.font_size * 0.8).max(11.0))
                                        .color(Color32::from_rgb(148, 155, 170)),
                                ),
                        );
                        response.request_focus();
                        if response.lost_focus() || ctx.input(|i| i.key_pressed(Key::Enter)) {
                            self.commit_active_text();
                        }
                    }

                    // 7. Rich Toolbar
                    if self.drag_state == DragState::Idle && self.drawing_item.is_none() {
                        let prev_tool = self.current_tool;
                        let mut state = ToolbarState {
                            tool: &mut self.current_tool,
                            color: &mut self.current_color,
                            stroke: &mut self.current_stroke,
                            filled: &mut self.filled,
                            font_size: &mut self.font_size,
                            mosaic_block: &mut self.mosaic_block,
                            blur_radius: &mut self.blur_radius,
                            position: &mut self.toolbar_pos,
                            family_tool: &mut self.family_tool,
                            badge_square: &mut self.badge_square,
                            can_undo: self.history.can_undo(),
                            can_redo: self.history.can_redo(),
                            can_clear: !self.history.is_empty(),
                        };
                        let action = Toolbar::render(ui, rect, screen_rect.size(), &mut state);

                        // Leaving the text tool commits whatever was typed.
                        if prev_tool == ToolType::Text && self.current_tool != ToolType::Text {
                            self.commit_active_text();
                        }
                        // Switching away finalizes any in-progress polygon.
                        if prev_tool.is_multi_point() && self.current_tool != prev_tool {
                            self.commit_poly(prev_tool);
                        }
                        self.remember_family();

                        match action {
                            Some(ToolbarAction::Pin) => self.handle_pin(),
                            Some(ToolbarAction::Copy) => self.handle_copy(),
                            Some(ToolbarAction::Save) => self.handle_save(),
                            Some(ToolbarAction::Cancel) => std::process::exit(0),
                            Some(ToolbarAction::Undo) => {
                                self.history.undo();
                                self.selected = None;
                                self.reset_draft();
                            }
                            Some(ToolbarAction::Redo) => {
                                self.history.redo();
                                self.selected = None;
                                self.reset_draft();
                            }
                            Some(ToolbarAction::Clear) => {
                                self.history.clear();
                                self.selected = None;
                                self.reset_draft();
                            }
                            None => {}
                        }
                    }
                } else {
                    // No selection yet: Dim entire screen slightly
                    painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(80));

                    // Show Magnifier / Loupe following cursor
                    if let Some(pos) = pointer_pos {
                        Magnifier::render(
                            painter,
                            pos,
                            screen_rect.size(),
                            &self.captured_image,
                            self.scale_factor,
                        );
                    }
                }

                // 8. Handle Keyboard Shortcuts
                ctx.input(|i| {
                    let editing = self.active_text_pos.is_some();
                    let cmd = i.modifiers.matches_logically(Modifiers::COMMAND)
                        || i.modifiers.matches_logically(Modifiers::CTRL);

                    if i.key_pressed(Key::Escape) {
                        if editing {
                            self.commit_active_text();
                        } else if !self.poly_points.is_empty() || self.editing_index.is_some() {
                            // Canceling a resumed edit restores the original shape.
                            let was_resuming = self.editing_index.is_some();
                            self.reset_draft();
                            if was_resuming {
                                self.current_tool = ToolType::Select;
                            }
                        } else if self.selected.is_some() {
                            self.selected = None;
                            self.active_vertex = None;
                        } else if self.current_tool != ToolType::Select {
                            self.current_tool = ToolType::Select;
                        } else {
                            std::process::exit(0);
                        }
                    }

                    // While typing, the TextEdit owns every other key (incl. Enter).
                    if editing {
                        return;
                    }

                    if !cmd
                        && (i.key_pressed(Key::P) || i.key_pressed(Key::F3))
                        && self.selection.is_some()
                    {
                        self.handle_pin();
                    }

                    if cmd {
                        if i.modifiers.shift && i.key_pressed(Key::Z) {
                            self.history.redo();
                            self.selected = None;
                            self.reset_draft();
                        } else if i.key_pressed(Key::Z) {
                            self.history.undo();
                            self.selected = None;
                            self.reset_draft();
                        } else if i.key_pressed(Key::Y) {
                            self.history.redo();
                            self.selected = None;
                            self.reset_draft();
                        } else if i.key_pressed(Key::C) {
                            self.handle_copy();
                        } else if i.key_pressed(Key::S) {
                            self.handle_save();
                        }
                        return;
                    }

                    if i.key_pressed(Key::Enter) {
                        if self.current_tool.is_multi_point() && !self.poly_points.is_empty() {
                            self.commit_poly(self.current_tool);
                        } else if let Some(v) = self.resume_target_vertex() {
                            // Enter on a selected polyline/polygon continues it
                            // from the active vertex, or the last one by default.
                            self.continue_from_vertex(v);
                        } else {
                            self.handle_copy();
                        }
                        return;
                    }

                    if self.selection.is_none() || i.modifiers.any() {
                        return;
                    }

                    let tool = if i.key_pressed(Key::V) {
                        Some(ToolType::Select)
                    } else if i.key_pressed(Key::R) {
                        Some(ToolType::Rectangle)
                    } else if i.key_pressed(Key::U) {
                        Some(ToolType::RoundedRect)
                    } else if i.key_pressed(Key::O) {
                        Some(ToolType::Ellipse)
                    } else if i.key_pressed(Key::G) {
                        Some(ToolType::Diamond)
                    } else if i.key_pressed(Key::Y) {
                        Some(ToolType::Polygon)
                    } else if i.key_pressed(Key::A) {
                        Some(ToolType::Arrow)
                    } else if i.key_pressed(Key::J) {
                        Some(ToolType::DoubleArrow)
                    } else if i.key_pressed(Key::L) {
                        Some(ToolType::Line)
                    } else if i.key_pressed(Key::K) {
                        Some(ToolType::Polyline)
                    } else if i.key_pressed(Key::D) {
                        Some(ToolType::Pen)
                    } else if i.key_pressed(Key::H) {
                        Some(ToolType::Highlighter)
                    } else if i.key_pressed(Key::M) {
                        Some(ToolType::Mosaic)
                    } else if i.key_pressed(Key::B) {
                        Some(ToolType::Blur)
                    } else if i.key_pressed(Key::T) {
                        Some(ToolType::Text)
                    } else if i.key_pressed(Key::N) {
                        Some(ToolType::StepBadge)
                    } else {
                        None
                    };
                    if let Some(t) = tool {
                        if self.current_tool.is_multi_point() && t != self.current_tool {
                            self.commit_poly(self.current_tool);
                        }
                        if self.current_tool == ToolType::Text && t != ToolType::Text {
                            self.commit_active_text();
                        }
                        self.current_tool = t;
                        self.remember_family();
                    }

                    if i.key_pressed(Key::F) && self.current_tool.can_fill() {
                        self.filled = !self.filled;
                    }
                    if self.current_tool == ToolType::Text {
                        if i.key_pressed(Key::OpenBracket) {
                            self.font_size = (self.font_size - 2.0).max(FONT_MIN);
                        } else if i.key_pressed(Key::CloseBracket) {
                            self.font_size = (self.font_size + 2.0).min(FONT_MAX);
                        }
                    }
                    if i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace) {
                        let backspace = i.key_pressed(Key::Backspace);
                        if self.current_tool.is_multi_point() && !self.poly_points.is_empty() {
                            // While drawing/resuming, step back one drafted vertex.
                            self.poly_points.pop();
                            self.poly_last_click = None;
                        } else if let Some(index) = self.selected {
                            let multi = self
                                .history
                                .items()
                                .get(index)
                                .and_then(|it| it.multi_point_kind())
                                .is_some();
                            if let Some(v) = self.active_vertex {
                                if let Some(item) = self.history.item_mut(index) {
                                    item.remove_vertex(v);
                                }
                                self.active_vertex = None;
                            } else if multi && backspace {
                                // Quick "undo the last point" on a selected shape.
                                if let Some(item) = self.history.item_mut(index) {
                                    if let Some(n) = item.editable_vertices().map(|v| v.len()) {
                                        if n > 0 {
                                            item.remove_vertex(n - 1);
                                        }
                                    }
                                }
                            } else {
                                self.delete_selected();
                            }
                        } else if !self.history.is_empty() {
                            self.history.clear();
                        }
                    }
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::App as _;

    #[test]
    fn rapid_distant_clicks_keep_adding_vertices() {
        // Regression: egui's time-only double-click used to swallow the second
        // vertex, so polygons/polylines could never be drawn.
        let a = Pos2::new(100.0, 100.0);
        let b = Pos2::new(220.0, 160.0);
        assert!(!poly_click_finishes(None, 1.0, a));
        assert!(
            !poly_click_finishes(Some((1.0, a)), 1.1, b),
            "fast but distant click must add a vertex"
        );
        assert!(!poly_click_finishes(Some((1.0, a)), 1.35, b));
    }

    #[test]
    fn quick_same_spot_click_finishes() {
        let a = Pos2::new(100.0, 100.0);
        assert!(poly_click_finishes(
            Some((1.0, a)),
            1.2,
            Pos2::new(103.0, 102.0)
        ));
        assert!(
            !poly_click_finishes(Some((1.0, a)), 1.6, a),
            "slow click adds a vertex"
        );
        assert!(!poly_click_finishes(
            Some((1.0, a)),
            1.1,
            Pos2::new(140.0, 100.0)
        ));
    }

    fn drive(
        app: &mut OverlayApp,
        ctx: &egui::Context,
        time: f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut frame = eframe::Frame::_new_kittest();
        ctx.run(raw, |ctx| app.update(ctx, &mut frame))
    }

    fn moved(pos: Pos2) -> egui::Event {
        egui::Event::PointerMoved(pos)
    }

    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    fn key(k: Key) -> egui::Event {
        egui::Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }
    }

    fn click(app: &mut OverlayApp, ctx: &egui::Context, t: &mut f64, pos: Pos2, gap: f64) {
        *t += gap;
        drive(app, ctx, *t, vec![moved(pos), button(pos, true)]);
        *t += 0.02;
        drive(app, ctx, *t, vec![moved(pos), button(pos, false)]);
    }

    fn drag(app: &mut OverlayApp, ctx: &egui::Context, t: &mut f64, from: Pos2, to: Pos2) {
        *t += 0.5;
        drive(app, ctx, *t, vec![moved(from), button(from, true)]);
        *t += 0.1;
        drive(app, ctx, *t, vec![moved(to), button(to, true)]);
        *t += 0.1;
        drive(app, ctx, *t, vec![moved(to), button(to, false)]);
    }

    fn seeded_app() -> (OverlayApp, egui::Context, f64) {
        let img = RgbaImage::from_pixel(800, 600, image::Rgba([40, 40, 40, 255]));
        let mut app = OverlayApp::new(img, 1.0, (0.0, 0.0));
        let ctx = egui::Context::default();
        let mut t = 1.0;
        drag(
            &mut app,
            &ctx,
            &mut t,
            Pos2::new(100.0, 100.0),
            Pos2::new(500.0, 400.0),
        );
        assert!(app.selection.is_some());
        (app, ctx, t)
    }

    #[test]
    fn full_edit_loop_select_move_rotate_delete() {
        let (mut app, ctx, mut t) = seeded_app();

        app.current_tool = ToolType::Rectangle;
        drag(
            &mut app,
            &ctx,
            &mut t,
            Pos2::new(200.0, 200.0),
            Pos2::new(300.0, 260.0),
        );
        assert_eq!(app.history.len(), 1);

        // Re-select the committed rectangle by clicking its top edge.
        app.current_tool = ToolType::Select;
        click(&mut app, &ctx, &mut t, Pos2::new(250.0, 200.0), 0.5);
        assert_eq!(app.selected, Some(0));

        // Move it by dragging.
        drag(
            &mut app,
            &ctx,
            &mut t,
            Pos2::new(250.0, 200.0),
            Pos2::new(270.0, 220.0),
        );
        let center = app.history.items()[0].bounds().center();
        assert!(
            (center.x - 270.0).abs() < 1.0 && (center.y - 250.0).abs() < 1.0,
            "moved by (20,20), got {center:?}"
        );
        assert_eq!(app.selected, Some(0));

        // Rotate +90 degrees via the handle above the bounds.
        let b = app.history.items()[0].bounds();
        let c = b.center();
        let handle = rotation_handle_pos(b);
        let radius = c.y - handle.y;
        let target = Pos2::new(c.x + radius, c.y);
        drag(&mut app, &ctx, &mut t, handle, target);
        let angle = app.history.items()[0].rotation();
        assert!(
            (angle - std::f32::consts::FRAC_PI_2).abs() < 1e-3,
            "expected 90 deg, got {angle}"
        );

        // Esc deselects, then re-select and Delete removes just this item.
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Escape)]);
        assert_eq!(app.selected, None);
        click(&mut app, &ctx, &mut t, Pos2::new(270.0, 200.0), 0.5);
        assert_eq!(
            app.selected,
            Some(0),
            "rotated outline still grabbable on its turned edge"
        );
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Delete)]);
        assert!(app.history.is_empty());
        assert_eq!(app.selected, None);
    }

    #[test]
    fn rapid_polygon_clicks_accumulate_then_finish() {
        let (mut app, ctx, mut t) = seeded_app();
        app.current_tool = ToolType::Polygon;

        for p in [
            Pos2::new(150.0, 150.0),
            Pos2::new(250.0, 150.0),
            Pos2::new(250.0, 250.0),
        ] {
            click(&mut app, &ctx, &mut t, p, 0.05);
        }
        assert_eq!(
            app.poly_points.len(),
            3,
            "rapid distant clicks must all become vertices"
        );
        assert!(app.history.is_empty(), "nothing committed yet");

        // Quick second click at the same spot finishes the shape.
        click(&mut app, &ctx, &mut t, Pos2::new(250.0, 250.0), 0.05);
        assert!(app.poly_points.is_empty());
        assert_eq!(app.history.len(), 1);
        let AnnotationItem::Polygon { points, .. } = &app.history.items()[0] else {
            panic!("expected a committed polygon");
        };
        assert_eq!(points.len(), 3);

        // The pointer rests on the new polygon: hover-to-edit engages without
        // switching to the arrow tool.
        drive(
            &mut app,
            &ctx,
            t + 0.5,
            vec![moved(Pos2::new(250.0, 250.0))],
        );
        t += 0.5;
        assert_eq!(
            app.selected,
            Some(0),
            "hovering the finished polygon selects it"
        );

        // Moving off the shape releases it; Esc with nothing pending returns to Select.
        drive(
            &mut app,
            &ctx,
            t + 0.5,
            vec![moved(Pos2::new(450.0, 350.0))],
        );
        t += 0.5;
        assert_eq!(
            app.selected, None,
            "hovering empty space releases the shape"
        );
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Escape)]);
        assert_eq!(app.current_tool, ToolType::Select);
    }

    #[test]
    fn commit_poly_drops_vertices_outside_selection() {
        let img = RgbaImage::from_pixel(800, 600, image::Rgba([40, 40, 40, 255]));
        let mut app = OverlayApp::new(img, 1.0, (0.0, 0.0));
        app.selection = Some(Selection::new(
            Pos2::new(100.0, 100.0),
            Pos2::new(500.0, 400.0),
        ));
        app.poly_points = vec![
            Pos2::new(150.0, 200.0),
            Pos2::new(900.0, 200.0),
            Pos2::new(300.0, 200.0),
            Pos2::new(-50.0, 200.0),
        ];
        app.commit_poly(ToolType::Polyline);
        assert_eq!(app.history.len(), 1);
        let AnnotationItem::Polyline { points, .. } = &app.history.items()[0] else {
            panic!("expected a committed polyline");
        };
        assert_eq!(points, &[Pos2::new(150.0, 200.0), Pos2::new(300.0, 200.0)]);

        app.selection = None;
        app.poly_points = vec![Pos2::new(10.0, 10.0), Pos2::new(20.0, 10.0)];
        app.commit_poly(ToolType::Polyline);
        assert_eq!(
            app.history.len(),
            1,
            "without a selection there is nothing to commit into"
        );
        assert!(app.poly_points.is_empty());
    }

    #[test]
    fn shrunk_selection_never_leaks_annotation_pixels_outside() {
        // Regression: collinear polyline vertices committed under a large
        // selection used to keep drawing a screen-spanning line after the
        // selection shrank. Rendering must now clip to the selection rect.
        let (mut app, ctx, mut t) = seeded_app();
        app.current_tool = ToolType::Polyline;

        for p in [
            Pos2::new(120.0, 200.0),
            Pos2::new(300.0, 200.0),
            Pos2::new(480.0, 200.0),
        ] {
            click(&mut app, &ctx, &mut t, p, 0.5);
        }
        assert_eq!(app.poly_points.len(), 3);

        // Switching to Select commits the in-progress polyline.
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::V)]);
        t += 0.5;
        assert_eq!(app.history.len(), 1);

        // Shrink the selection through its bottom-right handle.
        drag(
            &mut app,
            &ctx,
            &mut t,
            Pos2::new(500.0, 400.0),
            Pos2::new(320.0, 300.0),
        );
        let rect = app.selection.unwrap().to_rect();
        assert_eq!(
            rect,
            Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(320.0, 300.0))
        );

        let output = drive(&mut app, &ctx, t + 0.5, vec![]);
        let prims = ctx.tessellate(output.shapes, 1.0);
        let red = Color32::from_rgb(245, 34, 45);
        let allowed = rect.expand(0.5);
        let mut red_vertices = 0usize;
        for prim in &prims {
            let egui::epaint::Primitive::Mesh(mesh) = &prim.primitive else {
                continue;
            };
            let count = mesh.vertices.iter().filter(|v| v.color == red).count();
            if count == 0 {
                continue;
            }
            red_vertices += count;
            // The renderer scissors each primitive to its clip_rect (vertices
            // outside it stay in the mesh), so annotation ink must carry the
            // selection as its clip rect or it leaks across the screen.
            assert!(
                allowed.contains(prim.clip_rect.min) && allowed.contains(prim.clip_rect.max),
                "annotation primitive scissored to {:?}, outside selection {:?}",
                prim.clip_rect,
                rect
            );
        }
        assert!(
            red_vertices > 0,
            "the committed polyline must still render inside the selection"
        );
    }

    /// Draws a polyline by clicking vertices, finishes with Enter, then switches
    /// to Select and clicks a point on the path so the item is selected.
    fn selected_polyline(pts: &[Pos2], grab: Pos2) -> (OverlayApp, egui::Context, f64) {
        let (mut app, ctx, mut t) = seeded_app();
        app.current_tool = ToolType::Polyline;
        for p in pts {
            click(&mut app, &ctx, &mut t, *p, 0.5);
        }
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Enter)]);
        t += 0.5;
        assert_eq!(app.history.len(), 1);
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::V)]);
        t += 0.5;
        click(&mut app, &ctx, &mut t, grab, 0.5);
        assert_eq!(app.selected, Some(0));
        (app, ctx, t)
    }

    fn polyline_points(app: &OverlayApp) -> Vec<Pos2> {
        let AnnotationItem::Polyline { points, .. } = &app.history.items()[0] else {
            panic!(
                "expected a committed polyline, got {:?}",
                app.history.items()[0]
            );
        };
        points.clone()
    }

    #[test]
    fn vertex_drag_reshapes_polyline() {
        let (mut app, ctx, mut t) = selected_polyline(
            &[
                Pos2::new(150.0, 150.0),
                Pos2::new(250.0, 150.0),
                Pos2::new(250.0, 250.0),
            ],
            Pos2::new(200.0, 150.0),
        );
        drag(
            &mut app,
            &ctx,
            &mut t,
            Pos2::new(150.0, 150.0),
            Pos2::new(160.0, 170.0),
        );
        let points = polyline_points(&app);
        assert_eq!(
            points[0],
            Pos2::new(160.0, 170.0),
            "dragged vertex follows the pointer"
        );
        assert_eq!(points.len(), 3, "dragging must not add or drop vertices");
        assert_eq!(app.active_vertex, Some(0));
    }

    #[test]
    fn delete_active_vertex_and_backspace_removes_last() {
        let (mut app, ctx, mut t) = selected_polyline(
            &[
                Pos2::new(150.0, 150.0),
                Pos2::new(250.0, 150.0),
                Pos2::new(250.0, 250.0),
                Pos2::new(350.0, 250.0),
            ],
            Pos2::new(200.0, 150.0),
        );
        // Second click on the last vertex activates it (first already selected).
        click(&mut app, &ctx, &mut t, Pos2::new(350.0, 250.0), 0.5);
        assert_eq!(app.active_vertex, Some(3));
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Delete)]);
        t += 0.5;
        assert_eq!(
            polyline_points(&app).len(),
            3,
            "Delete removes the active vertex"
        );
        assert_eq!(app.active_vertex, None);

        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Backspace)]);
        t += 0.5;
        assert_eq!(
            polyline_points(&app).len(),
            2,
            "Backspace removes the last point"
        );

        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Backspace)]);
        assert_eq!(
            polyline_points(&app).len(),
            2,
            "a polyline keeps at least two points"
        );
        assert_eq!(app.history.len(), 1, "the shape itself is not deleted");
    }

    #[test]
    fn enter_resumes_from_last_vertex_and_extends() {
        let (mut app, ctx, mut t) = selected_polyline(
            &[
                Pos2::new(150.0, 150.0),
                Pos2::new(250.0, 150.0),
                Pos2::new(250.0, 250.0),
            ],
            Pos2::new(200.0, 150.0),
        );
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Enter)]);
        t += 0.5;
        assert_eq!(app.current_tool, ToolType::Polyline);
        assert_eq!(app.editing_index, Some(0));
        assert_eq!(
            app.poly_points.len(),
            3,
            "resume keeps the whole existing path"
        );
        assert!(app.selected.is_none());
        assert_eq!(
            app.history.len(),
            1,
            "original stays in history until committed"
        );

        click(&mut app, &ctx, &mut t, Pos2::new(350.0, 300.0), 0.5);
        assert_eq!(app.poly_points.len(), 4, "a new click extends the path");
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Enter)]);
        assert!(app.editing_index.is_none());
        assert_eq!(
            app.history.len(),
            1,
            "finishing replaces the original in place"
        );
        let points = polyline_points(&app);
        assert_eq!(points.len(), 4);
        assert_eq!(points[3], Pos2::new(350.0, 300.0));
    }

    #[test]
    fn resume_from_middle_vertex_replaces_the_tail() {
        let (mut app, ctx, mut t) = selected_polyline(
            &[
                Pos2::new(150.0, 150.0),
                Pos2::new(250.0, 150.0),
                Pos2::new(250.0, 250.0),
                Pos2::new(350.0, 250.0),
            ],
            Pos2::new(200.0, 150.0),
        );
        // Activate vertex 1, then Enter resumes from it (drops the tail).
        click(&mut app, &ctx, &mut t, Pos2::new(250.0, 150.0), 0.5);
        assert_eq!(app.active_vertex, Some(1));
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Enter)]);
        t += 0.5;
        assert_eq!(
            app.poly_points.len(),
            2,
            "path is kept only up to the chosen vertex"
        );

        click(&mut app, &ctx, &mut t, Pos2::new(300.0, 300.0), 0.5);
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Enter)]);
        let points = polyline_points(&app);
        assert_eq!(
            points,
            vec![
                Pos2::new(150.0, 150.0),
                Pos2::new(250.0, 150.0),
                Pos2::new(300.0, 300.0)
            ]
        );
    }

    #[test]
    fn double_click_vertex_resumes_and_esc_restores_original() {
        let (mut app, ctx, mut t) = selected_polyline(
            &[
                Pos2::new(150.0, 150.0),
                Pos2::new(250.0, 150.0),
                Pos2::new(250.0, 250.0),
            ],
            Pos2::new(200.0, 150.0),
        );
        // Two quick clicks on vertex 1 = double-click -> resume from it.
        click(&mut app, &ctx, &mut t, Pos2::new(250.0, 150.0), 0.5);
        click(&mut app, &ctx, &mut t, Pos2::new(250.0, 150.0), 0.1);
        assert_eq!(
            app.editing_index,
            Some(0),
            "double-clicking a vertex resumes drawing"
        );
        assert_eq!(app.poly_points.len(), 2);
        assert_eq!(app.current_tool, ToolType::Polyline);

        // Esc cancels the resume and restores the untouched original.
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Escape)]);
        assert!(app.editing_index.is_none());
        assert!(app.poly_points.is_empty());
        assert_eq!(app.current_tool, ToolType::Select);
        assert_eq!(app.history.len(), 1);
        assert_eq!(
            polyline_points(&app).len(),
            3,
            "Esc must not lose the original shape"
        );
    }

    #[test]
    fn hover_with_drawing_tool_selects_and_moves_without_arrow() {
        let (mut app, ctx, mut t) = seeded_app();
        app.current_tool = ToolType::Rectangle;
        drag(
            &mut app,
            &ctx,
            &mut t,
            Pos2::new(200.0, 200.0),
            Pos2::new(300.0, 260.0),
        );
        assert_eq!(app.history.len(), 1);
        assert_eq!(app.selected, None);

        // Hovering the rectangle's edge engages editing while Rectangle stays armed.
        drive(
            &mut app,
            &ctx,
            t + 0.5,
            vec![moved(Pos2::new(250.0, 200.0))],
        );
        t += 0.5;
        assert_eq!(
            app.selected,
            Some(0),
            "hover selects without the arrow tool"
        );
        assert_eq!(app.current_tool, ToolType::Rectangle);

        // Dragging the hovered shape moves it.
        drag(
            &mut app,
            &ctx,
            &mut t,
            Pos2::new(250.0, 200.0),
            Pos2::new(270.0, 220.0),
        );
        let center = app.history.items()[0].bounds().center();
        assert!(
            (center.x - 270.0).abs() < 1.0 && (center.y - 250.0).abs() < 1.0,
            "moved by (20,20), got {center:?}"
        );
        assert_eq!(
            app.current_tool,
            ToolType::Rectangle,
            "editing must not switch tools"
        );
    }

    #[test]
    fn hover_polyline_vertex_drag_without_arrow() {
        let (mut app, ctx, mut t) = seeded_app();
        app.current_tool = ToolType::Polyline;
        for p in [
            Pos2::new(150.0, 150.0),
            Pos2::new(250.0, 150.0),
            Pos2::new(250.0, 250.0),
        ] {
            click(&mut app, &ctx, &mut t, p, 0.5);
        }
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Enter)]);
        t += 0.5;
        assert_eq!(app.history.len(), 1);

        // Still on the Polyline tool: hover a vertex and drag to reshape it.
        drive(
            &mut app,
            &ctx,
            t + 0.5,
            vec![moved(Pos2::new(150.0, 150.0))],
        );
        t += 0.5;
        assert_eq!(app.selected, Some(0));
        drag(
            &mut app,
            &ctx,
            &mut t,
            Pos2::new(150.0, 150.0),
            Pos2::new(160.0, 170.0),
        );
        let points = polyline_points(&app);
        assert_eq!(
            points[0],
            Pos2::new(160.0, 170.0),
            "hovered vertex follows the pointer"
        );
        assert_eq!(points.len(), 3);
        assert_eq!(app.active_vertex, Some(0));
        assert_eq!(app.current_tool, ToolType::Polyline);
    }

    #[test]
    fn hover_then_delete_removes_shape_without_arrow() {
        let (mut app, ctx, mut t) = seeded_app();
        app.current_tool = ToolType::Rectangle;
        drag(
            &mut app,
            &ctx,
            &mut t,
            Pos2::new(200.0, 200.0),
            Pos2::new(300.0, 260.0),
        );
        assert_eq!(app.history.len(), 1);

        drive(
            &mut app,
            &ctx,
            t + 0.5,
            vec![moved(Pos2::new(250.0, 200.0))],
        );
        t += 0.5;
        assert_eq!(app.selected, Some(0));
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Delete)]);
        assert!(app.history.is_empty(), "Delete removes the hovered shape");
        assert_eq!(app.current_tool, ToolType::Rectangle);
    }

    #[test]
    fn hover_empty_space_then_drag_still_draws_new_shape() {
        let (mut app, ctx, mut t) = seeded_app();
        app.current_tool = ToolType::Rectangle;
        drag(
            &mut app,
            &ctx,
            &mut t,
            Pos2::new(200.0, 200.0),
            Pos2::new(300.0, 260.0),
        );
        assert_eq!(app.history.len(), 1);

        // Hovering empty space releases any selection; a drag there draws anew.
        drive(
            &mut app,
            &ctx,
            t + 0.5,
            vec![moved(Pos2::new(400.0, 350.0))],
        );
        t += 0.5;
        assert_eq!(app.selected, None);
        drag(
            &mut app,
            &ctx,
            &mut t,
            Pos2::new(400.0, 350.0),
            Pos2::new(450.0, 380.0),
        );
        assert_eq!(
            app.history.len(),
            2,
            "drawing on empty space adds a second shape"
        );
        assert_eq!(app.current_tool, ToolType::Rectangle);
    }

    #[test]
    fn hover_gizmo_renders_only_while_hovering() {
        let (mut app, ctx, mut t) = seeded_app();
        app.current_tool = ToolType::Polyline;
        for p in [
            Pos2::new(150.0, 150.0),
            Pos2::new(250.0, 150.0),
            Pos2::new(250.0, 250.0),
        ] {
            click(&mut app, &ctx, &mut t, p, 0.5);
        }
        drive(&mut app, &ctx, t + 0.5, vec![key(Key::Enter)]);
        t += 0.5;

        let accent = Color32::from_rgb(24, 144, 255);
        let vertex = Pos2::new(150.0, 150.0);
        let gizmo_at_vertex = |out: egui::FullOutput| -> bool {
            ctx.tessellate(out.shapes, 1.0).iter().any(|prim| {
                let egui::epaint::Primitive::Mesh(mesh) = &prim.primitive else {
                    return false;
                };
                mesh.vertices
                    .iter()
                    .any(|v| v.color == accent && v.pos.distance(vertex) <= 10.0)
            })
        };

        // Empty-space hover: no gizmo ink on the vertex.
        let off = drive(
            &mut app,
            &ctx,
            t + 0.5,
            vec![moved(Pos2::new(400.0, 350.0))],
        );
        t += 0.5;
        assert_eq!(app.selected, None);
        assert!(
            !gizmo_at_vertex(off),
            "no vertex handle when not hovering the shape"
        );

        // Hovering the vertex: the gizmo paints an accent handle ring there.
        let on = drive(&mut app, &ctx, t + 0.5, vec![moved(vertex)]);
        assert_eq!(app.selected, Some(0));
        assert!(
            gizmo_at_vertex(on),
            "hovering paints the vertex gizmo without the arrow tool"
        );
    }
}
