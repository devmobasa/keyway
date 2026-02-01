use anyhow::{bail, Result};
use evdev::Key;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Hotkey {
    ctrl: bool,
    shift: bool,
    alt: bool,
    super_key: bool,
    key: String,
}

impl Hotkey {
    pub fn parse(input: &str) -> Result<Self> {
        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut super_key = false;
        let mut key: Option<String> = None;

        for token in input.split('+').map(|t| t.trim()).filter(|t| !t.is_empty()) {
            let lower = token.to_ascii_lowercase();
            match lower.as_str() {
                "ctrl" | "control" => ctrl = true,
                "shift" => shift = true,
                "alt" | "option" => alt = true,
                "super" | "meta" | "cmd" | "command" | "win" | "logo" => super_key = true,
                _ => {
                    let normalized = normalize_key_token(token);
                    key = Some(normalized);
                }
            }
        }

        let key = match key {
            Some(key) => key,
            None => bail!("Hotkey requires a non-modifier key"),
        };

        Ok(Self {
            ctrl,
            shift,
            alt,
            super_key,
            key,
        })
    }

    pub fn matches(&self, held_mods: &HashSet<Key>, key_label: &str) -> bool {
        let normalized = normalize_key_token(key_label);
        if normalized != self.key {
            return false;
        }

        let has_ctrl = held_mods.contains(&Key::KEY_LEFTCTRL) || held_mods.contains(&Key::KEY_RIGHTCTRL);
        let has_shift = held_mods.contains(&Key::KEY_LEFTSHIFT) || held_mods.contains(&Key::KEY_RIGHTSHIFT);
        let has_alt = held_mods.contains(&Key::KEY_LEFTALT) || held_mods.contains(&Key::KEY_RIGHTALT);
        let has_super = held_mods.contains(&Key::KEY_LEFTMETA) || held_mods.contains(&Key::KEY_RIGHTMETA);

        self.ctrl == has_ctrl
            && self.shift == has_shift
            && self.alt == has_alt
            && self.super_key == has_super
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.super_key {
            parts.push("Super");
        }
        parts.push(self.key.as_str());
        parts.join("+")
    }
}

fn normalize_key_token(token: &str) -> String {
    let trimmed = token.trim();
    let lower = trimmed.to_ascii_lowercase();

    if lower.len() == 1 {
        let ch = lower.chars().next().unwrap();
        if ch.is_ascii_alphabetic() {
            return ch.to_ascii_uppercase().to_string();
        }
        return trimmed.to_string();
    }

    if lower.starts_with('f') && lower.len() <= 3 {
        return lower.to_ascii_uppercase();
    }

    match lower.as_str() {
        "esc" | "escape" => "Esc".to_string(),
        "enter" | "return" => "Enter".to_string(),
        "space" => "Space".to_string(),
        "tab" => "Tab".to_string(),
        "backspace" | "bksp" => "Backspace".to_string(),
        "del" | "delete" => "Del".to_string(),
        "ins" | "insert" => "Ins".to_string(),
        "pgup" | "pageup" => "PgUp".to_string(),
        "pgdn" | "pagedown" => "PgDn".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "left" => "Left".to_string(),
        "right" => "Right".to_string(),
        "up" => "Up".to_string(),
        "down" => "Down".to_string(),
        "prtsc" | "print" | "printscreen" => "PrtSc".to_string(),
        "plus" | "add" => "+".to_string(),
        "minus" | "dash" | "subtract" => "-".to_string(),
        "equal" | "equals" => "=".to_string(),
        "comma" => ",".to_string(),
        "period" | "dot" => ".".to_string(),
        "slash" => "/".to_string(),
        "backslash" => "\\".to_string(),
        "grave" | "backtick" => "`".to_string(),
        "apostrophe" | "quote" => "'".to_string(),
        "semicolon" => ";".to_string(),
        "leftbracket" | "lbracket" => "[".to_string(),
        "rightbracket" | "rbracket" => "]".to_string(),
        _ => token.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn parse_hotkey() {
        let hotkey = Hotkey::parse("Ctrl+Shift+P").unwrap();
        assert_eq!(hotkey.describe(), "Ctrl+Shift+P");
    }

    #[test]
    fn matches_exact_mods() {
        let hotkey = Hotkey::parse("Ctrl+P").unwrap();
        let mut mods = HashSet::new();
        mods.insert(Key::KEY_LEFTCTRL);
        assert!(hotkey.matches(&mods, "P"));

        mods.insert(Key::KEY_LEFTSHIFT);
        assert!(!hotkey.matches(&mods, "P"));
    }
}
