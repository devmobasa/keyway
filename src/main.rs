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
use settings_window::SettingsWindow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use tracing::{error, info};
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
    }));

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
                TrayAction::Quit => {
                    quit = true;
                }
            }
        }

        {
            let mut app_state = state.borrow_mut();
            while let Ok(event) = rx.try_recv() {
                let action = app_state.combo.handle_event(event);
                apply_combo_action(&mut changed, &mut paused_changed, action);
            }

            if app_state.combo.prune_expired() {
                changed = true;
            }

            if changed {
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
                window.set_status("Saved");
            } else {
                window.set_status("Applied");
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
}

impl AppState {
    fn apply_settings(&mut self, new_settings: Settings) -> Result<()> {
        let hotkey = Hotkey::parse(&new_settings.pause_hotkey)?;

        if new_settings.show_mouse != self.settings.show_mouse {
            let new_handle = start_listener(&self.input_tx, new_settings.show_mouse)?;
            self.listener_handle = new_handle;
        }

        self.overlay
            .update_position(new_settings.position, new_settings.margin);

        self.combo.update_settings(
            new_settings.max_items,
            Duration::from_millis(new_settings.ttl_ms),
            Duration::from_millis(new_settings.repeat_coalesce_ms),
            Duration::from_millis(new_settings.modifier_grace_ms),
            hotkey,
        );

        self.settings = new_settings;
        self.overlay.render(self.combo.items(), self.combo.paused());

        Ok(())
    }
}
