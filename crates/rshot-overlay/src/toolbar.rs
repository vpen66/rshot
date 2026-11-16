//! Compact single-row floating toolbar (after the reference design): a six-dot
//! drag grip on the left, tool icons each paired with a dropdown caret that opens
//! a contextual options popover, vertical separators, and output actions on the
//! right. The whole panel can be dragged by its grip and double-click reset.

use crate::annotation::{COLOR_PRESETS, FAMILIES, StrokeWidth, ToolFamily, ToolType};
use crate::icons::{self, Icon};
use egui::{
    Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    Copy,
    Save,
    Cancel,
    Undo,
    Redo,
    Clear,
    Pin,
}

/// Mutable toolbar state borrowed from the overlay app for one frame.
pub struct ToolbarState<'a> {
    pub tool: &'a mut ToolType,
    pub color: &'a mut Color32,
    pub stroke: &'a mut StrokeWidth,
    pub filled: &'a mut bool,
    pub font_size: &'a mut f32,
    pub mosaic_block: &'a mut u32,
    pub blur_radius: &'a mut u32,
    /// User-dragged top-left of the panel; `None` keeps the automatic placement.
    pub position: &'a mut Option<Pos2>,
    /// Last-chosen variant per family, so a group icon remembers its form.
    pub family_tool: &'a mut [ToolType; FAMILIES.len()],
    /// Whether step badges render as squares instead of circles.
    pub badge_square: &'a mut bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub can_clear: bool,
}

// Theme (airsnip-inspired dark palette, accent kept in sync with the selection border).
// Translucent tones are authored unmultiplied; `rgba` premultiplies at compile time
// because epaint's unmultiplied constructors are not const. Passing raw bright
// channels straight to from_rgba_premultiplied would render opaque white/blue.
const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    const fn pm(c: u8, a: u8) -> u8 {
        ((c as u16 * a as u16 + 127) / 255) as u8
    }
    Color32::from_rgba_premultiplied(pm(r, a), pm(g, a), pm(b, a), a)
}
const PANEL_BG: Color32 = rgba(26, 28, 33, 242);
const PANEL_BORDER: Color32 = rgba(255, 255, 255, 42);
const ICON_COLOR: Color32 = Color32::from_rgb(233, 237, 245);
const ICON_DIM: Color32 = rgba(233, 237, 245, 88);
const ACCENT: Color32 = Color32::from_rgb(24, 144, 255);
const ACCENT_SOFT: Color32 = rgba(24, 144, 255, 72);
const HOVER_BG: Color32 = rgba(255, 255, 255, 20);
const PRESS_BG: Color32 = rgba(255, 255, 255, 34);
const DANGER: Color32 = Color32::from_rgb(255, 99, 88);
const DANGER_HOVER: Color32 = rgba(36, 14, 12, 36);
const DIVIDER: Color32 = rgba(255, 255, 255, 34);
const LABEL: Color32 = Color32::from_rgb(233, 237, 245);
const LABEL_DIM: Color32 = Color32::from_rgb(148, 155, 170);

// Layout metrics. The panel is a fixed-size single row so dragging is trivial.
const BTN: f32 = 26.0;
const CARET: f32 = 11.0;
const ACTION: f32 = 26.0;
const GRIP: f32 = 18.0;
const SEP: f32 = 8.0;
const GAP: f32 = 2.0;
const PAD_X: f32 = 8.0;
const PAD_Y: f32 = 5.0;
const ROW_H: f32 = 34.0;
const ROW2_ITEM: f32 = 24.0;
const SWATCH: f32 = 20.0;
const RADIUS: f32 = 10.0;
const MARGIN: f32 = 8.0;
const SPACING: f32 = 10.0;

/// Panel height (single row plus vertical padding).
pub const HEIGHT: f32 = PAD_Y * 2.0 + ROW_H;

/// 18 items (grip, select, 7 family groups, 2 separators, 7 actions) => 17 gaps.
const ROW_W: f32 =
    2.0 * PAD_X + GRIP + BTN + 7.0 * (BTN + CARET) + 2.0 * SEP + 7.0 * ACTION + 17.0 * GAP;

pub const FONT_MIN: f32 = 10.0;
pub const FONT_MAX: f32 = 72.0;
pub const MOSAIC_LEVELS: [(u32, &str); 3] =
    [(6, "细腻 · 6px"), (10, "标准 · 10px"), (16, "粗犷 · 16px")];
pub const BLUR_LEVELS: [(u32, &str); 3] =
    [(4, "轻度 · 4px"), (8, "标准 · 8px"), (14, "强烈 · 14px")];

