use std::borrow::Cow;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const APP_ID: &str = "io.github.cosmic.AppletUnsplash";
pub const CONFIG_VERSION: u64 = 1;
pub const SETTINGS_KEY: &str = "settings";

/// Returns the user's real `~/.config` directory, even when running inside a Flatpak sandbox.
pub fn host_config_dir() -> PathBuf {
    if env::var_os("FLATPAK_ID").is_some() {
        if let Some(host_config) = env::var_os("HOST_XDG_CONFIG_HOME") {
            return PathBuf::from(host_config);
        }
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join(".config");
        }
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Orientation filter supported by the Unsplash `/photos/random` endpoint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    Any,
    #[default]
    Landscape,
    Portrait,
    Squarish,
}

impl Orientation {
    pub const ALL: [Self; 4] = [Self::Any, Self::Landscape, Self::Portrait, Self::Squarish];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Any => "Any",
            Self::Landscape => "Landscape",
            Self::Portrait => "Portrait",
            Self::Squarish => "Squarish",
        }
    }

    pub const fn api_param(self) -> Option<&'static str> {
        match self {
            Self::Any => None,
            Self::Landscape => Some("landscape"),
            Self::Portrait => Some("portrait"),
            Self::Squarish => Some("squarish"),
        }
    }
}

impl FromStr for Orientation {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "any" => Ok(Self::Any),
            "landscape" => Ok(Self::Landscape),
            "portrait" => Ok(Self::Portrait),
            "squarish" => Ok(Self::Squarish),
            other => Err(AppError::CliUsage(format!(
                "Invalid orientation '{other}' (expected: any, landscape, portrait, squarish)"
            ))),
        }
    }
}

/// Content safety filter supported by the Unsplash `/photos/random` endpoint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentFilter {
    #[default]
    Low,
    High,
}

impl ContentFilter {
    pub const ALL: [Self; 2] = [Self::Low, Self::High];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Standard (Low)",
            Self::High => "Strict (High)",
        }
    }

    pub const fn api_param(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::High => "high",
        }
    }
}

impl FromStr for ContentFilter {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "high" => Ok(Self::High),
            other => Err(AppError::CliUsage(format!(
                "Invalid content filter '{other}' (expected: low, high)"
            ))),
        }
    }
}

/// Target download resolution / Unsplash image URL variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageResolution {
    #[default]
    Full,
    Uhd4K,
    Qhd1440p,
    Regular1080p,
    Raw,
}

impl ImageResolution {
    pub const ALL: [Self; 5] = [
        Self::Full,
        Self::Uhd4K,
        Self::Qhd1440p,
        Self::Regular1080p,
        Self::Raw,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Uhd4K => "4K UHD",
            Self::Qhd1440p => "1440p",
            Self::Regular1080p => "1080p",
            Self::Raw => "Raw",
        }
    }
}

/// Wallpaper scaling mode for `cosmic-bg`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallpaperScalingMode {
    #[default]
    Zoom,
    Fit,
    Stretch,
}

impl WallpaperScalingMode {
    pub const ALL: [Self; 3] = [Self::Zoom, Self::Fit, Self::Stretch];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Zoom => "Zoom (Fill)",
            Self::Fit => "Fit",
            Self::Stretch => "Stretch",
        }
    }

    pub const fn cosmic_bg_ron(self) -> &'static str {
        match self {
            Self::Zoom => "Zoom",
            Self::Fit => "Fit((0.0, 0.0, 0.0))",
            Self::Stretch => "Stretch",
        }
    }
}

/// Metadata about the most recently applied Unsplash photo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastPhotoInfo {
    pub id: String,
    pub photographer: String,
    pub username: String,
    pub description: Option<String>,
    pub width: u32,
    pub height: u32,
    pub file_path: PathBuf,
}

