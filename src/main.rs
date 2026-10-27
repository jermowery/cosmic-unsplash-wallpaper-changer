// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod config;
mod error;
mod i18n;
mod unsplash;
mod wallpaper;

use std::process::ExitCode;

use crate::app::AppModel;
use crate::config::{Config, ContentFilter, Orientation, SettingsStore};
use crate::error::AppError;
use crate::unsplash::fetch_and_apply_wallpaper;

const HELP_TEXT: &str = "\
cosmic-ext-applet-random-wallpaper — Random Wallpaper Applet for the COSMIC™ Desktop

USAGE:
    cosmic-ext-applet-random-wallpaper [OPTIONS]

OPTIONS:
    (no flags)                  Run as a panel applet for the COSMIC™ desktop
    -w, --window                Open the settings dialog in a standalone desktop window
    --fetch-now                 Fetch a random photo and set the desktop wallpaper immediately
    --save                      Persist any CLI-supplied settings to cosmic-config
    --access-key <KEY>          Unsplash API Access Key (Client-ID)
    --query <TERMS>             Search terms (e.g. \"mountains, nature\")
    --topics <TOPICS>           Comma-separated topic slugs/IDs
    --collections <IDS>         Comma-separated collection IDs
    --username <USER>           Filter by photographer username
    --orientation <ORIENT>      Orientation: any, landscape, portrait, squarish
    --content-filter <LEVEL>    Safety filter: low, high
    --featured                  Restrict to featured photos
    -h, --help                  Show this help message";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ExecutionMode {
    #[default]
    PanelApplet,
    StandaloneWindow,
    FetchNow,
    ShowHelp,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct CliOptions {
    mode: ExecutionMode,
    save: bool,
}

fn next_flag_value<I>(args: &mut I, flag: &str) -> Result<String, AppError>
where
    I: Iterator<Item = String>,
{
    args.next()
        .ok_or_else(|| AppError::CliUsage(format!("Missing value for '{flag}'")))
}

fn apply_cli_args<I>(args: I, settings: &mut Config) -> Result<CliOptions, AppError>
where
    I: IntoIterator<Item = String>,
{
    let mut opts = CliOptions::default();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                opts.mode = ExecutionMode::ShowHelp;
                return Ok(opts);
            }
            "--fetch-now" => opts.mode = ExecutionMode::FetchNow,
            "-w" | "--window" => opts.mode = ExecutionMode::StandaloneWindow,
            "--save" => opts.save = true,
            "--featured" => settings.featured = true,
            "--access-key" => {
                settings.access_key = next_flag_value(&mut iter, "--access-key")?;
            }
            "--query" => {
                settings.query = next_flag_value(&mut iter, "--query")?;
            }
            "--topics" => {
                settings.topics = next_flag_value(&mut iter, "--topics")?;
            }
            "--collections" => {
                settings.collections = next_flag_value(&mut iter, "--collections")?;
            }
            "--username" => {
                settings.username = next_flag_value(&mut iter, "--username")?;
            }
            "--orientation" => {
                let raw = next_flag_value(&mut iter, "--orientation")?;
                settings.orientation = raw.parse::<Orientation>()?;
            }
            "--content-filter" => {
                let raw = next_flag_value(&mut iter, "--content-filter")?;
                settings.content_filter = raw.parse::<ContentFilter>()?;
            }
            other => {
                return Err(AppError::CliUsage(format!(
                    "Unknown option '{other}'. Run with --help for usage."
                )));
            }
        }
    }

    Ok(opts)
}

fn run_cli_fetch(store: &SettingsStore, mut settings: Config) -> Result<(), AppError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(AppError::WallpaperCacheIo)?;

    let info = rt.block_on(fetch_and_apply_wallpaper(&settings))?;
    println!(
        "Successfully updated COSMIC wallpaper!\n  Photo ID:     {}\n  Photographer: {} (@{})\n  Resolution:   {}x{}\n  Saved Path:   {}",
        info.id,
        info.photographer,
        info.username,
        info.width,
        info.height,
        info.file_path.display()
    );
    settings.last_photo = Some(info);
    let _ = store.save(&settings);
    Ok(())
}