/// Places the panel right-aligned below the selection, flipping above (or inside
/// the selection's bottom edge) when there is no room, always fully on screen.
pub fn panel_rect(selection: Rect, screen: Vec2, width: f32, height: f32) -> Rect {
    let x = (selection.right() - width).clamp(MARGIN, (screen.x - width - MARGIN).max(MARGIN));
    let mut y = selection.bottom() + SPACING;
    if y + height > screen.y - MARGIN {
        y = selection.top() - height - SPACING;
        if y < MARGIN {
            y = (selection.bottom() - height - SPACING).max(MARGIN);
        }
    }
    Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height))
}

/// Keeps a dragged panel fully on screen.
fn clamp_pos(p: Pos2, screen: Vec2, w: f32, h: f32) -> Pos2 {
    Pos2::new(
        p.x.clamp(MARGIN, (screen.x - w - MARGIN).max(MARGIN)),
        p.y.clamp(MARGIN, (screen.y - h - MARGIN).max(MARGIN)),
    )
}

fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn tool_icon(tool: ToolType) -> Icon {
    match tool {
        ToolType::Select => Icon::Select,
        ToolType::Rectangle => Icon::Rect,
        ToolType::RoundedRect => Icon::RoundedRect,
        ToolType::Ellipse => Icon::Ellipse,
        ToolType::Diamond => Icon::Diamond,
        ToolType::Polygon => Icon::Polygon,
        ToolType::Arrow => Icon::Arrow,
        ToolType::DoubleArrow => Icon::DoubleArrow,
        ToolType::Line => Icon::Line,
        ToolType::Polyline => Icon::Polyline,
        ToolType::Pen => Icon::Pen,
        ToolType::Highlighter => Icon::Highlighter,
        ToolType::Mosaic => Icon::Mosaic,
        ToolType::Blur => Icon::Blur,
        ToolType::Text => Icon::Text,
        ToolType::StepBadge => Icon::StepBadge,
    }
}

/// The icon shown on a family's top-level button: its remembered variant.
fn family_icon(family: ToolFamily, variant: ToolType, badge_square: bool) -> Icon {
    match family {
        ToolFamily::Badge => {
            if badge_square {
                Icon::BadgeSquare
            } else {
                Icon::StepBadge
            }
        }
        _ => tool_icon(variant),
    }
}

fn tooltip(ui: &mut Ui, label: &str, shortcut: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(LABEL));
        if let Some(key) = shortcut {
            ui.add_space(6.0);
            ui.label(RichText::new(key).size(11.0).color(LABEL_DIM));
        }
    });
}

fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    width: f32,
    height: f32,
    tip: &str,
    shortcut: Option<&str>,
    active: bool,
    enabled: bool,
    danger: bool,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), sense);
    let painter = ui.painter();
    let basis = width.min(height);
    let highlight = Rect::from_center_size(rect.center(), Vec2::splat(basis));

    if enabled {
        if active {
            painter.rect_filled(highlight, 7.0, ACCENT_SOFT);
        } else if resp.is_pointer_button_down_on() {
            painter.rect_filled(highlight, 7.0, PRESS_BG);
        } else if resp.hovered() {
            let bg = if danger { DANGER_HOVER } else { HOVER_BG };
            painter.rect_filled(highlight, 7.0, bg);
        }
    }

    let color = if !enabled {
        ICON_DIM
    } else if danger {
        DANGER
    } else if active {
        ACCENT
    } else {
        ICON_COLOR
    };
    let icon_size = (basis - 8.0).clamp(12.0, 18.0);
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(icon_size));
    icons::draw(painter, icon, icon_rect, color, PANEL_BG);

    resp.on_hover_ui(|ui| tooltip(ui, tip, shortcut))
}

/// Small dropdown chevron paired with a tool icon; opens that tool's options.
fn caret_button(ui: &mut Ui, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(CARET, ROW_H), Sense::click());
    let painter = ui.painter();
    let bg = Rect::from_center_size(rect.center(), Vec2::new(CARET, ROW_H - 12.0));
    if active || resp.hovered() || resp.is_pointer_button_down_on() {
        painter.rect_filled(bg, 4.0, HOVER_BG);
    }
    let color = if active { ACCENT } else { ICON_DIM };
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(10.0));
    icons::draw(painter, Icon::Caret, icon_rect, color, PANEL_BG);
    resp.on_hover_ui(|ui| tooltip(ui, "工具选项", None))
}

