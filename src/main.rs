mod combo;
mod hotkey;
mod input;
mod overlay;
mod settings;
mod settings_window;
mod tray;
mod xkb;

use anyhow::Result;
use async_channel::{Receiver, Sender};
use combo::{ComboAction, ComboState};
use clap::Parser;
use hotkey::Hotkey;
use gtk4::glib::{self, ControlFlow};
use gtk4::prelude::*;
use gtk4::Application;
use input::{InputListener, ListenerConfig};
use overlay::OverlayWindow;
use settings::{CliArgs, Settings};
use serde_json::Value;
use settings_window::SettingsWindow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tray::{TrayAction, TrayHandle};

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
    }
}

fn run() -> Result<()> {
    init_logging();

    let cli = CliArgs::parse();
    let (settings, config_path) = settings::Settings::load(&cli)?;

    let app = Application::builder()
        .application_id("dev.keyway.visualizer")
        .build();

    app.connect_activate(move |app| {
        if let Err(e) = build_ui(app, settings.clone(), config_path.clone()) {
            error!("Failed to start app: {}", e);
            app.quit();
        }
    });

    let _ = app.run();
    Ok(())
}

fn build_ui(app: &Application, settings: Settings, config_path: PathBuf) -> Result<()> {
    info!("Starting keyway-visualizer");

    let (tx, rx) = async_channel::bounded(256);
    let hotkey = Hotkey::parse(&settings.pause_hotkey)?;
    info!("Pause hotkey: {}", hotkey.describe());
    let combo = ComboState::new(
        settings.max_items,
        Duration::from_millis(settings.ttl_ms),
        Duration::from_millis(settings.repeat_coalesce_ms),
        Duration::from_millis(settings.modifier_grace_ms),
        hotkey,
    );

    let tray = tray::start_tray().ok();
    let (tray_rx, tray_handle) = tray
        .map(|(rx, handle)| (Some(rx), Some(handle)))
        .unwrap_or((None, None));

    let overlay = OverlayWindow::new(app, &settings);
    overlay.set_drag_enabled(settings.drag_enabled);
    let listener_handle = start_listener(&tx, settings.show_mouse)?;

    let state = Rc::new(RefCell::new(AppState {
        settings,
        config_path,
        overlay,
        combo,
        input_tx: tx,
        listener_handle,
        tray_handle,
        settings_window: None,
        dragging: false,
        drag_base_x: 0,
        drag_base_y: 0,
        app_filter_suppressed: false,
        last_app_check: Instant::now(),
        app_filter_warned: false,
    }));

    if let Some(handle) = &state.borrow().tray_handle {
        handle.set_drag_enabled(state.borrow().settings.drag_enabled);
    }

    {
        let state_begin = Rc::clone(&state);
        let state_update = Rc::clone(&state);
        let state_end = Rc::clone(&state);
        let overlay = state.borrow().overlay.clone();
        overlay.connect_drag_handlers(
            move |_x, _y| {
                state_begin.borrow_mut().begin_drag();
            },
            move |dx, dy| {
                state_update.borrow_mut().update_drag(dx, dy);
            },
            move || {
                state_end.borrow_mut().end_drag();
            },
        );
    }

    start_event_pump(app.clone(), rx, tray_rx, Rc::clone(&state));

    Ok(())
}

fn start_event_pump(
    app: Application,
    rx: Receiver<input::InputEvent>,
    tray_rx: Option<Receiver<TrayAction>>,
    state: Rc<RefCell<AppState>>,
) {
    let tray_rx = tray_rx.unwrap_or_else(|| async_channel::bounded(1).1);
    glib::timeout_add_local(Duration::from_millis(16), move || {
        let mut changed = false;
        let mut paused_changed: Option<bool> = None;
        let mut open_settings = false;
        let mut quit = false;

        while let Ok(action) = tray_rx.try_recv() {
            match action {
                TrayAction::TogglePause => {
                    let mut app_state = state.borrow_mut();
                    if app_state.combo.toggle_pause() {
                        changed = true;
                    }
                    paused_changed = Some(app_state.combo.paused());
                }
                TrayAction::OpenSettings => {
                    open_settings = true;
                }
                TrayAction::ToggleDrag => {
                    let mut app_state = state.borrow_mut();
                    app_state.toggle_drag();
                    if let Some(handle) = &app_state.tray_handle {
                        handle.set_drag_enabled(app_state.settings.drag_enabled);
                    }
                }
                TrayAction::Quit => {
                    quit = true;
                }
            }
        }

        {
            let mut app_state = state.borrow_mut();
            let now = Instant::now();
            if app_state.update_app_filter(now) {
                changed = true;
            }

            while let Ok(event) = rx.try_recv() {
                if app_state.app_filter_suppressed {
                    app_state.combo.handle_event_suppressed(event);
                } else {
                    let action = app_state.combo.handle_event(event);
                    apply_combo_action(&mut changed, &mut paused_changed, action);
                }
            }

            if !app_state.app_filter_suppressed && app_state.combo.prune_expired() {
                changed = true;
            }

            if changed && !app_state.app_filter_suppressed {
                app_state
                    .overlay
                    .render(app_state.combo.items(), app_state.combo.paused());
            }
        }

        if let Some(paused) = paused_changed {
            if let Some(handle) = &state.borrow().tray_handle {
                handle.set_paused(paused);
            }
        }

        if open_settings {
            open_settings_window(&app, Rc::clone(&state));
        }

        if quit {
            app.quit();
            return ControlFlow::Break;
        }

        ControlFlow::Continue
    });
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,keyway_visualizer=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();
}