/// Persistent settings for the COSMIC Unsplash Wallpaper Applet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnsplashSettings {
    /// Unsplash API Access Key (`Client-ID`).
    pub access_key: String,
    /// Search terms (`query` parameter).
    pub query: String,
    /// Comma-separated topic ID(s) or slug(s) (`topics` parameter).
    pub topics: String,
    /// Comma-separated public collection ID(s) (`collections` parameter).
    pub collections: String,
    /// Limit selection to a specific Unsplash photographer (`username` parameter).
    pub username: String,
    /// Filter by photo orientation (`orientation` parameter).
    pub orientation: Orientation,
    /// Content safety level (`content_filter` parameter).
    pub content_filter: ContentFilter,
    /// Limit selection to featured photos (`featured` parameter).
    pub featured: bool,
    /// Image quality / resolution to download.
    pub resolution: ImageResolution,
    /// Wallpaper scaling mode in `cosmic-bg`.
    pub scaling_mode: WallpaperScalingMode,
    /// Auto-refresh interval in minutes (`0` = manual only).
    pub auto_refresh_minutes: u32,
    /// Metadata for the last applied wallpaper.
    pub last_photo: Option<LastPhotoInfo>,
}

impl Default for UnsplashSettings {
    fn default() -> Self {
        Self {
            access_key: String::new(),
            query: String::new(),
            topics: String::new(),
            collections: String::new(),
            username: String::new(),
            orientation: Orientation::Landscape,
            content_filter: ContentFilter::Low,
            featured: false,
            resolution: ImageResolution::Full,
            scaling_mode: WallpaperScalingMode::Zoom,
            auto_refresh_minutes: 0,
            last_photo: None,
        }
    }
}

impl UnsplashSettings {
    pub const REFRESH_PRESETS: [(u32, &'static str); 6] = [
        (0, "Off"),
        (15, "15m"),
        (30, "30m"),
        (60, "1h"),
        (360, "6h"),
        (1440, "24h"),
    ];

    /// Returns the trimmed Access Key, borrowing from `self.access_key` when set or
    /// falling back to `UNSPLASH_ACCESS_KEY` from the process environment.
    pub fn effective_access_key(&self) -> Option<Cow<'_, str>> {
        let trimmed = self.access_key.trim();
        if !trimmed.is_empty() {
            return Some(Cow::Borrowed(trimmed));
        }
        let env_val = env::var("UNSPLASH_ACCESS_KEY").ok()?;
        let env_trimmed = env_val.trim();
        if env_trimmed.is_empty() {
            None
        } else {
            Some(Cow::Owned(env_trimmed.to_owned()))
        }
    }
}

/// Manages loading and saving `UnsplashSettings` via `cosmic-config` in `~/.config/cosmic/<APP_ID>/v1/settings`.
#[derive(Clone, Debug)]
pub struct SettingsStore {
    config: Option<Config>,
    fallback_path: PathBuf,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsStore {
    pub fn new() -> Self {
        let root = host_config_dir();
        let fallback_path = root
            .join("cosmic")
            .join(APP_ID)
            .join(format!("v{CONFIG_VERSION}"))
            .join(SETTINGS_KEY);
        let config = Config::new(APP_ID, CONFIG_VERSION).ok();
        Self {
            config,
            fallback_path,
        }
    }

    pub fn load(&self) -> UnsplashSettings {
        if let Some(config) = &self.config {
            if let Ok(settings) = config.get_local::<UnsplashSettings>(SETTINGS_KEY) {
                return settings;
            }
        }
        if let Ok(contents) = fs::read_to_string(&self.fallback_path) {
            if let Ok(settings) = ron::from_str::<UnsplashSettings>(&contents) {
                return settings;
            }
        }
        UnsplashSettings::default()
    }

    /// Persists `settings` to `cosmic-config` (or the RON fallback file).
    ///
    /// # Errors
    /// Returns [`AppError`] if the fallback directory cannot be created or the RON file cannot be written.
    pub fn save(&self, settings: &UnsplashSettings) -> Result<(), AppError> {
        if let Some(config) = &self.config {
            if config.set(SETTINGS_KEY, settings).is_ok() {
                return Ok(());
            }
        }
        if let Some(parent) = self.fallback_path.parent() {
            fs::create_dir_all(parent).map_err(AppError::ConfigIo)?;
        }
        let pretty = ron::ser::PrettyConfig::new();
        let serialized = ron::ser::to_string_pretty(settings, pretty)?;
        fs::write(&self.fallback_path, serialized).map_err(AppError::ConfigIo)
    }
}
