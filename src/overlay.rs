use crate::combo::ComboItem;
use crate::settings::{Position, Settings};
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box as GtkBox, CssProvider, Label, Orientation};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::collections::VecDeque;

const OVERLAY_CSS: &str = r#"
.keyway-window {
    background: transparent;
}

.key-bubble {
    background: rgba(20, 20, 20, 0.70);
    color: #ffffff;
    padding: 6px 10px;
    border-radius: 8px;
    font-weight: 600;
    font-size: 14px;
}

.key-bubble.status {
    background: rgba(160, 60, 60, 0.85);
}

.keyway-window.paused .key-bubble {
    background: rgba(50, 50, 50, 0.60);
    color: #d8d8d8;
}
"#;

#[derive(Clone)]
pub struct OverlayWindow {
    window: ApplicationWindow,
    container: GtkBox,
}

impl OverlayWindow {
    pub fn new(app: &Application, settings: &Settings) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .build();

        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_namespace("keyway-visualizer");
        window.set_keyboard_mode(KeyboardMode::None);

        apply_position(&window, settings.position, settings.margin);
        window.set_exclusive_zone(0);

        let container = GtkBox::new(Orientation::Horizontal, 8);
        container.set_margin_top(8);
        container.set_margin_bottom(8);
        container.set_margin_start(8);
        container.set_margin_end(8);

        window.set_child(Some(&container));
        window.add_css_class("keyway-window");

        apply_css(&window);

        window.present();

        Self { window, container }
    }

    pub fn render(&self, combos: &VecDeque<ComboItem>, paused: bool) {
        if paused {
            self.window.add_css_class("paused");
        } else {
            self.window.remove_css_class("paused");
        }

        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }

        for combo in combos {
            let label = Label::new(Some(&combo.text));
            label.add_css_class("key-bubble");
            if combo.text == "Paused" || combo.text == "Resumed" {
                label.add_css_class("status");
            }
            self.container.append(&label);
        }

        self.window.queue_resize();
    }

    pub fn update_position(&self, position: Position, margin: i32) {
        apply_position(&self.window, position, margin);
        self.window.queue_resize();
    }
}

fn apply_css(window: &ApplicationWindow) {
    let provider = CssProvider::new();
    provider.load_from_string(OVERLAY_CSS);

    let display = gtk4::prelude::WidgetExt::display(window);
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn apply_position(window: &ApplicationWindow, position: Position, margin: i32) {
    match position {
        Position::BottomRight => {
            window.set_anchor(Edge::Top, false);
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, false);
            window.set_anchor(Edge::Right, true);
        }
        Position::BottomLeft => {
            window.set_anchor(Edge::Top, false);
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, false);
        }
        Position::TopRight => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, false);
            window.set_anchor(Edge::Left, false);
            window.set_anchor(Edge::Right, true);
        }
        Position::TopLeft => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, false);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, false);
        }
    }

    window.set_margin(Edge::Top, margin);
    window.set_margin(Edge::Bottom, margin);
    window.set_margin(Edge::Left, margin);
    window.set_margin(Edge::Right, margin);
}