fn start_listener(tx: &Sender<input::InputEvent>, include_mouse: bool) -> Result<input::ListenerHandle> {
    let listener = InputListener::new(
        tx.clone(),
        ListenerConfig {
            all_keyboards: true,
            include_mouse,
        },
    );
    listener.start()
}

fn apply_combo_action(
    changed: &mut bool,
    paused_changed: &mut Option<bool>,
    action: ComboAction,
) {
    if action.render {
        *changed = true;
    }
    if let Some(paused) = action.paused_changed {
        *paused_changed = Some(paused);
    }
}

fn open_settings_window(app: &Application, state: Rc<RefCell<AppState>>) {
    let window = {
        let mut app_state = state.borrow_mut();
        if let Some(window) = app_state.settings_window.as_ref() {
            window.set_from_settings(&app_state.settings);
            return window.present();
        }

        let window = Rc::new(SettingsWindow::new(app));
        window.set_from_settings(&app_state.settings);

        let window_apply = Rc::clone(&window);
        let state_apply = Rc::clone(&state);
        window.connect_apply(move || {
            apply_settings_from_window(&window_apply, &state_apply, false);
        });

        let window_save = Rc::clone(&window);
        let state_save = Rc::clone(&state);
        window.connect_save(move || {
            apply_settings_from_window(&window_save, &state_save, true);
        });

        let window_close = Rc::clone(&window);
        window.connect_close(move || {
            window_close.window.set_visible(false);
        });

        app_state.settings_window = Some(Rc::clone(&window));
        window
    };

    window.present();
}

fn apply_settings_from_window(window: &SettingsWindow, state: &Rc<RefCell<AppState>>, save: bool) {
    let base_settings = state.borrow().settings.clone();
    let new_settings = window.read_settings(&base_settings);

    if let Err(msg) = window.validate(&new_settings) {
        window.set_status(&msg);
        return;
    }

    let warn_empty_filter =
        new_settings.app_filter_enabled && new_settings.disabled_apps.is_empty();

    let result = {
        let mut app_state = state.borrow_mut();
        app_state.apply_settings(new_settings)
    };

    match result {
        Ok(_) => {
            if save {
                let (settings, path) = {
                    let app_state = state.borrow();
                    (app_state.settings.clone(), app_state.config_path.clone())
                };
                if let Err(e) = settings.save_to(&path) {
                    window.set_status(&format!("Save failed: {}", e));
                    return;
                }
                if warn_empty_filter {
                    window.set_status("Saved (app filter enabled but list is empty)");
                } else {
                    window.set_status("Saved");
                }
            } else {
                if warn_empty_filter {
                    window.set_status("Applied (app filter enabled but list is empty)");
                } else {
                    window.set_status("Applied");
                }
            }
        }
        Err(e) => {
            window.set_status(&format!("Error: {}", e));
        }
    }
}

struct AppState {
    settings: Settings,
    config_path: PathBuf,
    overlay: OverlayWindow,
    combo: ComboState,
    input_tx: Sender<input::InputEvent>,
    listener_handle: input::ListenerHandle,
    tray_handle: Option<TrayHandle>,
    settings_window: Option<Rc<SettingsWindow>>,
    dragging: bool,
    drag_base_x: i32,
    drag_base_y: i32,
    app_filter_suppressed: bool,
    last_app_check: Instant,
    app_filter_warned: bool,
}

impl AppState {
    fn apply_settings(&mut self, new_settings: Settings) -> Result<()> {
        let hotkey = Hotkey::parse(&new_settings.pause_hotkey)?;

        if new_settings.show_mouse != self.settings.show_mouse {
            let new_handle = start_listener(&self.input_tx, new_settings.show_mouse)?;
            self.listener_handle = new_handle;
        }

        self.overlay.update_position(&new_settings);
        self.overlay.set_drag_enabled(new_settings.drag_enabled);
        if let Some(handle) = &self.tray_handle {
            handle.set_drag_enabled(new_settings.drag_enabled);
        }

        self.combo.update_settings(
            new_settings.max_items,
            Duration::from_millis(new_settings.ttl_ms),
            Duration::from_millis(new_settings.repeat_coalesce_ms),
            Duration::from_millis(new_settings.modifier_grace_ms),
            hotkey,
        );

        self.settings = new_settings;
        self.app_filter_warned = false;
        self.last_app_check = Instant::now()
            .checked_sub(Duration::from_millis(1000))
            .unwrap_or_else(Instant::now);
        let _ = self.update_app_filter(Instant::now());
        self.overlay.render(self.combo.items(), self.combo.paused());

        Ok(())
    }

