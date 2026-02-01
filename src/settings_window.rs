use crate::settings::{Position, Settings};
use gtk4::prelude::*;
use gtk4::{
    Adjustment, Application, ApplicationWindow, Box as GtkBox, Button, DropDown, Entry, Grid,
    Label, Orientation, SpinButton, StringList, Switch,
};

const POSITIONS: [&str; 8] = [
    "bottom-right",
    "bottom-center",
    "bottom-left",
    "top-right",
    "top-center",
    "top-left",
    "center",
    "custom",
];

pub struct SettingsWindow {
    pub window: ApplicationWindow,
    position: DropDown,
    margin: SpinButton,
    max_items: SpinButton,
    ttl_ms: SpinButton,
    show_mouse: Switch,
    pause_hotkey: Entry,
    repeat_coalesce_ms: SpinButton,
    modifier_grace_ms: SpinButton,
    drag_enabled: Switch,
    custom_x: SpinButton,
    custom_y: SpinButton,
    status: Label,
    apply_button: Button,
    save_button: Button,
    close_button: Button,
}

impl SettingsWindow {
    pub fn new(app: &Application) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Keyway Visualizer Settings")
            .default_width(420)
            .default_height(320)
            .build();

        let content = GtkBox::new(Orientation::Vertical, 12);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(16);
        content.set_margin_end(16);

        let grid = Grid::new();
        grid.set_row_spacing(10);
        grid.set_column_spacing(12);

        let position = DropDown::new(Some(StringList::new(&POSITIONS)), None::<&gtk4::Expression>);
        let margin = spin_i32(40, 0, 300, 1);
        let max_items = spin_i32(5, 1, 20, 1);
        let ttl_ms = spin_i32(900, 100, 5000, 50);
        let show_mouse = Switch::new();
        let pause_hotkey = Entry::new();
        let repeat_coalesce_ms = spin_i32(200, 0, 1000, 20);
        let modifier_grace_ms = spin_i32(120, 0, 1000, 10);
        let drag_enabled = Switch::new();
        let custom_x = spin_i32(40, 0, 5000, 10);
        let custom_y = spin_i32(40, 0, 5000, 10);

        attach_row(&grid, 0, "Position", &position);
        attach_row(&grid, 1, "Margin", &margin);
        attach_row(&grid, 2, "Max items", &max_items);
        attach_row(&grid, 3, "TTL (ms)", &ttl_ms);
        attach_row(&grid, 4, "Show mouse", &show_mouse);
        attach_row(&grid, 5, "Pause hotkey", &pause_hotkey);
        attach_row(&grid, 6, "Repeat coalesce (ms)", &repeat_coalesce_ms);
        attach_row(&grid, 7, "Modifier grace (ms)", &modifier_grace_ms);
        attach_row(&grid, 8, "Drag mode", &drag_enabled);
        attach_row(&grid, 9, "Custom X", &custom_x);
        attach_row(&grid, 10, "Custom Y", &custom_y);

        let status = Label::new(None);
        status.set_wrap(true);
        status.set_xalign(0.0);
        status.add_css_class("dim-label");

        let button_row = GtkBox::new(Orientation::Horizontal, 8);
        let apply_button = Button::with_label("Apply");
        let save_button = Button::with_label("Save");
        let close_button = Button::with_label("Close");

        button_row.append(&apply_button);
        button_row.append(&save_button);
        button_row.append(&close_button);

        content.append(&grid);
        content.append(&status);
        content.append(&button_row);

        window.set_child(Some(&content));

        Self {
            window,
            position,
            margin,
            max_items,
            ttl_ms,
            show_mouse,
            pause_hotkey,
            repeat_coalesce_ms,
            modifier_grace_ms,
            drag_enabled,
            custom_x,
            custom_y,
            status,
            apply_button,
            save_button,
            close_button,
        }
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn set_from_settings(&self, settings: &Settings) {
        self.position.set_selected(position_to_index(settings.position));
        self.margin.set_value(settings.margin as f64);
        self.max_items.set_value(settings.max_items as f64);
        self.ttl_ms.set_value(settings.ttl_ms as f64);
        self.show_mouse.set_active(settings.show_mouse);
        self.pause_hotkey.set_text(&settings.pause_hotkey);
        self.repeat_coalesce_ms
            .set_value(settings.repeat_coalesce_ms as f64);
        self.modifier_grace_ms
            .set_value(settings.modifier_grace_ms as f64);
        self.drag_enabled.set_active(settings.drag_enabled);
        self.custom_x.set_value(settings.custom_x as f64);
        self.custom_y.set_value(settings.custom_y as f64);
        self.set_status("");
    }

    pub fn read_settings(&self, base: &Settings) -> Settings {
        Settings {
            position: index_to_position(self.position.selected()),
            margin: self.margin.value() as i32,
            max_items: self.max_items.value() as usize,
            ttl_ms: self.ttl_ms.value() as u64,
            show_mouse: self.show_mouse.is_active(),
            pause_hotkey: self.pause_hotkey.text().to_string(),
            repeat_coalesce_ms: self.repeat_coalesce_ms.value() as u64,
            modifier_grace_ms: self.modifier_grace_ms.value() as u64,
            drag_enabled: self.drag_enabled.is_active(),
            custom_x: self.custom_x.value() as i32,
            custom_y: self.custom_y.value() as i32,
            ..base.clone()
        }
    }

    pub fn connect_apply<F: Fn() + 'static>(&self, callback: F) {
        self.apply_button.connect_clicked(move |_| callback());
    }

    pub fn connect_save<F: Fn() + 'static>(&self, callback: F) {
        self.save_button.connect_clicked(move |_| callback());
    }

    pub fn connect_close<F: Fn() + 'static>(&self, callback: F) {
        self.close_button.connect_clicked(move |_| callback());
    }

    pub fn set_status(&self, message: &str) {
        self.status.set_text(message);
    }
}

fn spin_i32(value: i32, min: i32, max: i32, step: i32) -> SpinButton {
    let adjustment = Adjustment::new(value as f64, min as f64, max as f64, step as f64, 10.0, 0.0);
    SpinButton::new(Some(&adjustment), 1.0, 0)
}

fn attach_row(grid: &Grid, row: i32, label: &str, widget: &impl IsA<gtk4::Widget>) {
    let lbl = Label::new(Some(label));
    lbl.set_xalign(0.0);
    grid.attach(&lbl, 0, row, 1, 1);
    grid.attach(widget, 1, row, 1, 1);
}

fn position_to_index(position: Position) -> u32 {
    match position {
        Position::BottomRight => 0,
        Position::BottomCenter => 1,
        Position::BottomLeft => 2,
        Position::TopRight => 3,
        Position::TopCenter => 4,
        Position::TopLeft => 5,
        Position::Center => 6,
        Position::Custom => 7,
    }
}

fn index_to_position(index: u32) -> Position {
    match index {
        1 => Position::BottomCenter,
        2 => Position::BottomLeft,
        3 => Position::TopRight,
        4 => Position::TopCenter,
        5 => Position::TopLeft,
        6 => Position::Center,
        7 => Position::Custom,
        _ => Position::BottomRight,
    }
}
