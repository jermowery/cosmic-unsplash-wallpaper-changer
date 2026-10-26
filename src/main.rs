pub mod config;
pub mod unsplash;
pub mod wallpaper;
pub mod window;

use crate::config::{ContentFilter, Orientation, SettingsStore};
use crate::unsplash::fetch_and_apply_wallpaper;
use crate::window::UnsplashApplet;

fn print_help() {
    println!(
        "cosmic-applet-unsplash — COSMIC Desktop Panel Applet for Unsplash Wallpapers

USAGE:
    cosmic-applet-unsplash [OPTIONS]

OPTIONS:
    (no flags)                  Run as a COSMIC panel applet
    -w, --window                Open the settings dialog in a standalone desktop window
    --fetch-now                 Fetch a random Unsplash photo and set the COSMIC wallpaper immediately
    --save                      Persist any CLI-supplied settings to cosmic-config
    --access-key <KEY>          Unsplash API Access Key (Client-ID)
    --query <TERMS>             Search terms (e.g. \"mountains, nature\")
    --topics <TOPICS>           Comma-separated Unsplash topic slugs/IDs
    --collections <IDS>         Comma-separated Unsplash collection IDs
    --username <USER>           Filter by Unsplash photographer username
    --orientation <ORIENT>      Orientation: any, landscape, portrait, squarish
    --content-filter <LEVEL>    Safety filter: low, high
    --featured                  Restrict to featured Unsplash photos
    -h, --help                  Show this help message"
    );
}

fn main() -> cosmic::iced::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return Ok(());
    }

    let store = SettingsStore::new();
    let mut settings = store.load();
    let mut fetch_now = false;
    let mut standalone_window = false;
    let mut save_flag = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fetch-now" => fetch_now = true,
            "-w" | "--window" => standalone_window = true,
            "--save" => save_flag = true,
            "--featured" => settings.featured = true,
            "--access-key" if i + 1 < args.len() => {
                i += 1;
                settings.access_key.clone_from(&args[i]);
            }
            "--query" if i + 1 < args.len() => {
                i += 1;
                settings.query.clone_from(&args[i]);
            }
            "--topics" if i + 1 < args.len() => {
                i += 1;
                settings.topics.clone_from(&args[i]);
            }
            "--collections" if i + 1 < args.len() => {
                i += 1;
                settings.collections.clone_from(&args[i]);
            }
            "--username" if i + 1 < args.len() => {
                i += 1;
                settings.username.clone_from(&args[i]);
            }
            "--orientation" if i + 1 < args.len() => {
                i += 1;
                settings.orientation = match args[i].to_lowercase().as_str() {
                    "any" => Orientation::Any,
                    "portrait" => Orientation::Portrait,
                    "squarish" => Orientation::Squarish,
                    _ => Orientation::Landscape,
                };
            }
            "--content-filter" if i + 1 < args.len() => {
                i += 1;
                settings.content_filter = match args[i].to_lowercase().as_str() {
                    "high" => ContentFilter::High,
                    _ => ContentFilter::Low,
                };
            }
            _ => {}
        }
        i += 1;
    }

    if save_flag {
        if let Err(err) = store.save(&settings) {
            eprintln!("Warning: could not save settings: {err}");
        } else if !fetch_now && !standalone_window {
            println!("Settings saved.");
            return Ok(());
        }
    }

    if fetch_now {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        match rt.block_on(fetch_and_apply_wallpaper(settings.clone())) {
            Ok(info) => {
                println!(
                    "Successfully updated COSMIC wallpaper!\n  Photo ID:     {}\n  Photographer: {} (@{})\n  Resolution:   {}x{}\n  Saved Path:   {}",
                    info.id, info.photographer, info.username, info.width, info.height, info.file_path
                );
                settings.last_photo = Some(info);
                let _ = store.save(&settings);
                return Ok(());
            }
            Err(err) => {
                eprintln!("Error fetching wallpaper: {err}");
                std::process::exit(1);
            }
        }
    }

    if standalone_window {
        let win_settings =
            cosmic::app::Settings::default().size(cosmic::iced::Size::new(460.0, 720.0));
        return cosmic::app::run::<UnsplashApplet>(win_settings, true);
    }

    cosmic::applet::run::<UnsplashApplet>(false)
}

#[cfg(test)]
mod tests {
    use crate::config::{
        ContentFilter, ImageResolution, Orientation, UnsplashSettings, WallpaperScalingMode,
    };
    use crate::unsplash::{
        build_random_photo_url, select_download_url, PhotoLinks, PhotoUrls, PhotoUser,
        UnsplashPhoto,
    };
    use crate::wallpaper::format_cosmic_bg_entry_ron;
    use std::path::Path;

    #[test]
    fn test_build_random_photo_url_with_all_options() {
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
    fn test_select_download_url_resolutions() {
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
                small: "https://images.unsplash.com/photo-abc?ixid=123&w=400".into(),
            },
            links: PhotoLinks {
                html: "https://unsplash.com/photos/abc".into(),
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
    fn test_format_cosmic_bg_entry_ron() {
        let ron_str = format_cosmic_bg_entry_ron(
            "all",
            Path::new("/home/user/.config/cosmic/com.system76.CosmicBackground/unsplash/test.jpg"),
            WallpaperScalingMode::Zoom,
        );
        assert!(ron_str.contains("output: \"all\""));
        assert!(ron_str.contains("source: Path(\"/home/user/.config/cosmic/com.system76.CosmicBackground/unsplash/test.jpg\")"));
        assert!(ron_str.contains("scaling_mode: Zoom"));
    }
}
