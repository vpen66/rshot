use crate::annotation::{
    AnnotationItem, HistoryManager, StrokeWidth, ToolType, COLOR_PRESETS,
};
use crate::magnifier::Magnifier;
use crate::state::{DragState, ResizeHandle, Selection};
use crate::toolbar::{Toolbar, ToolbarAction};
use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, Key, Modifiers, Pos2, Rect, Stroke, StrokeKind,
    Vec2,
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
    history: HistoryManager,
    drawing_item: Option<AnnotationItem>,
    draw_start_pos: Option<Pos2>,
    pen_points: Vec<Pos2>,
    active_text_pos: Option<Pos2>,
    active_text_buffer: String,
}

impl OverlayApp {
    pub fn new(
        captured_image: RgbaImage,
        scale_factor: f32,
        monitor_origin: (f32, f32),
    ) -> Self {
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
            history: HistoryManager::new(),
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

        let mut cropped = ScreenCapture::crop_region(&self.captured_image, min_x, min_y, width, height).ok()?;

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
                Ok(path) => info!("Screenshot (with annotations) successfully saved to: {}", path.display()),
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
                    font_size: self.current_stroke.to_pixels() * 4.0 + 10.0,
                });
            }
            self.active_text_buffer.clear();
        }
    }
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
                let double_clicked = ctx.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary));
                let pointer_over_widget = ctx.wants_pointer_input();

                if double_clicked && !pointer_over_widget && self.selection.is_some() && self.current_tool == ToolType::Select {
                    self.handle_copy();
                    return;
                }

                // Hit test resize handles (only active in Select mode)
                if self.current_tool == ToolType::Select {
                    if let (Some(sel), Some(pos)) = (self.selection, pointer_pos) {
                        self.hovered_handle = sel.hit_test_handle(pos, self.handle_radius + 4.0);
                    } else {
                        self.hovered_handle = None;
                    }
                } else {
                    self.hovered_handle = None;
                }

                // Set cursor icon
                if let Some(handle) = self.hovered_handle {
                    let cursor = match handle {
                        ResizeHandle::TopLeft | ResizeHandle::BottomRight => CursorIcon::ResizeNwSe,
                        ResizeHandle::TopRight | ResizeHandle::BottomLeft => CursorIcon::ResizeNeSw,
                        ResizeHandle::TopCenter | ResizeHandle::BottomCenter => CursorIcon::ResizeVertical,
                        ResizeHandle::LeftCenter | ResizeHandle::RightCenter => CursorIcon::ResizeHorizontal,
                    };
                    ctx.set_cursor_icon(cursor);
                } else if let (Some(sel), Some(pos)) = (self.selection, pointer_pos) {
                    if self.current_tool == ToolType::Select {
                        if sel.to_rect().contains(pos) {
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
                        if self.current_tool == ToolType::Select {
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
                                    } else if let Some(sel) = self.selection {
                                        if sel.to_rect().contains(pos) {
                                            self.drag_state = DragState::Moving {
                                                original_rect: sel.to_rect(),
                                                grab_offset: pos - sel.min,
                                            };
                                        } else {
                                            self.drag_state = DragState::Creating { start: pos };
                                        }
                                    } else {
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
                            }
                        } else if let Some(sel) = self.selection {
                            // Annotation drawing inside selection
                            let stroke_w = self.current_stroke.to_pixels();
                            let color = self.current_color;

                            if sel.to_rect().contains(pos) {
                                if self.draw_start_pos.is_none() {
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
                                            });
                                        }
                                        ToolType::Ellipse => {
                                            self.drawing_item = Some(AnnotationItem::Ellipse {
                                                rect: Rect::from_two_pos(start, pos),
                                                color,
                                                stroke_width: stroke_w,
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
                                                block_size: 10,
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
                    }
                    self.draw_start_pos = None;
                    self.pen_points.clear();
                }

                // Handle single clicks for StepBadge or Text
                let pointer_clicked = ctx.input(|i| i.pointer.primary_clicked());
                if pointer_clicked && !pointer_over_widget {
                    if let (Some(sel), Some(pos)) = (self.selection, pointer_pos) {
                        if sel.to_rect().contains(pos) {
                            match self.current_tool {
                                ToolType::StepBadge => {
                                    self.history.push(AnnotationItem::StepBadge {
                                        pos,
                                        number: self.history.step_counter,
                                        color: self.current_color,
                                        radius: (self.current_stroke.to_pixels() * 3.0).max(12.0),
                                    });
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
                        Rect::from_min_max(screen_rect.left_top(), Pos2::new(screen_rect.right(), rect.top())),
                        0.0,
                        mask_color,
                    );
                    // Dim bottom
                    painter.rect_filled(
                        Rect::from_min_max(Pos2::new(screen_rect.left(), rect.bottom()), screen_rect.right_bottom()),
                        0.0,
                        mask_color,
                    );
                    // Dim left
                    painter.rect_filled(
                        Rect::from_min_max(Pos2::new(screen_rect.left(), rect.top()), Pos2::new(rect.left(), rect.bottom())),
                        0.0,
                        mask_color,
                    );
                    // Dim right
                    painter.rect_filled(
                        Rect::from_min_max(Pos2::new(rect.right(), rect.top()), Pos2::new(screen_rect.right(), rect.bottom())),
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

                    // Dimension badge (e.g. "800 x 600")
                    let (phys_w, phys_h) = (
                        (sel.width() * self.scale_factor).round() as u32,
                        (sel.height() * self.scale_factor).round() as u32,
                    );
                    let badge_text = format!("{phys_w} × {phys_h}");
                    let badge_pos = Pos2::new(rect.left() + 4.0, (rect.top() - 22.0).max(4.0));
                    painter.rect(
                        Rect::from_min_size(badge_pos, Vec2::new(badge_text.len() as f32 * 7.5 + 12.0, 18.0)),
                        3.0,
                        Color32::from_black_alpha(200),
                        Stroke::NONE,
                        StrokeKind::Inside,
                    );
                    painter.text(
                        Pos2::new(badge_pos.x + 6.0, badge_pos.y + 2.0),
                        Align2::LEFT_TOP,
                        &badge_text,
                        FontId::monospace(11.0),
                        Color32::WHITE,
                    );

                    // 6. Render All Committed Annotations
                    for item in self.history.items() {
                        item.render(painter);
                    }

                    // Render currently drawing item
                    if let Some(item) = &self.drawing_item {
                        item.render(painter);
                    }

                    // Render inline text input if active
                    if let Some(pos) = self.active_text_pos {
                        let text_box_rect = Rect::from_min_size(pos, Vec2::new(180.0, 28.0));
                        let mut text_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_box_rect));
                        let response = text_ui.add(
                            egui::TextEdit::singleline(&mut self.active_text_buffer)
                                .text_color(self.current_color)
                                .hint_text("输入文字... (回车确认)"),
                        );
                        response.request_focus();
                        if response.lost_focus() || ctx.input(|i| i.key_pressed(Key::Enter)) {
                            self.commit_active_text();
                        }
                    }

                    // 7. Rich Toolbar
                    if self.drag_state == DragState::Idle && self.drawing_item.is_none() {
                        let can_undo = self.history.can_undo();
                        let can_redo = self.history.can_redo();

                        if let Some(action) = Toolbar::render(
                            ui,
                            rect,
                            screen_rect.size(),
                            &mut self.current_tool,
                            &mut self.current_color,
                            &mut self.current_stroke,
                            can_undo,
                            can_redo,
                        ) {
                            match action {
                                ToolbarAction::Pin => self.handle_pin(),
                                ToolbarAction::Copy => self.handle_copy(),
                                ToolbarAction::Save => self.handle_save(),
                                ToolbarAction::Cancel => std::process::exit(0),
                                ToolbarAction::Undo => {
                                    self.history.undo();
                                }
                                ToolbarAction::Redo => {
                                    self.history.redo();
                                }
                            }
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
                    if i.key_pressed(Key::Escape) {
                        if self.current_tool != ToolType::Select {
                            self.current_tool = ToolType::Select;
                        } else {
                            std::process::exit(0);
                        }
                    }

                    if (i.key_pressed(Key::P) || i.key_pressed(Key::F3)) && self.selection.is_some() {
                        self.handle_pin();
                    }

                    let cmd = i.modifiers.matches_logically(Modifiers::COMMAND)
                        || i.modifiers.matches_logically(Modifiers::CTRL);

                    if cmd {
                        if i.modifiers.shift && i.key_pressed(Key::Z) {
                            self.history.redo();
                        } else if i.key_pressed(Key::Z) {
                            self.history.undo();
                        } else if i.key_pressed(Key::Y) {
                            self.history.redo();
                        } else if i.key_pressed(Key::C) {
                            self.handle_copy();
                        } else if i.key_pressed(Key::S) {
                            self.handle_save();
                        }
                    } else if i.key_pressed(Key::Enter) {
                        self.handle_copy();
                    }
                });
            });
    }
}
