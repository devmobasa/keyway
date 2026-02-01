use crate::combo::ComboItem;
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
"#;

#[derive(Clone)]
pub struct OverlayWindow {
    window: ApplicationWindow,
    container: GtkBox,
}

impl OverlayWindow {
    pub fn new(app: &Application) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .build();

        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_namespace("keyway-visualizer");
        window.set_keyboard_mode(KeyboardMode::None);

        window.set_anchor(Edge::Top, false);
        window.set_anchor(Edge::Bottom, true);
        window.set_anchor(Edge::Left, false);
        window.set_anchor(Edge::Right, true);

        window.set_margin(Edge::Bottom, 40);
        window.set_margin(Edge::Right, 40);
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

    pub fn render(&self, combos: &VecDeque<ComboItem>) {
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }

        for combo in combos {
            let label = Label::new(Some(&combo.text));
            label.add_css_class("key-bubble");
            self.container.append(&label);
        }

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
