use egui::{
    Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Stroke, StrokeKind, Vec2,
};
use image::RgbaImage;
use log::error;
use rshot_core::ClipboardService;
use rshot_storage::{OutputFormat, StorageService};

pub struct PinApp {
    image: RgbaImage,
    texture: Option<egui::TextureHandle>,
    scale_factor: f32,
    zoom: f32,
    opacity: f32,
    click_through: bool,
    badge_text: Option<String>,
    badge_alpha: f32,
}

impl PinApp {
    pub fn new(image: RgbaImage, scale_factor: f32) -> Self {
        Self {
            image,
            texture: None,
            scale_factor,
            zoom: 1.0,
            opacity: 1.0,
            click_through: false,
            badge_text: None,
            badge_alpha: 0.0,
        }
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn opacity(&self) -> f32 {
        self.opacity
    }

    pub fn click_through(&self) -> bool {
        self.click_through
    }

    /// Rotates the image 90 degrees clockwise and updates window dimension.
    fn rotate_clockwise(&mut self, ctx: &egui::Context) {
        self.image = image::imageops::rotate90(&self.image);
        self.texture = None; // Re-upload texture on next frame

        // Swap window size
        let (w, h) = self.image.dimensions();
        let logical_w = (w as f32 / self.scale_factor) * self.zoom;
        let logical_h = (h as f32 / self.scale_factor) * self.zoom;
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(logical_w, logical_h)));

        self.show_badge("旋转 90°".to_string());
    }

    /// Resets zoom to 100%.
    fn reset_zoom(&mut self, ctx: &egui::Context) {
        self.zoom = 1.0;
        let (w, h) = self.image.dimensions();
        let logical_w = w as f32 / self.scale_factor;
        let logical_h = h as f32 / self.scale_factor;
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(logical_w, logical_h)));
        self.show_badge("100%".to_string());
    }

    fn show_badge(&mut self, text: String) {
        self.badge_text = Some(text);
        self.badge_alpha = 1.0;
    }
}

impl eframe::App for PinApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Texture management
        if self.texture.is_none() {
            let (w, h) = self.image.dimensions();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [w as usize, h as usize],
                self.image.as_raw(),
            );
            let tex = ctx.load_texture("pin_image", color_image, Default::default());
            self.texture = Some(tex);
        }

        // 2. Fade out badge
        if self.badge_alpha > 0.0 {
            self.badge_alpha = (self.badge_alpha - 0.02).max(0.0);
            ctx.request_repaint();
        }

        // 3. Central panel
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter();

                // Draw the pinned image with opacity
                if let Some(texture) = &self.texture {
                    let alpha_u8 = (self.opacity * 255.0).round() as u8;
                    let tint = Color32::from_rgba_unmultiplied(255, 255, 255, alpha_u8);
                    painter.image(
                        texture.id(),
                        rect,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        tint,
                    );
                }

                // Delicate outer border and shadow indicator
                let border_color = Color32::from_rgba_unmultiplied(100, 100, 100, (self.opacity * 180.0) as u8);
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, border_color), StrokeKind::Inside);

                // Show badge if active
                if self.badge_alpha > 0.0 {
                    if let Some(text) = &self.badge_text {
                        let alpha = (self.badge_alpha * 220.0).round() as u8;
                        let badge_pos = Pos2::new(rect.center().x, rect.center().y);
                        let font = FontId::proportional(14.0);

                        painter.rect(
                            Rect::from_center_size(badge_pos, Vec2::new(text.len() as f32 * 8.5 + 24.0, 30.0)),
                            6.0,
                            Color32::from_rgba_unmultiplied(0, 0, 0, alpha),
                            Stroke::NONE,
                            StrokeKind::Inside,
                        );
                        painter.text(
                            badge_pos,
                            Align2::CENTER_CENTER,
                            text,
                            font,
                            Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                        );
                    }
                }

                // Window Dragging: anywhere clicked and dragged
                let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                if response.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                // Double click to close
                if response.double_clicked() {
                    std::process::exit(0);
                }

                // Right click context menu
                response.context_menu(|ui| {
                    if ui.button("📋 复制图片").clicked() {
                        if let Err(e) = ClipboardService::copy_image(&self.image) {
                            error!("Failed to copy pinned image: {e}");
                        }
                        ui.close_menu();
                    }
                    if ui.button("💾 保存图片...").clicked() {
                        if let Err(e) = StorageService::save_to_default_dir(&self.image, OutputFormat::Png) {
                            error!("Failed to save pinned image: {e}");
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🔄 旋转 90° (R)").clicked() {
                        self.rotate_clockwise(ctx);
                        ui.close_menu();
                    }
                    if ui.button("🔍 恢复原始大小 (100%)").clicked() {
                        self.reset_zoom(ctx);
                        ui.close_menu();
                    }
                    if ui.button("👁 切换鼠标穿透 (X)").clicked() {
                        self.click_through = !self.click_through;
                        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(self.click_through));
                        let status = if self.click_through { "开启" } else { "关闭" };
                        self.show_badge(format!("鼠标穿透: {status}"));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("❌ 关闭 (Esc)").clicked() {
                        std::process::exit(0);
                    }
                });

                // Scroll wheel: Zoom or Opacity
                let scroll_delta = ctx.input(|i| i.raw_scroll_delta.y);
                if scroll_delta.abs() > 0.1 {
                    let cmd_down = ctx.input(|i| {
                        i.modifiers.matches_logically(Modifiers::COMMAND)
                            || i.modifiers.matches_logically(Modifiers::CTRL)
                    });

                    if cmd_down {
                        // Adjust Opacity
                        self.opacity = (self.opacity + scroll_delta * 0.002).clamp(0.15, 1.0);
                        self.show_badge(format!("透明度: {}%", (self.opacity * 100.0).round()));
                    } else {
                        // Adjust Zoom
                        let factor = 1.0 + scroll_delta * 0.002;
                        self.zoom = (self.zoom * factor).clamp(0.1, 10.0);

                        let (w, h) = self.image.dimensions();
                        let logical_w = (w as f32 / self.scale_factor) * self.zoom;
                        let logical_h = (h as f32 / self.scale_factor) * self.zoom;
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(logical_w, logical_h)));
                        self.show_badge(format!("缩放: {}%", (self.zoom * 100.0).round()));
                    }
                }

                // Keyboard shortcuts
                ctx.input(|i| {
                    if i.key_pressed(Key::Escape) {
                        std::process::exit(0);
                    }
                    if i.key_pressed(Key::R) {
                        self.rotate_clockwise(ctx);
                    }
                    if i.key_pressed(Key::X) {
                        self.click_through = !self.click_through;
                        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(self.click_through));
                        let status = if self.click_through { "开启" } else { "关闭" };
                        self.show_badge(format!("鼠标穿透: {status}"));
                    }
                    if i.modifiers.matches_logically(Modifiers::COMMAND)
                        || i.modifiers.matches_logically(Modifiers::CTRL)
                    {
                        if i.key_pressed(Key::C) {
                            let _ = ClipboardService::copy_image(&self.image);
                            self.show_badge("已复制到剪贴板".to_string());
                        } else if i.key_pressed(Key::S) {
                            let _ = StorageService::save_to_default_dir(&self.image, OutputFormat::Png);
                            self.show_badge("已保存截图".to_string());
                        }
                    }
                });
            });
    }
}
