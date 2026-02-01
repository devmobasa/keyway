use crate::combo::ComboItem;
use crate::settings::{Position, Settings};
use gtk4::prelude::*;
use gtk4::{gdk, Application, ApplicationWindow, Box as GtkBox, CenterBox, CssProvider, GestureDrag, Label, Orientation};
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
    root: CenterBox,
    container: GtkBox,
    drag: GestureDrag,
    drag_enabled: std::cell::Cell<bool>,
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
        let root = CenterBox::new();
        root.set_hexpand(true);
        root.set_vexpand(true);

        let container = GtkBox::new(Orientation::Horizontal, 8);
        container.set_margin_top(8);
        container.set_margin_bottom(8);
        container.set_margin_start(8);
        container.set_margin_end(8);

        window.set_keyboard_mode(KeyboardMode::None);

        apply_position(
            &window,
            &root,
            &container,
            settings.position,
            settings.margin,
            settings.custom_x,
            settings.custom_y,
        );
        window.set_exclusive_zone(0);

        window.set_child(Some(&root));
        window.add_css_class("keyway-window");

        apply_css(&window);

        window.present();

        let drag = GestureDrag::new();
        drag.set_button(0);
        root.add_controller(drag.clone());

        Self {
            window,
            root,
            container,
            drag,
            drag_enabled: std::cell::Cell::new(false),
        }
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

    pub fn update_position(&self, settings: &Settings) {
        apply_position(
            &self.window,
            &self.root,
            &self.container,
            settings.position,
            settings.margin,
            settings.custom_x,
            settings.custom_y,
        );
        self.window.queue_resize();
    }

    pub fn set_drag_enabled(&self, enabled: bool) {
        self.drag_enabled.set(enabled);
        self.window.set_can_target(enabled);
    }

    pub fn connect_drag_handlers<F1, F2, F3>(&self, on_begin: F1, on_update: F2, on_end: F3)
    where
        F1: Fn(f64, f64) + 'static,
        F2: Fn(f64, f64) + 'static,
        F3: Fn() + 'static,
    {
        let enabled = self.drag_enabled.clone();
        self.drag.connect_drag_begin(move |_, x, y| {
            if enabled.get() {
                on_begin(x, y);
            }
        });
        let enabled = self.drag_enabled.clone();
        self.drag.connect_drag_update(move |_, dx, dy| {
            if enabled.get() {
                on_update(dx, dy);
            }
        });
        let enabled = self.drag_enabled.clone();
        self.drag.connect_drag_end(move |_, _x, _y| {
            if enabled.get() {
                on_end();
            }
        });
    }

    pub fn window_size(&self) -> (i32, i32) {
        (self.window.allocated_width(), self.window.allocated_height())
    }

    pub fn monitor_geometry(&self) -> Option<gdk::Rectangle> {
        let surface = self.window.surface()?;
        let display = surface.display();
        let monitor = display.monitor_at_surface(&surface)?;
        Some(monitor.geometry())
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

fn apply_position(
    window: &ApplicationWindow,
    root: &CenterBox,
    container: &GtkBox,
    position: Position,
    margin: i32,
    custom_x: i32,
    custom_y: i32,
) {
    apply_size_for_position(window, position, margin);

    root.set_start_widget(None::<&gtk4::Widget>);
    root.set_center_widget(None::<&gtk4::Widget>);
    root.set_end_widget(None::<&gtk4::Widget>);

    match position {
        Position::BottomRight => {
            window.set_anchor(Edge::Top, false);
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, false);
            window.set_anchor(Edge::Right, true);
            root.set_end_widget(Some(container));
            container.set_valign(gtk4::Align::End);
        }
        Position::BottomCenter => {
            window.set_anchor(Edge::Top, false);
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, true);
            root.set_center_widget(Some(container));
            container.set_valign(gtk4::Align::End);
        }
        Position::BottomLeft => {
            window.set_anchor(Edge::Top, false);
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, false);
            root.set_start_widget(Some(container));
            container.set_valign(gtk4::Align::End);
        }
        Position::TopRight => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, false);
            window.set_anchor(Edge::Left, false);
            window.set_anchor(Edge::Right, true);
            root.set_end_widget(Some(container));
            container.set_valign(gtk4::Align::Start);
        }
        Position::TopCenter => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, false);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, true);
            root.set_center_widget(Some(container));
            container.set_valign(gtk4::Align::Start);
        }
        Position::TopLeft => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, false);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, false);
            root.set_start_widget(Some(container));
            container.set_valign(gtk4::Align::Start);
        }
        Position::Center => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, true);
            root.set_center_widget(Some(container));
            container.set_valign(gtk4::Align::Center);
        }
        Position::Custom => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, false);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, false);
            root.set_start_widget(Some(container));
            container.set_valign(gtk4::Align::Start);
        }
    }

    if matches!(position, Position::Custom) {
        window.set_margin(Edge::Top, custom_y);
        window.set_margin(Edge::Bottom, 0);
        window.set_margin(Edge::Left, custom_x);
        window.set_margin(Edge::Right, 0);
    } else {
        window.set_margin(Edge::Top, margin);
        window.set_margin(Edge::Bottom, margin);
        window.set_margin(Edge::Left, margin);
        window.set_margin(Edge::Right, margin);
    }
}

fn apply_size_for_position(window: &ApplicationWindow, position: Position, margin: i32) {
    let span_x = matches!(
        position,
        Position::BottomCenter | Position::TopCenter | Position::Center
    );
    let span_y = matches!(position, Position::Center);

    if !(span_x || span_y) {
        window.set_default_size(-1, -1);
        window.set_size_request(-1, -1);
        return;
    }

    let Some(display) = gdk::Display::default() else {
        return;
    };

    let monitor = display
        .monitors()
        .item(0)
        .and_downcast::<gdk::Monitor>();

    let Some(monitor) = monitor else {
        return;
    };

    let geometry = monitor.geometry();
    let width = if span_x {
        (geometry.width() - margin.saturating_mul(2)).max(1)
    } else {
        -1
    };
    let height = if span_y {
        (geometry.height() - margin.saturating_mul(2)).max(1)
    } else {
        -1
    };

    window.set_default_size(width, height);
    window.set_size_request(width, height);
}
