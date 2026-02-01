mod combo;
mod input;
mod overlay;
mod xkb;

use anyhow::Result;
use async_channel::Receiver;
use combo::ComboState;
use gtk4::glib::{self, ControlFlow};
use gtk4::prelude::*;
use gtk4::Application;
use input::{InputListener, ListenerConfig};
use overlay::OverlayWindow;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
    }
}

fn run() -> Result<()> {
    init_logging();

    let app = Application::builder()
        .application_id("dev.keyway.visualizer")
        .build();

    app.connect_activate(|app| {
        if let Err(e) = build_ui(app) {
            error!("Failed to start app: {}", e);
            app.quit();
        }
    });

    let _ = app.run();
    Ok(())
}

fn build_ui(app: &Application) -> Result<()> {
    info!("Starting keyway-visualizer");

    let overlay = OverlayWindow::new(app);

    let (tx, rx) = async_channel::bounded(256);
    let listener = InputListener::new(tx, ListenerConfig::default());
    let listener_handle = Rc::new(listener.start()?);

    let state = Rc::new(RefCell::new(ComboState::new(5, Duration::from_millis(900))));

    start_event_pump(rx, overlay, state, listener_handle);

    Ok(())
}

fn start_event_pump(
    rx: Receiver<input::InputEvent>,
    overlay: OverlayWindow,
    state: Rc<RefCell<ComboState>>,
    listener_handle: Rc<input::ListenerHandle>,
) {
    glib::timeout_add_local(Duration::from_millis(16), move || {
        let _keep_alive = &listener_handle;
        let mut changed = false;

        {
            let mut state = state.borrow_mut();
            while let Ok(event) = rx.try_recv() {
                if state.handle_event(event) {
                    changed = true;
                }
            }

            if state.prune_expired() {
                changed = true;
            }

            if changed {
                overlay.render(state.items());
            }
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