/// Six-dot handle: drag to move the whole panel, double-click to reset placement.
fn grip_handle(ui: &mut Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(GRIP, ROW_H), Sense::click_and_drag());
    let painter = ui.painter();
    let bg = rect.shrink2(Vec2::new(1.0, 6.0));
    if resp.dragged() || resp.hovered() || resp.is_pointer_button_down_on() {
        painter.rect_filled(bg, 5.0, HOVER_BG);
    }
    let color = if resp.dragged() || resp.hovered() {
        ICON_COLOR
    } else {
        ICON_DIM
    };
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(14.0));
    icons::draw(painter, Icon::Grip, icon_rect, color, PANEL_BG);
    resp.on_hover_ui(|ui| tooltip(ui, "拖动移动 · 双击复位", None))
}

fn divider(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(SEP, height), Sense::hover());
    let x = rect.center().x;
    ui.painter().line_segment(
        [
            Pos2::new(x, rect.top() + 6.0),
            Pos2::new(x, rect.bottom() - 6.0),
        ],
        Stroke::new(1.0, DIVIDER),
    );
}

fn swatch(ui: &mut Ui, color: Color32, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(ROW2_ITEM), Sense::click());
    let painter = ui.painter();
    let sw = Rect::from_center_size(rect.center(), Vec2::splat(SWATCH));

    if active {
        painter.rect_filled(sw.expand(3.0), 7.0, with_alpha(color, 40));
        painter.rect_filled(sw.expand(1.5), 6.0, with_alpha(color, 80));
    }
    painter.rect_filled(sw, 5.0, color);
    painter.rect_stroke(
        sw,
        5.0,
        Stroke::new(1.0, Color32::from_black_alpha(90)),
        StrokeKind::Inside,
    );
    if active {
        painter.rect_stroke(
            sw.expand(1.5),
            6.0,
            Stroke::new(1.5, Color32::WHITE),
            StrokeKind::Outside,
        );
    } else if resp.hovered() {
        painter.rect_stroke(
            sw.expand(1.0),
            5.5,
            Stroke::new(1.0, Color32::from_white_alpha(120)),
            StrokeKind::Outside,
        );
    }
    resp
}

fn color_group(ui: &mut Ui, color: &mut Color32) {
    for &preset in COLOR_PRESETS {
        let active = *color == preset;
        let hex = format!("#{:02X}{:02X}{:02X}", preset.r(), preset.g(), preset.b());
        let resp = swatch(ui, preset, active).on_hover_ui(|ui| tooltip(ui, &hex, None));
        if resp.clicked() {
            *color = preset;
        }
    }

    let custom_active = !COLOR_PRESETS.contains(color);
    let wheel = icon_button(
        ui,
        Icon::ColorWheel,
        ROW2_ITEM,
        ROW2_ITEM,
        "自定义颜色",
        None,
        custom_active,
        true,
        false,
    );
    if wheel.clicked() {
        // The picker itself is rendered at the toolbar level (see `render`) so it
        // survives egui's single popup slot replacing this tool's options popover.
        ui.data_mut(|d| d.insert_temp(egui::Id::new("rshot_custom_color_anchor"), wheel.rect));
        ui.memory_mut(|m| m.toggle_popup(egui::Id::new("rshot_custom_color")));
    }
}

fn stroke_group(ui: &mut Ui, stroke: &mut StrokeWidth) {
    let options = [
        (StrokeWidth::Thin, "细 · 2px", 2.5),
        (StrokeWidth::Medium, "中 · 4px", 4.0),
        (StrokeWidth::Thick, "粗 · 8px", 6.0),
    ];
    for (sw, tip, dot_r) in options {
        let active = *stroke == sw;
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(ROW2_ITEM), Sense::click());
        let painter = ui.painter();
        if active {
            painter.rect_filled(rect, 6.0, ACCENT_SOFT);
        } else if resp.hovered() {
            painter.rect_filled(rect, 6.0, HOVER_BG);
        }
        let dot = if active { ACCENT } else { ICON_COLOR };
        painter.circle_filled(rect.center(), dot_r, dot);
        if resp.on_hover_ui(|ui| tooltip(ui, tip, None)).clicked() {
            *stroke = sw;
        }
    }
}

/// Three-step intensity selector (mosaic block size / blur radius).
fn level_group(ui: &mut Ui, current: &mut u32, levels: [(u32, &str); 3]) {
    for (i, (value, tip)) in levels.iter().enumerate() {
        let active = *current == *value;
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(ROW2_ITEM), Sense::click());
        let painter = ui.painter();
        if active {
            painter.rect_filled(rect, 6.0, ACCENT_SOFT);
        } else if resp.hovered() {
            painter.rect_filled(rect, 6.0, HOVER_BG);
        }
        let dot = if active { ACCENT } else { ICON_COLOR };
        painter.circle_filled(rect.center(), 2.5 + i as f32 * 1.75, dot);
        if resp.on_hover_ui(|ui| tooltip(ui, tip, None)).clicked() {
            *current = *value;
        }
    }
}

