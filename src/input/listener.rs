use crate::input::device::{discover_keyboards, discover_mice, KeyboardDevice, MouseDevice};
use anyhow::{Context, Result};
use async_channel::{Sender, TrySendError};
use evdev::{Device, InputEventKind, Key};
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use std::collections::HashSet;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::{error, info, trace, warn};

#[derive(Debug, Clone)]
pub enum InputEvent {
    KeyPressed(Key),
    KeyReleased(Key),
    KeyRepeat(Key),
    MouseButtonPressed(Key),
    MouseButtonReleased,
}

#[derive(Debug, Clone)]
pub struct ListenerConfig {
    pub all_keyboards: bool,
    pub include_mouse: bool,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            all_keyboards: true,
            include_mouse: true,
        }
    }
}

pub struct ListenerHandle {
    running: Arc<AtomicBool>,
}

impl Drop for ListenerHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

pub struct InputListener {
    sender: Sender<InputEvent>,
    running: Arc<AtomicBool>,
    config: ListenerConfig,
}

#[derive(Clone)]
struct ListenerDevice {
    path: PathBuf,
    name: String,
    kind: &'static str,
    include_mouse_buttons: bool,
}

impl ListenerDevice {
    fn keyboard(device: KeyboardDevice, include_mouse_buttons: bool) -> Self {
        Self {
            path: device.path,
            name: device.name,
            kind: "keyboard",
            include_mouse_buttons,
        }
    }

    fn mouse(device: MouseDevice) -> Self {
        Self {
            path: device.path,
            name: device.name,
            kind: "mouse",
            include_mouse_buttons: true,
        }
    }

    fn open(&self) -> Result<Device> {
        Device::open(&self.path).with_context(|| format!("Failed to open device: {:?}", self.path))
    }
}

impl InputListener {
    #[must_use]
    pub fn new(sender: Sender<InputEvent>, config: ListenerConfig) -> Self {
        Self {
            sender,
            running: Arc::new(AtomicBool::new(false)),
            config,
        }
    }

    pub fn start(&self) -> Result<ListenerHandle> {
        let keyboards = discover_keyboards()?;
        if keyboards.is_empty() {
            anyhow::bail!(
                "No keyboard devices found. Ensure you are in the 'input' group or have permission for /dev/input/event*"
            );
        }

        let devices_to_use: Vec<KeyboardDevice> = if self.config.all_keyboards {
            keyboards
        } else {
            keyboards.into_iter().take(1).collect()
        };

        let mut devices: Vec<ListenerDevice> = devices_to_use
            .into_iter()
            .map(|d| ListenerDevice::keyboard(d, self.config.include_mouse))
            .collect();

        if self.config.include_mouse {
            match discover_mice() {
                Ok(mice) => {
                    let keyboard_paths: HashSet<PathBuf> = devices
                        .iter()
                        .map(|d| d.path.clone())
                        .collect();

                    devices.extend(
                        mice.into_iter()
                            .filter(|m| !keyboard_paths.contains(&m.path))
                            .map(ListenerDevice::mouse),
                    );
                }
                Err(e) => warn!("Failed to discover mouse devices: {}", e),
            }
        }

        self.running.store(true, Ordering::SeqCst);

        for device in devices {
            let sender = self.sender.clone();
            let running = Arc::clone(&self.running);

            thread::spawn(move || {
                if let Err(e) = listen_device(device, sender, running) {
                    error!("Input listener error: {}", e);
                }
            });
        }

        Ok(ListenerHandle {
            running: self.running.clone(),
        })
    }
}

fn listen_device(device_info: ListenerDevice, sender: Sender<InputEvent>, running: Arc<AtomicBool>) -> Result<()> {
    let mut device = device_info.open()?;
    info!("Listening to {}: {}", device_info.kind, device_info.name);

    let raw_fd = device.as_raw_fd();
    let borrowed_fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };
    let mut poll_fds = [PollFd::new(borrowed_fd, PollFlags::POLLIN)];

    let mut pressed_keys: HashSet<Key> = HashSet::new();

    while running.load(Ordering::SeqCst) {
        let poll_result = poll(&mut poll_fds, PollTimeout::from(100_u16));

        match poll_result {
            Ok(_) => {
                if let Err(e) = process_events(&mut device, &sender, device_info.include_mouse_buttons, &mut pressed_keys) {
                    if e.to_string().contains("Channel closed") {
                        info!("Channel closed, stopping listener for {}", device_info.name);
                        break;
                    }
                    warn!("Error processing events: {}", e);
                }
            }
            Err(e) => {
                error!("Poll error: {}", e);
                break;
            }
        }
    }

    info!("Stopped listening to {}: {}", device_info.kind, device_info.name);
    Ok(())
}

fn process_events(
    device: &mut Device,
    sender: &Sender<InputEvent>,
    include_mouse_buttons: bool,
    pressed_keys: &mut HashSet<Key>,
) -> Result<()> {
    let events = device.fetch_events().context("Failed to fetch events")?;
    let mut activity = false;

    for event in events {
        if let InputEventKind::Key(key) = event.kind() {
            let value = event.value();

            if is_mouse_button(key) {
                if !include_mouse_buttons {
                    continue;
                }

                let mouse_event = match value {
                    1 => Some(InputEvent::MouseButtonPressed(key)),
                    0 => Some(InputEvent::MouseButtonReleased),
                    _ => None,
                };

                if let Some(mouse_event) = mouse_event {
                    send_event(sender, mouse_event)?;
                }

                continue;
            }

            activity = true;
            let key_event = match value {
                1 => {
                    trace!("Key pressed: {:?}", key);
                    pressed_keys.insert(key);
                    InputEvent::KeyPressed(key)
                }
                0 => {
                    trace!("Key released: {:?}", key);
                    pressed_keys.remove(&key);
                    InputEvent::KeyReleased(key)
                }
                2 => {
                    trace!("Key repeat: {:?}", key);
                    InputEvent::KeyRepeat(key)
                }
                _ => continue,
            };

            send_event(sender, key_event)?;
        }
    }

    if activity && !pressed_keys.is_empty() {
        if let Ok(actual_state) = device.get_key_state() {
            let stuck: Vec<Key> = pressed_keys
                .iter()
                .filter(|k| !actual_state.contains(**k))
                .cloned()
                .collect();

            for key in stuck {
                pressed_keys.remove(&key);
                let _ = send_event(sender, InputEvent::KeyReleased(key));
            }
        }
    }

    Ok(())
}

fn send_event(sender: &Sender<InputEvent>, event: InputEvent) -> Result<()> {
    if let Err(e) = sender.try_send(event) {
        match e {
            TrySendError::Full(_) => warn!("Channel full, dropping event"),
            TrySendError::Closed(_) => return Err(anyhow::anyhow!("Channel closed")),
        }
    }
    Ok(())
}

fn is_mouse_button(key: Key) -> bool {
    matches!(key, Key::BTN_LEFT | Key::BTN_RIGHT | Key::BTN_MIDDLE)
}
