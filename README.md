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