fn font_stepper(ui: &mut Ui, font_size: &mut f32) {
    let minus = icon_button(
        ui,
        Icon::Minus,
        ROW2_ITEM,
        ROW2_ITEM,
        "减小字号",
        Some("["),
        false,
        *font_size > FONT_MIN,
        false,
    );
    if minus.clicked() {
        *font_size = (*font_size - 2.0).max(FONT_MIN);
    }
    let (rect, _) = ui.allocate_exact_size(Vec2::new(36.0, ROW2_ITEM), Sense::hover());
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("{}", *font_size as i32),
        FontId::proportional(12.0),
        LABEL,
    );
    let plus = icon_button(
        ui,
        Icon::Plus,
        ROW2_ITEM,
        ROW2_ITEM,
        "增大字号",
        Some("]"),
        false,
        *font_size < FONT_MAX,
        false,
    );
    if plus.clicked() {
        *font_size = (*font_size + 2.0).min(FONT_MAX);
    }
}

/// One selectable shape/form variant in a popover's first row.
fn variant_button(ui: &mut Ui, icon: Icon, tip: &str, active: bool) -> egui::Response {
    icon_button(
        ui, icon, ROW2_ITEM, ROW2_ITEM, tip, None, active, true, false,
    )
}

/// Contextual popover: first row = the family's shape/form variants, second row
/// = color / stroke / fill (or intensity / font size) for the active variant.
fn tool_options(ui: &mut Ui, family: ToolFamily, fi: usize, state: &mut ToolbarState) {
    ui.spacing_mut().item_spacing = Vec2::new(GAP, 6.0);
    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        let variants = family.variants();
        if variants.len() > 1 {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(GAP, 0.0);
                for &v in variants {
                    if variant_button(ui, tool_icon(v), v.name(), *state.tool == v).clicked() {
                        *state.tool = v;
                        state.family_tool[fi] = v;
                    }
                }
            });
        } else if family == ToolFamily::Badge {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(GAP, 0.0);
                if variant_button(ui, Icon::StepBadge, "圆形序号", !*state.badge_square).clicked()
                {
                    *state.badge_square = false;
                }
                if variant_button(ui, Icon::BadgeSquare, "方形序号", *state.badge_square).clicked()
                {
                    *state.badge_square = true;
                }
            });
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(GAP, 0.0);
            match family {
                ToolFamily::Mosaic => level_group(ui, state.mosaic_block, MOSAIC_LEVELS),
                ToolFamily::Blur => level_group(ui, state.blur_radius, BLUR_LEVELS),
                ToolFamily::Text => {
                    color_group(ui, state.color);
                    divider(ui, ROW2_ITEM);
                    font_stepper(ui, state.font_size);
                }
                ToolFamily::Badge => color_group(ui, state.color),
                _ => {
                    color_group(ui, state.color);
                    divider(ui, ROW2_ITEM);
                    stroke_group(ui, state.stroke);
                    if state.tool.can_fill() {
                        divider(ui, ROW2_ITEM);
                        let resp = icon_button(
                            ui,
                            Icon::Fill,
                            ROW2_ITEM,
                            ROW2_ITEM,
                            "填充",
                            Some("F"),
                            *state.filled,
                            true,
                            false,
                        );
                        if resp.clicked() {
                            *state.filled = !*state.filled;
                        }
                    }
                }
            }
        });
    });
}

pub struct Toolbar;

impl Toolbar {
    /// Renders the movable single-row toolbar; returns the triggered output action, if any.
    pub fn render(
        ui: &mut Ui,
        selection_rect: Rect,
        screen_size: Vec2,
        state: &mut ToolbarState,
    ) -> Option<ToolbarAction> {
        let mut action = None;
        // 弹层铬色固定深色、与面板一致；否则宿主 egui 浅色样式会白底配浅图标，看不清。
        let visuals = &mut ui.style_mut().visuals;
        visuals.dark_mode = true;
        visuals.window_fill = Color32::from_rgba_unmultiplied(24, 26, 31, 250);
        visuals.window_stroke = Stroke::new(1.0, Color32::from_white_alpha(40));
        let default = panel_rect(selection_rect, screen_size, ROW_W, HEIGHT);
        let panel = match *state.position {
            Some(p) => Rect::from_min_size(
                clamp_pos(p, screen_size, ROW_W, HEIGHT),
                Vec2::new(ROW_W, HEIGHT),
            ),
            None => default,
        };

        let painter = ui.painter();
        painter.rect_filled(
            panel.expand(3.0),
            RADIUS + 3.0,
            Color32::from_black_alpha(36),
        );
        painter.rect_filled(
            panel.expand(1.5),
            RADIUS + 1.5,
            Color32::from_black_alpha(48),
        );
        painter.rect_filled(panel, RADIUS, PANEL_BG);
        painter.rect_stroke(
            panel,
            RADIUS,
            Stroke::new(1.0, PANEL_BORDER),
            StrokeKind::Inside,
        );

        // Inset child so the first widget lands exactly at panel.min + padding.
        let inner = panel.shrink2(Vec2::new(PAD_X, PAD_Y));
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("rshot_toolbar")
                .max_rect(inner)
                .layout(Layout::left_to_right(Align::Center)),
        );
        child.spacing_mut().item_spacing = Vec2::new(GAP, 0.0);

