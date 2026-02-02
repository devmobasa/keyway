# keyway-visualizer

Minimal Wayland keystroke overlay using **evdev** for input and **GTK4 + layer-shell** for the on-screen display.

## Requirements

- Linux (Wayland)
- GTK4 + gtk4-layer-shell dev packages
- xkbcommon

### Arch Linux

```bash
sudo pacman -S gtk4 gtk-layer-shell xkbcommon
```

## Build & Run

```bash
cargo run
```

## Config

Default config path:

- `~/.config/keyway-visualizer/config.toml`

The file is created on first run. Example:

```toml
position = "bottom-right"
margin = 40
max_items = 5
ttl_ms = 900
show_mouse = true
pause_hotkey = "Ctrl+Shift+P"
repeat_coalesce_ms = 200
modifier_grace_ms = 120
drag_enabled = false
custom_x = 40
custom_y = 40
app_filter_enabled = false
disabled_apps = ["firefox", "org.keepassxc.keepassxc"]
```

You can override via CLI:

```bash
cargo run -- --position top-left --ttl-ms 1200 --show-mouse false
```

Positions supported: `bottom-right`, `bottom-center`, `bottom-left`, `top-right`, `top-center`, `top-left`, `center`, `custom`.
Use `custom` with `custom_x/custom_y` for pixel placement, or enable drag mode and move the overlay.

Hotkey parsing accepts tokens like `Ctrl+Shift+P`, `Super+F13`, and named keys like `Plus` or `Comma` for symbols.

## Settings UI

Open the system tray icon and choose **Settings**. Changes can be applied live or saved to the config file.

## Permissions

This app reads input events from `/dev/input/event*`.

If you see "No keyboard devices found" or permission errors:

```bash
sudo usermod -aG input $USER
# log out and back in
```

Alternatively, set up a udev rule to grant read access.

## Behavior

- Shows key combos (e.g., `Ctrl+Shift+A`) in a small overlay.
- Shows mouse clicks: `LMB`, `RMB`, `MMB`.
- Items fade out after ~900ms.
- Pause/resume capture via hotkey (default: `Ctrl+Shift+P`).
- System tray menu for pause/resume and quit.
- Drag mode to reposition the overlay (tray or settings).
- App filter to disable the overlay for specific apps.

## App Filter

App filtering uses `hyprctl -j activewindow` on Hyprland to match against the active window
`class` or `title` (case-insensitive). Add one string per line in Settings, or use:

```bash
cargo run -- --app-filter-enabled true --disabled-app firefox --disabled-app keepass
```

If `hyprctl` is not available, the filter is ignored.

## Packaging (manual)

This repo includes example files you can adapt:

- `packaging/keyway-visualizer.desktop`
- `packaging/keyway-visualizer.service`