    fn toggle_drag(&mut self) {
        self.settings.drag_enabled = !self.settings.drag_enabled;
        self.overlay.set_drag_enabled(self.settings.drag_enabled);
        if let Some(window) = &self.settings_window {
            window.set_from_settings(&self.settings);
        }
    }

    fn begin_drag(&mut self) {
        if !self.settings.drag_enabled {
            return;
        }

        let (window_w, window_h) = self.overlay.window_size();
        let geometry = match self.overlay.monitor_geometry() {
            Some(g) => g,
            None => return,
        };

        let monitor_w = geometry.width();
        let monitor_h = geometry.height();

        let (base_x, base_y) = compute_custom_offsets(
            self.settings.position,
            self.settings.margin,
            self.settings.custom_x,
            self.settings.custom_y,
            window_w,
            window_h,
            monitor_w,
            monitor_h,
        );

        self.dragging = true;
        self.drag_base_x = base_x;
        self.drag_base_y = base_y;

        self.settings.position = settings::Position::Custom;
        self.settings.custom_x = base_x;
        self.settings.custom_y = base_y;
        self.overlay.update_position(&self.settings);

        if let Some(window) = &self.settings_window {
            window.set_from_settings(&self.settings);
        }
    }

    fn update_drag(&mut self, dx: f64, dy: f64) {
        if !self.dragging || !self.settings.drag_enabled {
            return;
        }

        let geometry = match self.overlay.monitor_geometry() {
            Some(g) => g,
            None => return,
        };
        let (window_w, window_h) = self.overlay.window_size();

        let max_x = (geometry.width() - window_w).max(0);
        let max_y = (geometry.height() - window_h).max(0);

        let mut new_x = self.drag_base_x + dx.round() as i32;
        let mut new_y = self.drag_base_y + dy.round() as i32;

        new_x = new_x.clamp(0, max_x);
        new_y = new_y.clamp(0, max_y);

        self.settings.custom_x = new_x;
        self.settings.custom_y = new_y;
        self.settings.position = settings::Position::Custom;

        self.overlay.update_position(&self.settings);

        if let Some(window) = &self.settings_window {
            window.set_from_settings(&self.settings);
        }
    }

    fn end_drag(&mut self) {
        self.dragging = false;
    }

    fn update_app_filter(&mut self, now: Instant) -> bool {
        if !self.settings.app_filter_enabled {
            if self.app_filter_suppressed {
                self.app_filter_suppressed = false;
                self.overlay.set_visible(true);
                return true;
            }
            return false;
        }

        if now.duration_since(self.last_app_check) < Duration::from_millis(500) {
            return false;
        }

        self.last_app_check = now;

        let Some(info) = get_active_app_info() else {
            if !self.app_filter_warned {
                warn!("App filter enabled but hyprctl is not available or returned no data.");
                self.app_filter_warned = true;
            }
            if self.app_filter_suppressed {
                self.app_filter_suppressed = false;
                self.combo.clear_items();
                self.overlay.set_visible(true);
                return true;
            }
            return false;
        };

        let class_lower = info.class.to_ascii_lowercase();
        let title_lower = info.title.to_ascii_lowercase();
        let disabled = self.settings.disabled_apps.iter().any(|entry| {
            let needle = entry.to_ascii_lowercase();
            class_lower.contains(&needle) || title_lower.contains(&needle)
        });

        if disabled != self.app_filter_suppressed {
            self.app_filter_suppressed = disabled;
            if disabled {
                self.combo.clear_items();
                self.overlay.set_visible(false);
            } else {
                self.overlay.set_visible(true);
            }
            return true;
        }

        false
    }
}

struct ActiveAppInfo {
    class: String,
    title: String,
}

fn get_active_app_info() -> Option<ActiveAppInfo> {
    let output = Command::new("hyprctl")
        .args(["-j", "activewindow"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value: Value = serde_json::from_slice(&output.stdout).ok()?;
    let class = value.get("class")?.as_str()?.to_string();
    let title = value.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();

    Some(ActiveAppInfo { class, title })
}

fn compute_custom_offsets(
    position: settings::Position,
    margin: i32,
    custom_x: i32,
    custom_y: i32,
    window_w: i32,
    window_h: i32,
    monitor_w: i32,
    monitor_h: i32,
) -> (i32, i32) {
    let center_x = (monitor_w - window_w).max(0) / 2;
    let center_y = (monitor_h - window_h).max(0) / 2;

    let (mut x, mut y) = match position {
        settings::Position::BottomRight => (
            monitor_w - window_w - margin,
            monitor_h - window_h - margin,
        ),
        settings::Position::BottomCenter => (center_x, monitor_h - window_h - margin),
        settings::Position::BottomLeft => (margin, monitor_h - window_h - margin),
        settings::Position::TopRight => (monitor_w - window_w - margin, margin),
        settings::Position::TopCenter => (center_x, margin),
        settings::Position::TopLeft => (margin, margin),
        settings::Position::Center => (center_x, center_y),
        settings::Position::Custom => (custom_x, custom_y),
    };

    x = x.clamp(0, (monitor_w - window_w).max(0));
    y = y.clamp(0, (monitor_h - window_h).max(0));

    (x, y)
}