        // Stable popover ids (one per family) scoped to the toolbar child.
        let ids: [egui::Id; FAMILIES.len()] =
            std::array::from_fn(|i| child.make_persistent_id(format!("rshot_opts_{}", i)));

        let grip = grip_handle(&mut child);

        // Select has no options, so no caret.
        let select_active = *state.tool == ToolType::Select;
        if icon_button(
            &mut child,
            Icon::Select,
            BTN,
            ROW_H,
            "选择 · 编辑标注",
            Some(ToolType::Select.hotkey()),
            select_active,
            true,
            false,
        )
        .clicked()
        {
            *state.tool = ToolType::Select;
            state.family_tool[0] = ToolType::Select;
        }

        for fi in 1..FAMILIES.len() {
            let family = FAMILIES[fi];
            let variant = state.family_tool[fi];
            let active = state.tool.family() == family;
            let icon = family_icon(family, variant, *state.badge_square);
            // Icon and caret are direct siblings (no nested horizontal): nesting
            // expands the parent cursor and makes each group sink lower.
            if icon_button(
                &mut child,
                icon,
                BTN,
                ROW_H,
                family.name(),
                Some(variant.hotkey()),
                active,
                true,
                false,
            )
            .clicked()
            {
                *state.tool = variant;
            }
            child.spacing_mut().item_spacing.x = 0.0;
            let caret = caret_button(&mut child, active);
            child.spacing_mut().item_spacing.x = GAP;
            if caret.clicked() {
                *state.tool = variant;
                // egui keeps at most one popup open, so this replaces any other.
                child.memory_mut(|m| m.toggle_popup(ids[fi]));
            }
            egui::popup_below_widget(
                &child,
                ids[fi],
                &caret,
                egui::PopupCloseBehavior::CloseOnClickOutside,
                |gui| tool_options(gui, family, fi, state),
            );
        }

        divider(&mut child, ROW_H);

        if icon_button(
            &mut child,
            Icon::Undo,
            ACTION,
            ROW_H,
            "撤销",
            Some("⌘Z"),
            false,
            state.can_undo,
            false,
        )
        .clicked()
        {
            action = Some(ToolbarAction::Undo);
        }
        if icon_button(
            &mut child,
            Icon::Redo,
            ACTION,
            ROW_H,
            "重做",
            Some("⇧⌘Z"),
            false,
            state.can_redo,
            false,
        )
        .clicked()
        {
            action = Some(ToolbarAction::Redo);
        }
        if icon_button(
            &mut child,
            Icon::Trash,
            ACTION,
            ROW_H,
            "清空标注",
            Some("Del"),
            false,
            state.can_clear,
            false,
        )
        .clicked()
        {
            action = Some(ToolbarAction::Clear);
        }

        divider(&mut child, ROW_H);

        if icon_button(
            &mut child,
            Icon::Pin,
            ACTION,
            ROW_H,
            "贴图",
            Some("P"),
            false,
            true,
            false,
        )
        .clicked()
        {
            action = Some(ToolbarAction::Pin);
        }
        if icon_button(
            &mut child,
            Icon::Copy,
            ACTION,
            ROW_H,
            "复制",
            Some("⏎"),
            false,
            true,
            false,
        )
        .clicked()
        {
            action = Some(ToolbarAction::Copy);
        }
        if icon_button(
            &mut child,
            Icon::Save,
            ACTION,
            ROW_H,
            "保存",
            Some("⌘S"),
            false,
            true,
            false,
        )
        .clicked()
        {
            action = Some(ToolbarAction::Save);
        }
        if icon_button(
            &mut child,
            Icon::Close,
            ACTION,
            ROW_H,
            "取消",
            Some("Esc"),
            false,
            true,
            true,
        )
        .clicked()
        {
            action = Some(ToolbarAction::Cancel);
        }

