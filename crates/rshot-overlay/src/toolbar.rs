use crate::annotation::{StrokeWidth, ToolType, COLOR_PRESETS};
use egui::{
    Color32, Pos2, Rect, RichText, Stroke, StrokeKind, Ui, Vec2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    Copy,
    Save,
    Cancel,
    Undo,
    Redo,
    Pin,
}

pub struct Toolbar;

impl Toolbar {
    /// Renders the rich 2-row action toolbar near the selection rectangle.
    pub fn render(
        ui: &mut Ui,
        selection_rect: Rect,
        screen_size: Vec2,
        current_tool: &mut ToolType,
        current_color: &mut Color32,
        current_stroke: &mut StrokeWidth,
        can_undo: bool,
        can_redo: bool,
    ) -> Option<ToolbarAction> {
        let mut triggered_action = None;

        let toolbar_width = 580.0;
        let show_subbar = *current_tool != ToolType::Select && *current_tool != ToolType::Mosaic;
        let toolbar_height = if show_subbar { 68.0 } else { 38.0 };
        let spacing = 8.0;

        // Position toolbar below selection by default; if not enough space, place above
        let mut pos_x = selection_rect.right() - toolbar_width;
        if pos_x < 10.0 {
            pos_x = 10.0;
        }
        if pos_x + toolbar_width > screen_size.x - 10.0 {
            pos_x = screen_size.x - toolbar_width - 10.0;
        }

        let mut pos_y = selection_rect.bottom() + spacing;
        if pos_y + toolbar_height > screen_size.y - 10.0 {
            pos_y = selection_rect.top() - toolbar_height - spacing;
            if pos_y < 10.0 {
                pos_y = selection_rect.bottom() - toolbar_height - spacing;
            }
        }

        let toolbar_rect = Rect::from_min_size(
            Pos2::new(pos_x, pos_y),
            Vec2::new(toolbar_width, toolbar_height),
        );

        // Render background panel
        let painter = ui.painter();
        painter.rect(
            toolbar_rect,
            6.0,
            Color32::from_black_alpha(235),
            Stroke::new(1.0, Color32::from_gray(80)),
            StrokeKind::Middle,
        );

        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(toolbar_rect));
        child_ui.vertical(|ui| {
            // Row 1: Tools & Actions
            ui.horizontal_centered(|ui| {
                ui.add_space(6.0);

                let tools = [
                    ToolType::Select,
                    ToolType::Rectangle,
                    ToolType::Ellipse,
                    ToolType::Arrow,
                    ToolType::Line,
                    ToolType::Pen,
                    ToolType::Highlighter,
                    ToolType::Mosaic,
                    ToolType::Text,
                    ToolType::StepBadge,
                ];

                for tool in tools {
                    let is_active = *current_tool == tool;
                    let text = RichText::new(tool.icon())
                        .size(14.0)
                        .color(if is_active {
                            Color32::from_rgb(24, 144, 255)
                        } else {
                            Color32::WHITE
                        });

                    let btn = egui::Button::new(text)
                        .fill(if is_active {
                            Color32::from_black_alpha(100)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(if is_active {
                            Stroke::new(1.5, Color32::from_rgb(24, 144, 255))
                        } else {
                            Stroke::NONE
                        });

                    if ui.add(btn).on_hover_text(tool.name()).clicked() {
                        *current_tool = tool;
                    }
                }

                ui.separator();

                // Undo button
                let undo_color = if can_undo { Color32::WHITE } else { Color32::GRAY };
                if ui
                    .add_enabled(can_undo, egui::Button::new(RichText::new("⮌").size(14.0).color(undo_color)))
                    .on_hover_text("撤销 (Ctrl+Z)")
                    .clicked()
                {
                    triggered_action = Some(ToolbarAction::Undo);
                }

                // Redo button
                let redo_color = if can_redo { Color32::WHITE } else { Color32::GRAY };
                if ui
                    .add_enabled(can_redo, egui::Button::new(RichText::new("⮎").size(14.0).color(redo_color)))
                    .on_hover_text("重做 (Ctrl+Y)")
                    .clicked()
                {
                    triggered_action = Some(ToolbarAction::Redo);
                }

                ui.separator();

                // Pin
                if ui
                    .button(RichText::new("📌").size(14.0).color(Color32::WHITE))
                    .on_hover_text("置顶贴图 (P / F3)")
                    .clicked()
                {
                    triggered_action = Some(ToolbarAction::Pin);
                }

                // Copy
                if ui
                    .button(RichText::new("📋").size(14.0).color(Color32::WHITE))
                    .on_hover_text("复制截图 (Enter / Ctrl+C)")
                    .clicked()
                {
                    triggered_action = Some(ToolbarAction::Copy);
                }

                // Save
                if ui
                    .button(RichText::new("💾").size(14.0).color(Color32::WHITE))
                    .on_hover_text("保存截图 (Ctrl+S)")
                    .clicked()
                {
                    triggered_action = Some(ToolbarAction::Save);
                }

                // Cancel
                if ui
                    .button(RichText::new("❌").size(13.0).color(Color32::from_rgb(255, 100, 100)))
                    .on_hover_text("取消 (Esc)")
                    .clicked()
                {
                    triggered_action = Some(ToolbarAction::Cancel);
                }
            });

            // Row 2: Secondary Tool Properties (Color & Stroke Width)
            if show_subbar {
                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);

                    // Colors
                    for &color in COLOR_PRESETS {
                        let is_active = *current_color == color;
                        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(14.0), egui::Sense::click());
                        let painter = ui.painter();

                        painter.rect_filled(rect, 2.0, color);
                        if is_active {
                            painter.rect_stroke(rect.expand(2.0), 3.0, Stroke::new(2.0, Color32::WHITE), StrokeKind::Outside);
                        } else {
                            painter.rect_stroke(rect, 2.0, Stroke::new(0.5, Color32::from_gray(100)), StrokeKind::Inside);
                        }

                        if resp.clicked() {
                            *current_color = color;
                        }
                    }

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // Stroke widths (Thin, Medium, Thick)
                    let strokes = [
                        (StrokeWidth::Thin, "细 (2px)", 3.0),
                        (StrokeWidth::Medium, "中 (4px)", 5.0),
                        (StrokeWidth::Thick, "粗 (8px)", 8.0),
                    ];

                    for (sw, label, dot_r) in strokes {
                        let is_active = *current_stroke == sw;
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), egui::Sense::click());
                        let painter = ui.painter();

                        painter.circle_filled(
                            rect.center(),
                            dot_r,
                            if is_active { Color32::from_rgb(24, 144, 255) } else { Color32::GRAY },
                        );

                        if is_active {
                            painter.circle_stroke(rect.center(), dot_r + 3.0, Stroke::new(1.5, Color32::WHITE));
                        }

                        if resp.on_hover_text(label).clicked() {
                            *current_stroke = sw;
                        }
                    }
                });
            }
        });

        triggered_action
    }
}
