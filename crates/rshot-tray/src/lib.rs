mod config;

use config::Config;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use log::{error, info, warn};
use rshot_updater::CheckOutcome;
use std::process::Command;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};
use winit::{
    application::ApplicationHandler,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureKind {
    Interactive,
    Fullscreen,
}

enum UserEvent {
    Menu(String),
    Hotkey(u32),
    UpdateChecked {
        result: Result<CheckOutcome, String>,
        manual: bool,
    },
    UpdateInstalled(Result<String, String>),
}

#[derive(Clone, PartialEq)]
enum UpdateState {
    Idle,
    Checking,
    Available(String),
    UpToDate,
    Installing,
    Failed,
}

impl UpdateState {
    fn menu_label(&self) -> (String, bool) {
        match self {
            Self::Idle => ("检查更新".into(), true),
            Self::Checking => ("正在检查更新…".into(), false),
            Self::Available(v) => (format!("安装更新 v{v}"), true),
            Self::UpToDate => ("已是最新版本".into(), true),
            Self::Installing => ("正在下载并安装更新…".into(), false),
            Self::Failed => ("更新失败，点击重试".into(), true),
        }
    }
}

#[derive(Clone)]
struct MenuIds {
    capture: String,
    fullscreen: String,
    edit_config: String,
    reload_config: String,
    update: String,
    quit: String,
}

struct TrayApp {
    initialized: bool,
    config: Config,
    proxy: EventLoopProxy<UserEvent>,
    hotkey_manager: Option<GlobalHotKeyManager>,
    registered: Vec<HotKey>,
    hotkey_actions: Vec<(u32, CaptureKind)>,
    tray: Option<TrayIcon>,
    menu_ids: Option<MenuIds>,
    update_state: UpdateState,
    update_item: Option<MenuItem>,
}

// 隐藏而非 FreeConsole：detach 会使标准句柄失效、子进程 spawn 全失败；共享终端（count>1）不藏。
#[cfg(target_os = "windows")]
fn hide_owned_console() {
    use windows_sys::Win32::System::Console::{GetConsoleProcessList, GetConsoleWindow};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
    let mut pids = [0u32; 2];
    let count = unsafe { GetConsoleProcessList(pids.as_mut_ptr(), pids.len() as u32) };
    if count == 1 {
        let hwnd = unsafe { GetConsoleWindow() };
        if !hwnd.is_null() {
            unsafe { ShowWindow(hwnd, SW_HIDE) };
        }
    }
}

/// Runs the resident menu-bar app. Returns when the user quits from the tray menu.
pub fn run() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    hide_owned_console();
    let config = Config::load();
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    // Resident menu-bar app: accessory policy keeps it out of the Dock and app switcher.
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        builder.with_activation_policy(ActivationPolicy::Accessory);
    }
    let event_loop = builder.build()?;
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
        proxy,
        hotkey_manager: None,
        registered: Vec::new(),
        hotkey_actions: Vec::new(),
        tray: None,
        menu_ids: None,
        update_state: UpdateState::Idle,
        update_item: None,
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
                } else if id == ids.update {
                    self.on_update_menu_clicked();
                } else if id == ids.quit {
                    event_loop.exit();
                }
            }
            UserEvent::Hotkey(id) => {
                if let Some((_, kind)) = self.hotkey_actions.iter().find(|(hid, _)| *hid == id) {
                    self.spawn_capture(*kind);
                }
            }
            UserEvent::UpdateChecked { result, manual } => {
                self.on_update_checked(result, manual);
            }
            UserEvent::UpdateInstalled(result) => {
                self.on_update_installed(result, event_loop);
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
        if self.config.update.check_on_startup {
            self.spawn_update_check(false);
        }
        Ok(())
    }

    fn register_hotkeys(&mut self) {
        self.hotkey_actions.clear();
        let Some(manager) = &self.hotkey_manager else {
            return;
        };
        let bindings = [
            (
                self.config.hotkeys.capture.clone(),
                CaptureKind::Interactive,
            ),
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

    fn build_menu(&mut self) -> (Menu, MenuIds) {
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
        let (update_label, update_enabled) = self.update_state.menu_label();
        let update = MenuItem::new(update_label, update_enabled, None);
        let quit = MenuItem::new("退出 RShot", true, None);

        let ids = MenuIds {
            capture: capture.id().0.clone(),
            fullscreen: fullscreen.id().0.clone(),
            edit_config: edit_config.id().0.clone(),
            reload_config: reload_config.id().0.clone(),
            update: update.id().0.clone(),
            quit: quit.id().0.clone(),
        };

        let menu = Menu::new();
        for item in [
            &capture,
            &fullscreen,
            &edit_config,
            &reload_config,
            &update,
            &quit,
        ] {
            if let Err(e) = menu.append(item) {
                error!("Failed to append menu item: {e}");
            }
        }
        self.update_item = Some(update);
        (menu, ids)
    }

    fn set_update_state(&mut self, state: UpdateState) {
        self.update_state = state;
        if let Some(item) = &self.update_item {
            let (label, enabled) = self.update_state.menu_label();
            item.set_text(label);
            item.set_enabled(enabled);
        }
    }

    fn spawn_update_check(&mut self, manual: bool) {
        if manual {
            self.set_update_state(UpdateState::Checking);
        }
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result =
                rshot_updater::check(env!("CARGO_PKG_VERSION")).map_err(|e| format!("{e:#}"));
            let _ = proxy.send_event(UserEvent::UpdateChecked { result, manual });
        });
    }

    fn on_update_menu_clicked(&mut self) {
        match self.update_state.clone() {
            UpdateState::Available(version) => {
                self.set_update_state(UpdateState::Installing);
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = rshot_updater::install(env!("CARGO_PKG_VERSION"), &version)
                        .map(|()| version)
                        .map_err(|e| format!("{e:#}"));
                    let _ = proxy.send_event(UserEvent::UpdateInstalled(result));
                });
            }
            UpdateState::Checking | UpdateState::Installing => {}
            _ => self.spawn_update_check(true),
        }
    }

    fn on_update_checked(&mut self, result: Result<CheckOutcome, String>, manual: bool) {
        match result {
            Ok(CheckOutcome::Available(info)) => {
                info!("Update available: v{} ({})", info.version, info.date);
                self.set_update_state(UpdateState::Available(info.version));
            }
            Ok(CheckOutcome::UpToDate) => {
                let state = if manual {
                    UpdateState::UpToDate
                } else {
                    UpdateState::Idle
                };
                self.set_update_state(state);
            }
            Err(e) => {
                warn!("Update check failed: {e}");
                let state = if manual {
                    UpdateState::Failed
                } else {
                    UpdateState::Idle
                };
                self.set_update_state(state);
            }
        }
    }

    fn on_update_installed(
        &mut self,
        result: Result<String, String>,
        event_loop: &ActiveEventLoop,
    ) {
        match result {
            Ok(version) => {
                info!("Update v{version} installed, relaunching RShot");
                let spawned = std::env::current_exe().and_then(|exe| Command::new(exe).spawn());
                match spawned {
                    Ok(_) => event_loop.exit(),
                    Err(e) => {
                        error!("Failed to relaunch after update: {e}");
                        self.set_update_state(UpdateState::Failed);
                    }
                }
            }
            Err(e) => {
                error!("Update install failed: {e}");
                self.set_update_state(UpdateState::Failed);
            }
        }
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