        // Custom color picker floats at the toolbar level so it stays open even
        // after the single popup slot replaces the tool options popover.
        let picker_id = egui::Id::new("rshot_custom_color");
        if let Some(anchor) =
            child.data(|d| d.get_temp::<Rect>(egui::Id::new("rshot_custom_color_anchor")))
        {
            let anchor_resp = child.interact(
                anchor,
                egui::Id::new("rshot_custom_color_anchor"),
                Sense::hover(),
            );
            let mut picked = *state.color;
            egui::popup_below_widget(
                &child,
                picker_id,
                &anchor_resp,
                egui::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    egui::color_picker::color_picker_color32(
                        ui,
                        &mut picked,
                        egui::color_picker::Alpha::Opaque,
                    );
                },
            );
            *state.color = picked;
        }

        // Grip interaction: drag to move, double-click to reset, grab cursors.
        if grip.double_clicked() {
            *state.position = None;
        } else if grip.dragged() {
            let new_min = panel.min + grip.drag_delta();
            *state.position = Some(clamp_pos(new_min, screen_size, ROW_W, HEIGHT));
        }
        if grip.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if grip.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Vec2 {
        Vec2::new(1440.0, 900.0)
    }

    fn selection() -> Rect {
        Rect::from_min_size(Pos2::new(300.0, 150.0), Vec2::new(600.0, 400.0))
    }

    fn default_panel() -> Rect {
        panel_rect(selection(), screen(), ROW_W, HEIGHT)
    }

    fn row_y(panel: Rect) -> f32 {
        panel.top() + PAD_Y + ROW_H / 2.0
    }

    fn grip_center(panel: Rect) -> Pos2 {
        Pos2::new(panel.left() + PAD_X + GRIP / 2.0, row_y(panel))
    }

    /// Left edge of tool group `i` (i = 1..=10; group = icon + caret).
    fn group_start(panel: Rect, i: usize) -> f32 {
        let x0 = panel.left() + PAD_X;
        let base = x0 + GRIP + GAP + BTN + GAP;
        base + (i as f32 - 1.0) * (BTN + CARET + GAP)
    }

    fn tool_icon_center(panel: Rect, i: usize) -> Pos2 {
        Pos2::new(group_start(panel, i) + BTN / 2.0, row_y(panel))
    }

    fn tool_caret_center(panel: Rect, i: usize) -> Pos2 {
        Pos2::new(group_start(panel, i) + BTN + CARET / 2.0, row_y(panel))
    }

    fn action_start(panel: Rect) -> f32 {
        let x0 = panel.left() + PAD_X;
        let base = x0 + GRIP + GAP + BTN + GAP;
        base + 7.0 * (BTN + CARET + GAP) + SEP + GAP
    }

    fn output_start(panel: Rect) -> f32 {
        action_start(panel) + 3.0 * (ACTION + GAP) + SEP + GAP
    }

    /// Output action `k`: 0 pin, 1 copy, 2 save, 3 close.
    fn output_center(panel: Rect, k: usize) -> Pos2 {
        Pos2::new(
            output_start(panel) + k as f32 * (ACTION + GAP) + ACTION / 2.0,
            row_y(panel),
        )
    }

    struct Harness {
        ctx: egui::Context,
        tool: ToolType,
        color: Color32,
        stroke: StrokeWidth,
        filled: bool,
        font_size: f32,
        mosaic_block: u32,
        blur_radius: u32,
        position: Option<Pos2>,
        family_tool: [ToolType; FAMILIES.len()],
        badge_square: bool,
        options_family: Option<ToolFamily>,
        action: Option<ToolbarAction>,
    }

    impl Harness {
        fn new(tool: ToolType) -> Self {
            Self {
                ctx: egui::Context::default(),
                tool,
                color: COLOR_PRESETS[0],
                stroke: StrokeWidth::Medium,
                filled: false,
                font_size: 18.0,
                mosaic_block: 10,
                blur_radius: 8,
                position: None,
                family_tool: std::array::from_fn(|i| FAMILIES[i].default_tool()),
                badge_square: false,
                options_family: None,
                action: None,
            }
        }

        fn pass(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen())),
                events,
                ..Default::default()
            };
            let sel = selection();
            let Self {
                ctx,
                tool,
                color,
                stroke,
                filled,
                font_size,
                mosaic_block,
                blur_radius,
                position,
                family_tool,
                badge_square,
                options_family,
                action,
            } = self;
            ctx.run(input, |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        let mut state = ToolbarState {
                            tool,
                            color,
                            stroke,
                            filled,
                            font_size,
                            mosaic_block,
                            blur_radius,
                            position,
                            family_tool,
                            badge_square,
                            can_undo: true,
                            can_redo: true,
                            can_clear: true,
                        };
                        match options_family {
                            Some(fam) => tool_options(ui, *fam, fam.index(), &mut state),
                            None => *action = Toolbar::render(ui, sel, screen(), &mut state),
                        }
                    });
            })
        }

        fn click(&mut self, pos: Pos2) {
            self.pass(vec![egui::Event::PointerMoved(pos)]);
            self.pass(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ]);
            self.pass(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }]);
        }

        fn drag(&mut self, from: Pos2, delta: Vec2) {
            let to = from + delta;
            self.pass(vec![egui::Event::PointerMoved(from)]);
            self.pass(vec![egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }]);
            self.pass(vec![egui::Event::PointerMoved(to)]);
            self.pass(vec![egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }]);
        }
    }

    #[test]
    fn panel_stays_on_screen() {
        let cases = [
            Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(400.0, 300.0)),
            Rect::from_min_size(Pos2::new(100.0, 4.0), Vec2::new(400.0, 300.0)),
            Rect::from_min_size(Pos2::new(900.0, 700.0), Vec2::new(500.0, 190.0)),
            Rect::from_min_size(Pos2::new(1300.0, 800.0), Vec2::new(120.0, 80.0)),
            Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(1400.0, 860.0)),
        ];
        for sel in cases {
            let r = panel_rect(sel, screen(), ROW_W, HEIGHT);
            assert!(
                r.left() >= MARGIN && r.right() <= screen().x - MARGIN,
                "{sel:?} -> {r:?}"
            );
            assert!(
                r.top() >= MARGIN && r.bottom() <= screen().y - MARGIN,
                "{sel:?} -> {r:?}"
            );
        }
    }

    #[test]
    fn panel_defaults_below_selection() {
        let r = default_panel();
        assert!(r.top() >= selection().bottom());
    }

    #[test]
    fn renders_for_every_tool() {
        for family in FAMILIES {
            let tool = family.default_tool();
            let mut h = Harness::new(tool);
            let out = h.pass(vec![]);
            assert!(
                out.shapes.len() > 20,
                "{tool:?} should paint a full toolbar"
            );
        }
    }

    #[test]
    fn clicking_tool_button_switches_tool() {
        let mut h = Harness::new(ToolType::Select);
        h.click(tool_icon_center(default_panel(), 1));
        assert_eq!(h.tool, ToolType::Rectangle);
    }

    #[test]
    fn clicking_copy_returns_copy_action() {
        let mut h = Harness::new(ToolType::Select);
        h.click(output_center(default_panel(), 1));
        assert_eq!(h.action, Some(ToolbarAction::Copy));
    }

    #[test]
    fn caret_selects_tool_and_opens_popover() {
        for i in 1..FAMILIES.len() {
            let mut h = Harness::new(ToolType::Select);
            let baseline = h.pass(vec![]).shapes.len();
            h.click(tool_caret_center(default_panel(), i));
            assert_eq!(
                h.tool,
                FAMILIES[i].default_tool(),
                "caret {i} should select its family"
            );
            let out = h.pass(vec![]);
            assert!(
                out.shapes.len() > baseline,
                "caret {i} should open an options popover"
            );
        }
    }

    #[test]
    fn popover_chrome_stays_dark_under_light_host_style() {
        let ctx = egui::Context::default();
        ctx.set_style(std::sync::Arc::new(egui::Style {
            visuals: egui::Visuals::light(),
            ..Default::default()
        }));
        let mut tool = FAMILIES[1].default_tool();
        let mut color = COLOR_PRESETS[0];
        let mut stroke = StrokeWidth::Medium;
        let mut filled = false;
        let mut font_size = 18.0;
        let mut mosaic_block = 10;
        let mut blur_radius = 8;
        let mut position = None;
        let mut family_tool = std::array::from_fn(|i| FAMILIES[i].default_tool());
        let mut badge_square = false;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen())),
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let mut state = ToolbarState {
                        tool: &mut tool,
                        color: &mut color,
                        stroke: &mut stroke,
                        filled: &mut filled,
                        font_size: &mut font_size,
                        mosaic_block: &mut mosaic_block,
                        blur_radius: &mut blur_radius,
                        position: &mut position,
                        family_tool: &mut family_tool,
                        badge_square: &mut badge_square,
                        can_undo: true,
                        can_redo: true,
                        can_clear: true,
                    };
                    Toolbar::render(ui, selection(), screen(), &mut state);
                    // egui Frame::popup reads the parent ui style, so this is the popover chrome.
                    let v = &ui.style().visuals;
                    assert!(v.dark_mode, "toolbar ui must force dark mode for popovers");
                    assert_eq!(
                        v.window_fill,
                        Color32::from_rgba_unmultiplied(24, 26, 31, 250)
                    );
                });
        });
    }

    #[test]
    fn dragging_grip_moves_toolbar() {
        let mut h = Harness::new(ToolType::Select);
        let panel = default_panel();
        let delta = Vec2::new(40.0, 25.0);
        h.drag(grip_center(panel), delta);
        let moved = h.position.expect("grip drag should store a position");
        let expected = clamp_pos(panel.min + delta, screen(), ROW_W, HEIGHT);
        assert!(
            (moved - expected).length() < 1.0,
            "moved {moved:?} expected {expected:?}"
        );
    }

    #[test]
    fn stored_position_drives_layout() {
        let mut h = Harness::new(ToolType::Select);
        h.position = Some(Pos2::new(120.0, 120.0));
        let custom = Rect::from_min_size(Pos2::new(120.0, 120.0), Vec2::new(ROW_W, HEIGHT));
        let delta = Vec2::new(30.0, 20.0);
        h.drag(grip_center(custom), delta);
        let moved = h.position.expect("position should remain set");
        let expected = clamp_pos(custom.min + delta, screen(), ROW_W, HEIGHT);
        assert!(
            (moved - expected).length() < 1.0,
            "moved {moved:?} expected {expected:?}"
        );
    }

    #[test]
    fn row_icons_share_baseline_and_stay_inside_panel() {
        let mut h = Harness::new(ToolType::Select);
        let out = h.pass(vec![]);
        let panel = default_panel();
        let select_center = Pos2::new(panel.left() + PAD_X + GRIP + GAP + BTN / 2.0, row_y(panel));
        let centers: Vec<Pos2> = std::iter::once(grip_center(panel))
            .chain(std::iter::once(select_center))
            .chain((1..FAMILIES.len()).map(|i| tool_icon_center(panel, i)))
            .chain((0..4).map(|k| output_center(panel, k)))
            .collect();
        for c in centers {
            let mut ymin = f32::INFINITY;
            let mut ymax = f32::NEG_INFINITY;
            for cs in &out.shapes {
                let r = cs.shape.visual_bounding_rect();
                if r.width() <= 22.0
                    && r.height() <= 22.0
                    && (r.center().x - c.x).abs() <= 12.0
                    && r.center().y > panel.top()
                    && r.center().y < panel.bottom()
                {
                    ymin = ymin.min(r.min.y);
                    ymax = ymax.max(r.max.y);
                }
            }
            assert!(ymin.is_finite(), "no icon painted at {c:?}");
            let cy = (ymin + ymax) / 2.0;
            assert!(
                (cy - row_y(panel)).abs() <= 2.0,
                "icon x={} center {cy} vs row {}",
                c.x,
                row_y(panel)
            );
            assert!(
                ymin >= panel.top() && ymax <= panel.bottom(),
                "icon x={} overflows panel [{ymin},{ymax}]",
                c.x
            );
        }
    }

    #[test]
    fn shapes_popover_variant_buttons_switch_tool() {
        let mut h = Harness::new(ToolType::Rectangle);
        h.options_family = Some(ToolFamily::Shapes);
        // Row-1 variant centers: (12,12) rect, (38,12) rounded, (64,12) ellipse,
        // (90,12) diamond, (116,12) polygon.
        h.click(Pos2::new(38.0, 12.0));
        assert_eq!(h.tool, ToolType::RoundedRect);
        assert_eq!(h.family_tool[1], ToolType::RoundedRect);
        h.click(Pos2::new(64.0, 12.0));
        assert_eq!(h.tool, ToolType::Ellipse);
        h.click(Pos2::new(116.0, 12.0));
        assert_eq!(h.tool, ToolType::Polygon);
        assert_eq!(h.family_tool[1], ToolType::Polygon);
    }

    #[test]
    fn lines_popover_offers_arrow_and_polyline() {
        let mut h = Harness::new(ToolType::Line);
        h.options_family = Some(ToolFamily::Lines);
        h.click(Pos2::new(38.0, 12.0)); // arrow
        assert_eq!(h.tool, ToolType::Arrow);
        h.click(Pos2::new(64.0, 12.0)); // double arrow
        assert_eq!(h.tool, ToolType::DoubleArrow);
        h.click(Pos2::new(90.0, 12.0)); // polyline
        assert_eq!(h.tool, ToolType::Polyline);
    }

    #[test]
    fn badge_popover_toggles_square() {
        let mut h = Harness::new(ToolType::StepBadge);
        h.options_family = Some(ToolFamily::Badge);
        h.click(Pos2::new(38.0, 12.0)); // square
        assert!(h.badge_square);
        h.click(Pos2::new(12.0, 12.0)); // circle
        assert!(!h.badge_square);
    }

    #[test]
    fn family_icon_remembers_variant() {
        let mut h = Harness::new(ToolType::Select);
        h.family_tool[1] = ToolType::Ellipse;
        h.click(tool_icon_center(default_panel(), 1));
        assert_eq!(h.tool, ToolType::Ellipse);
    }
}
