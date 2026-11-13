mod config;

use config::Config;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use log::{error, info, warn};
use std::process::Command;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};
use winit::{
    application::ApplicationHandler,
    event_loop::{ActiveEventLoop, EventLoop},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureKind {
    Interactive,
    Fullscreen,
}

enum UserEvent {
    Menu(String),
    Hotkey(u32),
}

#[derive(Clone)]
struct MenuIds {
    capture: String,
    fullscreen: String,
    edit_config: String,
    reload_config: String,
    quit: String,
}

struct TrayApp {
    initialized: bool,
    config: Config,
    hotkey_manager: Option<GlobalHotKeyManager>,
    registered: Vec<HotKey>,
    hotkey_actions: Vec<(u32, CaptureKind)>,
    tray: Option<TrayIcon>,
    menu_ids: Option<MenuIds>,
}

/// Runs the resident menu-bar app. Returns when the user quits from the tray menu.
pub fn run() -> anyhow::Result<()> {
    let config = Config::load();
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    {
        let proxy = proxy.clone();
        MenuEvent::set_event_handler(Some(move |ev: MenuEvent| {
            let _ = proxy.send_event(UserEvent::Menu(ev.id().0.clone()));
        }));
    }
    {
        let proxy = proxy.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
            if ev.state() == HotKeyState::Pressed {
                let _ = proxy.send_event(UserEvent::Hotkey(ev.id()));
            }
        }));
    }

    let mut app = TrayApp {
        initialized: false,
        config,
        hotkey_manager: None,
        registered: Vec::new(),
        hotkey_actions: Vec::new(),
        tray: None,
        menu_ids: None,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

impl ApplicationHandler<UserEvent> for TrayApp {
    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        if let Err(e) = self.setup() {
            error!("Failed to initialize tray: {e}");
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Menu(id) => {
                let Some(ids) = self.menu_ids.clone() else {
                    return;
                };
                if id == ids.capture {
                    self.spawn_capture(CaptureKind::Interactive);
                } else if id == ids.fullscreen {
                    self.spawn_capture(CaptureKind::Fullscreen);
                } else if id == ids.edit_config {
                    open_config_file();
                } else if id == ids.reload_config {
                    self.reload_config();
                } else if id == ids.quit {
                    event_loop.exit();
                }
            }
            UserEvent::Hotkey(id) => {
                if let Some((_, kind)) = self.hotkey_actions.iter().find(|(hid, _)| *hid == id) {
                    self.spawn_capture(*kind);
                }
            }
        }
    }
}

impl TrayApp {
    fn setup(&mut self) -> anyhow::Result<()> {
        self.hotkey_manager = Some(GlobalHotKeyManager::new()?);
        self.register_hotkeys();

        let (menu, ids) = self.build_menu();
        self.menu_ids = Some(ids);
        let icon = make_icon()?;
        let builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("RShot 截图");
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(icon);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon);
        self.tray = Some(builder.build()?);

