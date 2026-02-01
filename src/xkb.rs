use evdev::Key;
use xkbcommon::xkb;

const EVDEV_OFFSET: u32 = 8;

pub struct XkbState {
    _context: xkb::Context,
    _keymap: xkb::Keymap,
    state: xkb::State,
}

impl XkbState {
    pub fn new() -> Self {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("Failed to create XKB keymap (is xkbcommon installed?)");

        let state = xkb::State::new(&keymap);

        Self {
            _context: context,
            _keymap: keymap,
            state,
        }
    }

    pub fn update_key(&mut self, key: Key, pressed: bool) {
        let keycode = key_to_keycode(key);
        let direction = if pressed {
            xkb::KeyDirection::Down
        } else {
            xkb::KeyDirection::Up
        };
        self.state.update_key(keycode, direction);
    }

    pub fn key_get_utf8(&self, key: Key) -> Option<String> {
        let keycode = key_to_keycode(key);
        let utf8 = self.state.key_get_utf8(keycode);
        if utf8.is_empty() {
            None
        } else {
            Some(utf8)
        }
    }
}

fn key_to_keycode(key: Key) -> xkb::Keycode {
    let evdev_code = key.code() as u32;
    xkb::Keycode::new(evdev_code + EVDEV_OFFSET)
}

pub fn is_modifier(key: Key) -> bool {
    matches!(
        key,
        Key::KEY_LEFTCTRL
            | Key::KEY_RIGHTCTRL
            | Key::KEY_LEFTSHIFT
            | Key::KEY_RIGHTSHIFT
            | Key::KEY_LEFTALT
            | Key::KEY_RIGHTALT
            | Key::KEY_LEFTMETA
            | Key::KEY_RIGHTMETA
    )
}

pub fn key_label(key: Key, state: &XkbState) -> String {
    if let Some(label) = special_key_label(key) {
        return label.to_string();
    }

    if let Some(utf8) = state.key_get_utf8(key) {
        if utf8 == " " {
            return "Space".to_string();
        }

        if utf8.len() == 1 {
            return utf8.to_uppercase();
        }

        return utf8;
    }

    fallback_label(key)
}

fn special_key_label(key: Key) -> Option<&'static str> {
    match key {
        Key::KEY_ENTER | Key::KEY_KPENTER => Some("Enter"),
        Key::KEY_ESC => Some("Esc"),
        Key::KEY_BACKSPACE => Some("Backspace"),
        Key::KEY_TAB => Some("Tab"),
        Key::KEY_CAPSLOCK => Some("Caps"),
        Key::KEY_SPACE => Some("Space"),
        Key::KEY_LEFT => Some("Left"),
        Key::KEY_RIGHT => Some("Right"),
        Key::KEY_UP => Some("Up"),
        Key::KEY_DOWN => Some("Down"),
        Key::KEY_DELETE => Some("Del"),
        Key::KEY_HOME => Some("Home"),
        Key::KEY_END => Some("End"),
        Key::KEY_PAGEUP => Some("PgUp"),
        Key::KEY_PAGEDOWN => Some("PgDn"),
        Key::KEY_INSERT => Some("Ins"),
        Key::KEY_PRINT => Some("PrtSc"),
        Key::KEY_SYSRQ => Some("SysRq"),
        Key::KEY_PAUSE => Some("Pause"),
        Key::KEY_NUMLOCK => Some("Num"),
        Key::KEY_SCROLLLOCK => Some("Scroll"),
        _ => None,
    }
}

fn fallback_label(key: Key) -> String {
    let name = format!("{:?}", key);
    if let Some(stripped) = name.strip_prefix("KEY_") {
        return stripped.to_string();
    }
    if let Some(stripped) = name.strip_prefix("BTN_") {
        return stripped.to_string();
    }
    name
}
