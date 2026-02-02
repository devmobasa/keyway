use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "keyway-visualizer")]
#[command(about = "Minimal Wayland keystroke overlay")]
pub struct CliArgs {
    /// Path to config file
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Overlay position (e.g. bottom-right)
    #[arg(long, value_enum)]
    pub position: Option<Position>,

    /// Window margin in pixels
    #[arg(long)]
    pub margin: Option<i32>,

    /// Max number of items to show
    #[arg(long)]
    pub max_items: Option<usize>,

    /// TTL for each combo in milliseconds
    #[arg(long)]
    pub ttl_ms: Option<u64>,

    /// Show mouse clicks (true/false)
    #[arg(long)]
    pub show_mouse: Option<bool>,

    /// Pause/resume hotkey (e.g. "Ctrl+Shift+P")
    #[arg(long)]
    pub pause_hotkey: Option<String>,

    /// Coalesce repeated combos within this many ms
    #[arg(long)]
    pub repeat_coalesce_ms: Option<u64>,

    /// Keep modifiers active this long after release (ms)
    #[arg(long)]
    pub modifier_grace_ms: Option<u64>,

    /// Enable drag-to-position mode
    #[arg(long)]
    pub drag_enabled: Option<bool>,

    /// Custom X position (pixels from left) when position=custom
    #[arg(long)]
    pub custom_x: Option<i32>,

    /// Custom Y position (pixels from top) when position=custom
    #[arg(long)]
    pub custom_y: Option<i32>,

    /// Enable per-app filtering
    #[arg(long)]
    pub app_filter_enabled: Option<bool>,

    /// Disable overlay for matching app class/title (repeatable)
    #[arg(long = "disabled-app")]
    pub disabled_apps: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
pub enum Position {
    BottomRight,
    BottomCenter,
    BottomLeft,
    TopRight,
    TopCenter,
    TopLeft,
    Center,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub position: Position,
    pub margin: i32,
    pub max_items: usize,
    pub ttl_ms: u64,
    pub show_mouse: bool,
    pub pause_hotkey: String,
    pub repeat_coalesce_ms: u64,
    pub modifier_grace_ms: u64,
    pub drag_enabled: bool,
    pub custom_x: i32,
    pub custom_y: i32,
    pub app_filter_enabled: bool,
    pub disabled_apps: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            position: Position::BottomRight,
            margin: 40,
            max_items: 5,
            ttl_ms: 900,
            show_mouse: true,
            pause_hotkey: "Ctrl+Shift+P".to_string(),
            repeat_coalesce_ms: 200,
            modifier_grace_ms: 120,
            drag_enabled: false,
            custom_x: 40,
            custom_y: 40,
            app_filter_enabled: false,
            disabled_apps: Vec::new(),
        }
    }
}

impl Settings {
    pub fn load(cli: &CliArgs) -> Result<(Self, PathBuf)> {
        let path = cli.config.clone().unwrap_or_else(default_config_path);
        let mut settings = if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config: {:?}", path))?;

            match toml::from_str::<Settings>(&raw) {
                Ok(parsed) => parsed,
                Err(e) => {
                    warn!("Failed to parse config {:?}: {}", path, e);
                    Settings::default()
                }
            }
        } else {
            Settings::default()
        };

        settings.apply_cli(cli);

        if !path.exists() {
            settings.save_to(&path)?;
        }

        info!("Config path: {:?}", path);

        Ok((settings, path))
    }

    fn apply_cli(&mut self, cli: &CliArgs) {
        if let Some(position) = cli.position {
            self.position = position;
        }
        if let Some(margin) = cli.margin {
            self.margin = margin;
        }
        if let Some(max_items) = cli.max_items {
            self.max_items = max_items;
        }
        if let Some(ttl_ms) = cli.ttl_ms {
            self.ttl_ms = ttl_ms;
        }
        if let Some(show_mouse) = cli.show_mouse {
            self.show_mouse = show_mouse;
        }
        if let Some(pause_hotkey) = cli.pause_hotkey.clone() {
            self.pause_hotkey = pause_hotkey;
        }
        if let Some(repeat_coalesce_ms) = cli.repeat_coalesce_ms {
            self.repeat_coalesce_ms = repeat_coalesce_ms;
        }
        if let Some(modifier_grace_ms) = cli.modifier_grace_ms {
            self.modifier_grace_ms = modifier_grace_ms;
        }
        if let Some(drag_enabled) = cli.drag_enabled {
            self.drag_enabled = drag_enabled;
        }
        if let Some(custom_x) = cli.custom_x {
            self.custom_x = custom_x;
        }
        if let Some(custom_y) = cli.custom_y {
            self.custom_y = custom_y;
        }
        if let Some(app_filter_enabled) = cli.app_filter_enabled {
            self.app_filter_enabled = app_filter_enabled;
        }
        if !cli.disabled_apps.is_empty() {
            self.disabled_apps = cli
                .disabled_apps
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config dir: {:?}", parent))?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(path, content)
            .with_context(|| format!("Failed to write config: {:?}", path))?;

        Ok(())
    }
}

fn default_config_path() -> PathBuf {
    if let Some(dir) = dirs::config_dir() {
        dir.join("keyway-visualizer").join("config.toml")
    } else {
        PathBuf::from("config.toml")
    }
}