        info!(
            "RShot is resident in the menu bar. Config: {}",
            config::config_path().display()
        );
        Ok(())
    }

    fn register_hotkeys(&mut self) {
        self.hotkey_actions.clear();
        let Some(manager) = &self.hotkey_manager else {
            return;
        };
        let bindings = [
            (self.config.hotkeys.capture.clone(), CaptureKind::Interactive),
            (
                self.config.hotkeys.fullscreen.clone(),
                CaptureKind::Fullscreen,
            ),
        ];
        let mut registered = Vec::new();
        for (text, kind) in bindings {
            match text.parse::<HotKey>() {
                Ok(hk) => match manager.register(hk) {
                    Ok(()) => {
                        self.hotkey_actions.push((hk.id(), kind));
                        registered.push(hk);
                    }
                    Err(e) => warn!("Failed to register hotkey '{text}': {e}"),
                },
                Err(e) => warn!("Invalid hotkey '{text}': {e}"),
            }
        }
        self.registered = registered;
    }

    fn reload_config(&mut self) {
        if let Some(manager) = &self.hotkey_manager {
            for hk in &self.registered {
                let _ = manager.unregister(*hk);
            }
        }
        self.config = Config::load();
        self.register_hotkeys();

        let (menu, ids) = self.build_menu();
        self.menu_ids = Some(ids);
        if let Some(tray) = &self.tray {
            tray.set_menu(Some(Box::new(menu)));
        }
        info!("Configuration reloaded");
    }

    fn build_menu(&self) -> (Menu, MenuIds) {
        let capture = MenuItem::new(
            format!("区域截图  ({})", self.config.hotkeys.capture),
            true,
            None,
        );
        let fullscreen = MenuItem::new(
            format!("全屏截图  ({})", self.config.hotkeys.fullscreen),
            true,
            None,
        );
        let edit_config = MenuItem::new("编辑配置…", true, None);
        let reload_config = MenuItem::new("重载配置", true, None);
        let quit = MenuItem::new("退出 RShot", true, None);

        let ids = MenuIds {
            capture: capture.id().0.clone(),
            fullscreen: fullscreen.id().0.clone(),
            edit_config: edit_config.id().0.clone(),
            reload_config: reload_config.id().0.clone(),
            quit: quit.id().0.clone(),
        };

        let menu = Menu::new();
        for item in [&capture, &fullscreen, &edit_config, &reload_config, &quit] {
            if let Err(e) = menu.append(item) {
                error!("Failed to append menu item: {e}");
            }
        }
        (menu, ids)
    }

    fn spawn_capture(&self, kind: CaptureKind) {
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => {
                error!("Failed to resolve current exe: {e}");
                return;
            }
        };
        let mut cmd = Command::new(exe);
        match kind {
            CaptureKind::Interactive => cmd.arg("--capture"),
            CaptureKind::Fullscreen => cmd.arg("--fullscreen"),
        };
        match cmd.spawn() {
            Ok(_) => info!("Launched capture ({kind:?})"),
            Err(e) => error!("Failed to launch capture process: {e}"),
        }
    }
}

fn open_config_file() {
    let path = config::config_path();
    if !path.exists() {
        let _ = Config::default().save();
    }
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg("-t").arg(&path).spawn();
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd")
        .args(["/C", "start", "", &path.to_string_lossy()])
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = Command::new("xdg-open").arg(&path).spawn();

    if let Err(e) = result {
        error!("Failed to open config file: {e}");
    }
}

fn set_pixel(rgba: &mut [u8], size: u32, x: u32, y: u32) {
    if x < size && y < size {
        let i = ((y * size + x) * 4) as usize;
        rgba[i + 3] = 255;
    }
}

/// A viewfinder-style glyph drawn as a macOS template image (alpha only).
fn make_icon() -> anyhow::Result<Icon> {
    const S: u32 = 32;
    let mut rgba = vec![0u8; (S * S * 4) as usize];
    let m = 5u32;
    let len = 9u32;
    let t = 2u32;
    let max = S - 1 - m;
    for d in 0..len {
        for k in 0..t {
            set_pixel(&mut rgba, S, m + d, m + k);
            set_pixel(&mut rgba, S, m + k, m + d);
            set_pixel(&mut rgba, S, max - d, m + k);
            set_pixel(&mut rgba, S, max - k, m + d);
            set_pixel(&mut rgba, S, m + d, max - k);
            set_pixel(&mut rgba, S, m + k, max - d);
            set_pixel(&mut rgba, S, max - d, max - k);
            set_pixel(&mut rgba, S, max - k, max - d);
        }
    }
    let c = 16i32;
    let r = 3i32;
    for y in 0..S as i32 {
        for x in 0..S as i32 {
            let dx = x - c;
            let dy = y - c;
            if dx * dx + dy * dy <= r * r {
                set_pixel(&mut rgba, S, x as u32, y as u32);
            }
        }
    }
    Icon::from_rgba(rgba, S, S).map_err(|e| anyhow::anyhow!("tray icon: {e:?}"))
}