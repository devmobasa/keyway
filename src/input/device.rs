use anyhow::{Context, Result};
use evdev::{Device, EventType, Key};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct KeyboardDevice {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct MouseDevice {
    pub path: PathBuf,
    pub name: String,
}

pub fn discover_keyboards() -> Result<Vec<KeyboardDevice>> {
    let mut devices = Vec::new();
    let input_dir = PathBuf::from("/dev/input");

    let entries = fs::read_dir(&input_dir)
        .with_context(|| format!("Failed to read directory: {:?}", input_dir))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if !file_name.starts_with("event") {
            continue;
        }

        match Device::open(&path) {
            Ok(device) => {
                if is_keyboard(&device) {
                    let name = device.name().unwrap_or("Unknown Keyboard").to_string();
                    info!("Found keyboard: {} at {:?}", name, path);
                    devices.push(KeyboardDevice { path, name });
                }
            }
            Err(e) => {
                debug!("Could not open {:?}: {}", path, e);
            }
        }
    }

    if devices.is_empty() {
        warn!("No keyboard devices found. Ensure you have permission to read /dev/input/event* (input group).");
    }

    Ok(devices)
}

pub fn discover_mice() -> Result<Vec<MouseDevice>> {
    let mut devices = Vec::new();
    let input_dir = PathBuf::from("/dev/input");

    let entries = fs::read_dir(&input_dir)
        .with_context(|| format!("Failed to read directory: {:?}", input_dir))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if !file_name.starts_with("event") {
            continue;
        }

        match Device::open(&path) {
            Ok(device) => {
                if is_mouse(&device) {
                    let name = device.name().unwrap_or("Unknown Mouse").to_string();
                    info!("Found mouse: {} at {:?}", name, path);
                    devices.push(MouseDevice { path, name });
                }
            }
            Err(e) => {
                debug!("Could not open {:?}: {}", path, e);
            }
        }
    }

    if devices.is_empty() {
        warn!("No mouse devices found. Ensure you have permission to read /dev/input/event* (input group).");
    }

    Ok(devices)
}

fn is_keyboard(device: &Device) -> bool {
    let supported = device.supported_events();
    if !supported.contains(EventType::KEY) {
        return false;
    }

    if let Some(keys) = device.supported_keys() {
        return keys.contains(Key::KEY_A)
            && keys.contains(Key::KEY_Z)
            && keys.contains(Key::KEY_SPACE);
    }

    false
}

fn is_mouse(device: &Device) -> bool {
    let supported = device.supported_events();
    if !supported.contains(EventType::KEY) {
        return false;
    }

    if let Some(keys) = device.supported_keys() {
        return keys.contains(Key::BTN_LEFT) || keys.contains(Key::BTN_RIGHT);
    }

    false
}
