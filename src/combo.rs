use crate::input::InputEvent;
use crate::xkb::{is_modifier, key_label, XkbState};
use evdev::Key;
use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ComboItem {
    pub text: String,
    pub at: Instant,
}

pub struct ComboState {
    held_mods: HashSet<Key>,
    items: VecDeque<ComboItem>,
    max_items: usize,
    ttl: Duration,
    xkb: XkbState,
}

impl ComboState {
    pub fn new(max_items: usize, ttl: Duration) -> Self {
        Self {
            held_mods: HashSet::new(),
            items: VecDeque::new(),
            max_items,
            ttl,
            xkb: XkbState::new(),
        }
    }

    pub fn handle_event(&mut self, event: InputEvent) -> bool {
        let mut changed = false;

        match event {
            InputEvent::KeyPressed(key) => {
                self.xkb.update_key(key, true);
                if is_modifier(key) {
                    self.held_mods.insert(key);
                } else {
                    let label = key_label(key, &self.xkb);
                    let combo = format_combo(&self.held_mods, &label);
                    changed |= self.push_combo(combo);
                }
            }
            InputEvent::KeyRepeat(key) => {
                self.xkb.update_key(key, true);
                if !is_modifier(key) {
                    let label = key_label(key, &self.xkb);
                    let combo = format_combo(&self.held_mods, &label);
                    changed |= self.push_combo(combo);
                }
            }
            InputEvent::KeyReleased(key) => {
                self.xkb.update_key(key, false);
                if is_modifier(key) {
                    self.held_mods.remove(&key);
                }
            }
            InputEvent::MouseButtonPressed(key) => {
                if let Some(label) = mouse_label(key) {
                    changed |= self.push_combo(label.to_string());
                }
            }
            InputEvent::MouseButtonReleased => {}
        }

        changed
    }

    pub fn prune_expired(&mut self) -> bool {
        let now = Instant::now();
        let mut changed = false;

        while let Some(front) = self.items.front() {
            if now.duration_since(front.at) > self.ttl {
                self.items.pop_front();
                changed = true;
            } else {
                break;
            }
        }

        changed
    }

    pub fn items(&self) -> &VecDeque<ComboItem> {
        &self.items
    }

    fn push_combo(&mut self, text: String) -> bool {
        self.items.push_back(ComboItem {
            text,
            at: Instant::now(),
        });

        while self.items.len() > self.max_items {
            self.items.pop_front();
        }

        true
    }
}

fn format_combo(held_mods: &HashSet<Key>, key_label: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();

    if has_ctrl(held_mods) {
        parts.push("Ctrl");
    }
    if has_shift(held_mods) {
        parts.push("Shift");
    }
    if has_alt(held_mods) {
        parts.push("Alt");
    }
    if has_super(held_mods) {
        parts.push("Super");
    }

    parts.push(key_label);
    parts.join("+")
}

fn has_ctrl(mods: &HashSet<Key>) -> bool {
    mods.contains(&Key::KEY_LEFTCTRL) || mods.contains(&Key::KEY_RIGHTCTRL)
}

fn has_shift(mods: &HashSet<Key>) -> bool {
    mods.contains(&Key::KEY_LEFTSHIFT) || mods.contains(&Key::KEY_RIGHTSHIFT)
}

fn has_alt(mods: &HashSet<Key>) -> bool {
    mods.contains(&Key::KEY_LEFTALT) || mods.contains(&Key::KEY_RIGHTALT)
}

fn has_super(mods: &HashSet<Key>) -> bool {
    mods.contains(&Key::KEY_LEFTMETA) || mods.contains(&Key::KEY_RIGHTMETA)
}

fn mouse_label(key: Key) -> Option<&'static str> {
    match key {
        Key::BTN_LEFT => Some("LMB"),
        Key::BTN_RIGHT => Some("RMB"),
        Key::BTN_MIDDLE => Some("MMB"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_combo_orders_mods() {
        let mut mods = HashSet::new();
        mods.insert(Key::KEY_LEFTALT);
        mods.insert(Key::KEY_LEFTCTRL);
        mods.insert(Key::KEY_LEFTSHIFT);

        let combo = format_combo(&mods, "A");
        assert_eq!(combo, "Ctrl+Shift+Alt+A");
    }
}
