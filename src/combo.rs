use crate::hotkey::Hotkey;
use crate::input::InputEvent;
use crate::xkb::{is_modifier, key_label, XkbState};
use evdev::Key;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ComboItem {
    pub text: String,
    pub at: Instant,
}

pub struct ComboState {
    held_mods: HashSet<Key>,
    mod_release_at: HashMap<Key, Instant>,
    items: VecDeque<ComboItem>,
    max_items: usize,
    ttl: Duration,
    repeat_coalesce: Duration,
    modifier_grace: Duration,
    paused: bool,
    pause_hotkey: Hotkey,
    xkb: XkbState,
}

impl ComboState {
    pub fn new(
        max_items: usize,
        ttl: Duration,
        repeat_coalesce: Duration,
        modifier_grace: Duration,
        pause_hotkey: Hotkey,
    ) -> Self {
        Self {
            held_mods: HashSet::new(),
            mod_release_at: HashMap::new(),
            items: VecDeque::new(),
            max_items,
            ttl,
            repeat_coalesce,
            modifier_grace,
            paused: false,
            pause_hotkey,
            xkb: XkbState::new(),
        }
    }

    pub fn handle_event(&mut self, event: InputEvent) -> ComboAction {
        let now = Instant::now();
        let mut action = ComboAction::default();

        self.prune_mods(now);

        match event {
            InputEvent::KeyPressed(key) => {
                self.xkb.update_key(key, true);
                if is_modifier(key) {
                    self.held_mods.insert(key);
                    self.mod_release_at.remove(&key);
                } else {
                    let label = key_label(key, &self.xkb);

                    if self.pause_hotkey.matches(&self.held_mods, &label) {
                        self.toggle_pause();
                        action.paused_changed = Some(self.paused());
                        action.render = true;
                        return action;
                    }

                    if self.paused {
                        return action;
                    }

                    let combo = format_combo(&self.held_mods, &label);
                    action.render |= self.push_combo(combo, now);
                }
            }
            InputEvent::KeyRepeat(key) => {
                self.xkb.update_key(key, true);
                if self.paused {
                    return action;
                }
                if !is_modifier(key) {
                    let label = key_label(key, &self.xkb);
                    let combo = format_combo(&self.held_mods, &label);
                    action.render |= self.push_combo(combo, now);
                }
            }
            InputEvent::KeyReleased(key) => {
                self.xkb.update_key(key, false);
                if is_modifier(key) {
                    self.mod_release_at.insert(key, now);
                }
            }
            InputEvent::MouseButtonPressed(key) => {
                if self.paused {
                    return action;
                }
                if let Some(label) = mouse_label(key) {
                    action.render |= self.push_combo(label.to_string(), now);
                }
            }
            InputEvent::MouseButtonReleased => {}
        }

        action
    }

    pub fn prune_expired(&mut self) -> bool {
        let now = Instant::now();
        let mut changed = false;

        self.prune_mods(now);

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

    pub fn clear_items(&mut self) {
        self.items.clear();
    }

    pub fn handle_event_suppressed(&mut self, event: InputEvent) {
        match event {
            InputEvent::KeyPressed(key) => {
                self.xkb.update_key(key, true);
                if is_modifier(key) {
                    self.held_mods.insert(key);
                    self.mod_release_at.remove(&key);
                }
            }
            InputEvent::KeyReleased(key) => {
                self.xkb.update_key(key, false);
                if is_modifier(key) {
                    self.mod_release_at.insert(key, Instant::now());
                }
            }
            InputEvent::KeyRepeat(key) => {
                self.xkb.update_key(key, true);
            }
            InputEvent::MouseButtonPressed(_) | InputEvent::MouseButtonReleased => {}
        }
    }

    pub fn toggle_pause(&mut self) -> bool {
        self.set_paused(!self.paused)
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn update_settings(
        &mut self,
        max_items: usize,
        ttl: Duration,
        repeat_coalesce: Duration,
        modifier_grace: Duration,
        pause_hotkey: Hotkey,
    ) {
        self.max_items = max_items;
        self.ttl = ttl;
        self.repeat_coalesce = repeat_coalesce;
        self.modifier_grace = modifier_grace;
        self.pause_hotkey = pause_hotkey;

        while self.items.len() > self.max_items {
            self.items.pop_front();
        }
    }

    fn set_paused(&mut self, paused: bool) -> bool {
        if self.paused == paused {
            return false;
        }

        self.paused = paused;
        let label = if paused { "Paused" } else { "Resumed" };
        let _ = self.push_combo(label.to_string(), Instant::now());
        true
    }

    fn push_combo(&mut self, text: String, now: Instant) -> bool {
        if let Some(back) = self.items.back_mut() {
            if back.text == text && now.duration_since(back.at) <= self.repeat_coalesce {
                back.at = now;
                return true;
            }
        }

        self.items.push_back(ComboItem { text, at: now });

        while self.items.len() > self.max_items {
            self.items.pop_front();
        }

        true
    }

    fn prune_mods(&mut self, now: Instant) {
        let grace = self.modifier_grace;
        let mut expired = Vec::new();
        for (key, released_at) in &self.mod_release_at {
            if now.duration_since(*released_at) > grace {
                expired.push(*key);
            }
        }

        for key in expired {
            self.mod_release_at.remove(&key);
            self.held_mods.remove(&key);
        }
    }
}

#[derive(Default, Debug)]
pub struct ComboAction {
    pub render: bool,
    pub paused_changed: Option<bool>,
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
