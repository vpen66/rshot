//! Brief auto-dismissing confirmation toast (e.g. fullscreen save feedback).
//! Frameless, always-on-top, no Dock/taskbar entry; closes itself after LIFE.
//! Clicking it reveals the saved file in the platform file manager.

use eframe::egui::{self, Color32, Pos2, Stroke, StrokeKind};
use std::time::{Duration, Instant};

const LIFE: Duration = Duration::from_millis(2500);
const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    const fn pm(c: u8, a: u8) -> u8 {
        ((c as u16 * a as u16 + 127) / 255) as u8
    }
    Color32::from_rgba_premultiplied(pm(r, a), pm(g, a), pm(b, a), a)
}
const BG: Color32 = rgba(24, 26, 31, 242);
const BORDER: Color32 = rgba(255, 255, 255, 40);
const TEXT: Color32 = Color32::from_rgb(233, 237, 245);
const DIM: Color32 = Color32::from_rgb(148, 155, 170);
const WIDTH: f32 = 460.0;
const HEIGHT: f32 = 78.0;
const PAD: f32 = 14.0;

struct Toast {
    title: String,
    detail: String,
    path: String,
    born: Instant,
}

impl eframe::App for Toast {
    // eframe's default clear color is a 70% dark veil over the whole square
    // window, which would hide the rounded corners on a transparent window.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.born.elapsed() >= LIFE && !ctx.is_pointer_over_area() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        // Hovering pauses the countdown so the toast stays clickable.
        if ctx.is_pointer_over_area() {
            self.born = Instant::now();
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let resp = ui.interact(rect, ui.id().with("toast"), egui::Sense::click());
                if resp.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    reveal_in_folder(&self.path);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                let painter = ui.painter();
                painter.rect_filled(rect, 10.0, BG);
                painter.rect_stroke(rect, 10.0, Stroke::new(1.0, BORDER), StrokeKind::Inside);
                painter.text(
                    Pos2::new(rect.left() + PAD, rect.top() + PAD),
                    egui::Align2::LEFT_TOP,
                    &self.title,
                    egui::FontId::proportional(14.0),
                    TEXT,
                );
                painter.text(
                    Pos2::new(rect.left() + PAD, rect.top() + PAD + 22.0),
                    egui::Align2::LEFT_TOP,
                    &self.detail,
                    egui::FontId::proportional(11.0),
                    DIM,
                );
                painter.text(
                    Pos2::new(rect.left() + PAD, rect.top() + PAD + 40.0),
                    egui::Align2::LEFT_TOP,
                    "点击打开所在目录",
                    egui::FontId::proportional(11.0),
                    DIM,
                );
            });
    }
}

/// Opens the platform file manager with the saved file selected (or its folder).
fn reveal_in_folder(path: &str) {
    let p = std::path::Path::new(path);
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(p).spawn();
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the fullscreen child may run without a console.
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{path}"))
            .creation_flags(0x0800_0000)
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open")
        .arg(p.parent().unwrap_or(p))
        .spawn();
}

fn bottom_center() -> Pos2 {
    let (ox, oy, lw, lh) = rshot_core::ScreenCapture::get_monitors()
        .ok()
        .and_then(|ms| ms.into_iter().find(|m| m.is_primary))
        .map(|m| {
            let s = m.scale_factor.max(0.1);
            (
                m.x as f32,
                m.y as f32,
                m.width as f32 / s,
                m.height as f32 / s,
            )
        })
        .unwrap_or((0.0, 0.0, 1920.0, 1080.0));
    Pos2::new(ox + (lw - WIDTH) / 2.0, oy + lh - HEIGHT - 96.0)
}

/// Shows a short-lived confirmation toast; blocks until it auto-dismisses.
pub fn show_toast(title: String, detail: String) -> Result<(), Box<dyn std::error::Error>> {
    let path = detail.clone();
    let detail = if detail.chars().count() > 60 {
        let truncated: String = detail.chars().take(59).collect();
        format!("{truncated}…")
    } else {
        detail
    };
    let pos = bottom_center();
    let viewport = egui::ViewportBuilder::default()
        .with_title("rshot-toast")
        .with_decorations(false)
        .with_always_on_top()
        .with_transparent(true)
        .with_resizable(false)
        .with_inner_size([WIDTH, HEIGHT])
        .with_position([pos.x, pos.y]);
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    #[cfg(target_os = "macos")]
    {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory);
        }));
    }
    eframe::run_native(
        "rshot-toast",
        options,
        Box::new(move |cc| {
            crate::install_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(Toast {
                title,
                detail,
                path,
                born: Instant::now(),
            }))
        }),
    )?;
    Ok(())
}