fn main() -> ExitCode {
    // Get the system's preferred languages and enable localizations.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);

    let store = SettingsStore::new();
    let mut settings = store.load();

    let opts = match apply_cli_args(std::env::args().skip(1), &mut settings) {
        Ok(opts) => opts,
        Err(err) => {
            eprintln!("Error: {err}");
            return ExitCode::from(2);
        }
    };

    if opts.mode == ExecutionMode::ShowHelp {
        println!("{HELP_TEXT}");
        return ExitCode::SUCCESS;
    }

    if opts.save {
        if let Err(err) = store.save(&settings) {
            eprintln!("Warning: could not save settings: {err}");
        } else if opts.mode == ExecutionMode::PanelApplet {
            println!("Settings saved.");
            return ExitCode::SUCCESS;
        }
    }

    match opts.mode {
        ExecutionMode::ShowHelp => ExitCode::SUCCESS,
        ExecutionMode::FetchNow => {
            if let Err(err) = run_cli_fetch(&store, settings) {
                eprintln!("Error fetching wallpaper: {err}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        ExecutionMode::StandaloneWindow => {
            let win_settings =
                cosmic::app::Settings::default().size(cosmic::iced::Size::new(460.0, 720.0));
            if let Err(err) = cosmic::app::run::<AppModel>(win_settings, true) {
                eprintln!("Application error: {err}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        ExecutionMode::PanelApplet => {
            if let Err(err) = cosmic::applet::run::<AppModel>(false) {
                eprintln!("Application error: {err}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::path::Path;

    use crate::config::{
        ContentFilter, ImageResolution, Orientation, UnsplashSettings, WallpaperScalingMode,
    };
    use crate::unsplash::{
        build_random_photo_url, select_download_url, PhotoLinks, PhotoUrls, PhotoUser,
        UnsplashPhoto,
    };
    use crate::wallpaper::format_cosmic_bg_entry_ron;

    use super::{apply_cli_args, ExecutionMode};

    #[test]
    fn build_random_photo_url_includes_configured_query_parameters() {
        let settings = UnsplashSettings {
            access_key: "test_key".into(),
            query: "mountains, nature".into(),
            topics: "wallpapers,nature".into(),
            collections: "12345".into(),
            username: "@johndoe".into(),
            orientation: Orientation::Landscape,
            content_filter: ContentFilter::High,
            featured: true,
            ..Default::default()
        };

        let url = build_random_photo_url(&settings).expect("valid url");
        let query_str = url.query().unwrap_or_default();
        assert!(query_str.contains("query=mountains"));
        assert!(query_str.contains("topics=wallpapers"));
        assert!(query_str.contains("collections=12345"));
        assert!(query_str.contains("username=johndoe"));
        assert!(query_str.contains("orientation=landscape"));
        assert!(query_str.contains("content_filter=high"));
        assert!(query_str.contains("featured=true"));
    }

    #[test]
    fn select_download_url_formats_imgix_parameters_for_target_resolutions() {
        let photo = UnsplashPhoto {
            id: "abc".into(),
            width: 4000,
            height: 3000,
            description: None,
            alt_description: None,
            urls: PhotoUrls {
                raw: "https://images.unsplash.com/photo-abc?ixid=123".into(),
                full: "https://images.unsplash.com/photo-abc?ixid=123&q=85".into(),
                regular: "https://images.unsplash.com/photo-abc?ixid=123&w=1080".into(),
            },
            links: PhotoLinks {
                download_location: None,
            },
            user: PhotoUser {
                name: "Jane".into(),
                username: "jane".into(),
            },
        };

        assert_eq!(
            select_download_url(&photo, ImageResolution::Full),
            "https://images.unsplash.com/photo-abc?ixid=123&q=85"
        );
        assert_eq!(
            select_download_url(&photo, ImageResolution::Uhd4K),
            "https://images.unsplash.com/photo-abc?ixid=123&w=3840&h=2160&fit=crop&q=90&fm=jpg"
        );
        assert_eq!(
            select_download_url(&photo, ImageResolution::Qhd1440p),
            "https://images.unsplash.com/photo-abc?ixid=123&w=2560&h=1440&fit=crop&q=85&fm=jpg"
        );
    }

    #[test]
    fn format_cosmic_bg_entry_ron_produces_valid_cosmic_bg_state() {
        let ron_str = format_cosmic_bg_entry_ron(
            "all",
            Path::new("/home/user/.config/cosmic/com.system76.CosmicBackground/unsplash/test.jpg"),
            WallpaperScalingMode::Zoom,
        );
        assert!(ron_str.contains("output: \"all\""));
        assert!(ron_str.contains(
            "source: Path(\"/home/user/.config/cosmic/com.system76.CosmicBackground/unsplash/test.jpg\")"
        ));
        assert!(ron_str.contains("scaling_mode: Zoom"));
    }

    #[test]
    fn apply_cli_args_parses_flags_and_rejects_invalid_inputs() {
        let mut settings = UnsplashSettings::default();
        let args = [
            "--access-key",
            "my_key",
            "--query",
            "fjord",
            "--orientation",
            "portrait",
            "--content-filter",
            "high",
            "--featured",
            "--save",
            "--fetch-now",
        ]
        .into_iter()
        .map(String::from);

        let opts = apply_cli_args(args, &mut settings).expect("valid CLI flags");
        assert!(opts.save);
        assert_eq!(opts.mode, ExecutionMode::FetchNow);
        assert_eq!(settings.access_key, "my_key");
        assert_eq!(settings.query, "fjord");
        assert_eq!(settings.orientation, Orientation::Portrait);
        assert_eq!(settings.content_filter, ContentFilter::High);
        assert!(settings.featured);

        let bad_orient = ["--orientation", "diagonal"].into_iter().map(String::from);
        assert!(apply_cli_args(bad_orient, &mut settings).is_err());

        let unknown_flag = ["--not-a-flag"].into_iter().map(String::from);
        assert!(apply_cli_args(unknown_flag, &mut settings).is_err());
    }

    #[test]
    fn fluent_localizations_load_and_format_expected_strings() {
        crate::i18n::init(&[]);
        assert_eq!(crate::fl!("app-title"), "Random Wallpaper");
        assert!(crate::fl!("app-comment").contains("COSMIC"));
    }
}
