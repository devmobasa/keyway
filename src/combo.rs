use crate::hotkey::Hotkey;
use crate::input::InputEvent;
use crate::xkb::{is_modifier, key_label, XkbState};
use evdev::Key;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

/// Repeat-counter separator, matching Wayscriber's input HUD (`Backspace ×5`).
const REPEAT_SEPARATOR: &str = " \u{00d7}";

#[derive(Debug, Clone)]
pub struct ComboItem {
    pub text: String,
    pub count: u32,
    pub at: Instant,
}

impl ComboItem {
    pub fn display_text(&self) -> String {
        display_combo_text(&self.text, self.count)
    }

    pub fn is_status(&self) -> bool {
        self.text == "Paused" || self.text == "Resumed"
    }
}

fn display_combo_text(label: &str, count: u32) -> String {
    if count > 1 {
        format!("{label}{REPEAT_SEPARATOR}{count}")
    } else {
        label.to_string()
    }
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
        self.handle_event_at(event, Instant::now())
    }

    fn handle_event_at(&mut self, event: InputEvent, now: Instant) -> ComboAction {
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
                        self.toggle_pause_at(now);
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
                    let combo = format_combo(&self.held_mods, label);
                    action.render |= self.push_combo(combo, now);
                }
            }
            InputEvent::MouseButtonReleased => {}
        }

        action
    }

    pub fn prune_expired(&mut self) -> bool {
        self.prune_expired_at(Instant::now())
    }

    fn prune_expired_at(&mut self, now: Instant) -> bool {
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
        self.toggle_pause_at(Instant::now())
    }

    fn toggle_pause_at(&mut self, now: Instant) -> bool {
        self.set_paused(!self.paused, now)
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

    fn set_paused(&mut self, paused: bool, now: Instant) -> bool {
        if self.paused == paused {
            return false;
        }

        self.paused = paused;
        let label = if paused { "Paused" } else { "Resumed" };
        let _ = self.push_combo(label.to_string(), now);
        true
    }

    fn push_combo(&mut self, text: String, now: Instant) -> bool {
        if self.should_coalesce(&text, now) {
            if let Some(back) = self.items.back_mut() {
                back.count = back.count.saturating_add(1);
                back.at = now;
                return true;
            }
        }

        self.items.push_back(ComboItem {
            text,
            count: 1,
            at: now,
        });

        while self.items.len() > self.max_items {
            self.items.pop_front();
        }

        true
    }

    /// Stack consecutive identical chips while the last one is still on screen
    /// and the gap since its last press is within `repeat_coalesce`.
    /// `repeat_coalesce_ms == 0` turns combining off.
    fn should_coalesce(&self, text: &str, now: Instant) -> bool {
        if self.repeat_coalesce.is_zero() {
            return false;
        }
        let window = self.repeat_coalesce.min(self.ttl);
        self.items
            .back()
            .is_some_and(|newest| newest.text == text && now.duration_since(newest.at) <= window)
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

    fn test_state() -> ComboState {
        ComboState::new(
            5,
            Duration::from_millis(900),
            Duration::from_millis(900),
            Duration::from_millis(120),
            Hotkey::parse("Ctrl+Shift+P").unwrap(),
        )
    }

    fn press_at(state: &mut ComboState, key: Key, at: Instant) {
        state.handle_event_at(InputEvent::KeyPressed(key), at);
        state.handle_event_at(InputEvent::KeyReleased(key), at + Duration::from_millis(10));
    }

    #[test]
    fn format_combo_orders_mods() {
        let mut mods = HashSet::new();
        mods.insert(Key::KEY_LEFTALT);
        mods.insert(Key::KEY_LEFTCTRL);
        mods.insert(Key::KEY_LEFTSHIFT);

        let combo = format_combo(&mods, "A");
        assert_eq!(combo, "Ctrl+Shift+Alt+A");
    }

    #[test]
    fn display_combo_text_omits_the_counter_for_a_single_press() {
        assert_eq!(display_combo_text("Backspace", 1), "Backspace");
        assert_eq!(display_combo_text("Backspace", 5), "Backspace ×5");
    }

    #[test]
    fn repeated_backspaces_coalesce_into_a_counter() {
        let mut state = test_state();
        let start = Instant::now();
        for i in 0..5 {
            press_at(
                &mut state,
                Key::KEY_BACKSPACE,
                start + Duration::from_millis(i * 40),
            );
        }

        assert_eq!(state.items().len(), 1);
        let item = state.items().front().unwrap();
        assert_eq!(item.text, "Backspace");
        assert_eq!(item.count, 5);
        assert_eq!(item.display_text(), "Backspace ×5");
    }

    #[test]
    fn key_repeat_increments_the_same_chip() {
        let mut state = test_state();
        let start = Instant::now();
        state.handle_event_at(InputEvent::KeyPressed(Key::KEY_BACKSPACE), start);
        state.handle_event_at(
            InputEvent::KeyRepeat(Key::KEY_BACKSPACE),
            start + Duration::from_millis(30),
        );
        state.handle_event_at(
            InputEvent::KeyRepeat(Key::KEY_BACKSPACE),
            start + Duration::from_millis(60),
        );

        let item = state.items().front().unwrap();
        assert_eq!(item.count, 3);
        assert_eq!(item.display_text(), "Backspace ×3");
    }

    #[test]
    fn a_different_key_starts_a_new_chip() {
        let mut state = test_state();
        let start = Instant::now();
        press_at(&mut state, Key::KEY_BACKSPACE, start);
        press_at(
            &mut state,
            Key::KEY_ENTER,
            start + Duration::from_millis(40),
        );

        let labels: Vec<_> = state
            .items()
            .iter()
            .map(|item| (item.text.as_str(), item.count))
            .collect();
        assert_eq!(labels, vec![("Backspace", 1), ("Enter", 1)]);
    }

    #[test]
    fn repeats_outside_the_coalesce_window_stay_separate() {
        let mut state = ComboState::new(
            5,
            Duration::from_millis(900),
            Duration::from_millis(80),
            Duration::from_millis(120),
            Hotkey::parse("Ctrl+Shift+P").unwrap(),
        );
        let start = Instant::now();
        press_at(&mut state, Key::KEY_A, start);
        press_at(&mut state, Key::KEY_A, start + Duration::from_millis(200));

        assert_eq!(state.items().len(), 2);
        assert!(state.items().iter().all(|item| item.count == 1));
    }

    #[test]
    fn zero_coalesce_window_keeps_repeats_separate() {
        let mut state = ComboState::new(
            5,
            Duration::from_millis(900),
            Duration::ZERO,
            Duration::from_millis(120),
            Hotkey::parse("Ctrl+Shift+P").unwrap(),
        );
        let start = Instant::now();
        press_at(&mut state, Key::KEY_A, start);
        press_at(&mut state, Key::KEY_A, start + Duration::from_millis(10));

        assert_eq!(state.items().len(), 2);
    }

    #[test]
    fn mouse_clicks_include_held_modifiers() {
        let mut state = test_state();
        let start = Instant::now();
        state.handle_event_at(InputEvent::KeyPressed(Key::KEY_LEFTCTRL), start);
        state.handle_event_at(InputEvent::MouseButtonPressed(Key::BTN_LEFT), start);

        let item = state.items().front().unwrap();
        assert_eq!(item.text, "Ctrl+LMB");
        assert_eq!(item.count, 1);
    }
}
