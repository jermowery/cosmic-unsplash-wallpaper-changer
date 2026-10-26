use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::config::{host_config_dir, WallpaperScalingMode};
use crate::error::AppError;

pub const COSMIC_BG_ID: &str = "com.system76.CosmicBackground";

/// Directory where downloaded Unsplash wallpaper images are stored.
/// Placed under `~/.config/cosmic/com.system76.CosmicBackground/unsplash` so that
/// both host builds and Flatpak builds (with `--filesystem=xdg-config/cosmic:rw`)
/// share the exact same host filesystem path with `cosmic-bg`.
pub fn wallpaper_storage_dir() -> PathBuf {
    host_config_dir()
        .join("cosmic")
        .join(COSMIC_BG_ID)
        .join("unsplash")
}

/// Directory containing `cosmic-bg` v1 configuration files.
pub fn cosmic_bg_config_dir() -> PathBuf {
    host_config_dir()
        .join("cosmic")
        .join(COSMIC_BG_ID)
        .join("v1")
}

/// Formats the RON configuration entry expected by `cosmic-bg`.
pub fn format_cosmic_bg_entry_ron(
    output: &str,
    image_path: &Path,
    scaling_mode: WallpaperScalingMode,
) -> String {
    let escaped_path = image_path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!(
        "(\n    output: \"{output}\",\n    source: Path(\"{escaped_path}\"),\n    filter_by_theme: false,\n    rotation_frequency: 300,\n    filter_method: Lanczos,\n    scaling_mode: {scaling},\n    sampling_method: Alphanumeric,\n)\n",
        scaling = scaling_mode.cosmic_bg_ron()
    )
}

/// Atomically writes `contents` to `target` via a temporary file in the same directory
/// so `cosmic-bg`'s `inotify` watcher observes a single atomic rename event.
fn write_atomic(target: &Path, contents: &[u8]) -> Result<(), AppError> {
    let parent = target.parent().ok_or_else(|| {
        AppError::CosmicBgConfigIo(io::Error::new(
            io::ErrorKind::InvalidInput,
            "target path has no parent directory",
        ))
    })?;
    fs::create_dir_all(parent).map_err(AppError::CosmicBgConfigIo)?;

    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(AppError::CosmicBgConfigIo)?;
    tmp.write_all(contents)
        .map_err(AppError::CosmicBgConfigIo)?;
    tmp.flush().map_err(AppError::CosmicBgConfigIo)?;
    tmp.persist(target)
        .map_err(|e| AppError::CosmicBgConfigIo(e.error))?;
    Ok(())
}

/// Applies the given image path as the active COSMIC desktop wallpaper.
///
/// # Errors
/// Returns [`AppError::WallpaperPathMissing`] if `image_path` does not exist or
/// [`AppError::CosmicBgConfigIo`] if the `cosmic-bg` configuration files cannot be written atomically.
pub fn set_cosmic_wallpaper(
    image_path: &Path,
    scaling_mode: WallpaperScalingMode,
) -> Result<(), AppError> {
    if !image_path.exists() {
        return Err(AppError::WallpaperPathMissing(image_path.to_path_buf()));
    }

    let bg_dir = cosmic_bg_config_dir();
    fs::create_dir_all(&bg_dir).map_err(AppError::CosmicBgConfigIo)?;

    // Ensure `same-on-all` is true so `all` applies across all displays.
    let same_on_all_path = bg_dir.join("same-on-all");
    write_atomic(&same_on_all_path, b"true\n")?;

    // Write the `all` configuration file.
    let all_ron = format_cosmic_bg_entry_ron("all", image_path, scaling_mode);
    let all_path = bg_dir.join("all");
    write_atomic(&all_path, all_ron.as_bytes())?;

    // If per-output configuration files exist in `v1/`, update them as well.
    if let Ok(entries) = fs::read_dir(&bg_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if matches!(name, "all" | "same-on-all" | "backgrounds") || name.starts_with('.') {
                continue;
            }
            let output_ron = format_cosmic_bg_entry_ron(name, image_path, scaling_mode);
            let _ = write_atomic(&path, output_ron.as_bytes());
        }
    }

    Ok(())
}

/// Keeps only the most recent `keep_count` downloaded Unsplash images in the storage directory.
pub fn prune_old_wallpapers(dir: &Path, keep_count: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let name = path.file_name()?.to_str()?;
            let is_jpg = Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("jpg"));
            if !name.starts_with("unsplash-") || !is_jpg {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect();

    if files.len() <= keep_count {
        return;
    }

    // Sort newest first
    files.sort_by_key(|a| std::cmp::Reverse(a.0));
    for (_, old_path) in files.into_iter().skip(keep_count) {
        let _ = fs::remove_file(old_path);
    }
}
