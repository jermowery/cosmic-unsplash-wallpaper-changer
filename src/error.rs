use std::path::PathBuf;
use thiserror::Error;

/// Unified domain error type for `cosmic-applet-unsplash`.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Please enter your Unsplash Access Key in the settings above.")]
    MissingAccessKey,

    #[error("Invalid characters in Unsplash Access Key")]
    InvalidAccessKeyHeader,

    #[error("Invalid Unsplash endpoint URL: {0}")]
    InvalidEndpointUrl(#[from] url::ParseError),

    #[error("Failed to initialize HTTP client: {0}")]
    HttpClientInit(#[source] reqwest::Error),

    #[error("Network error contacting Unsplash API: {0}")]
    ApiRequest(#[source] reqwest::Error),

    #[error("Unsplash API error ({status}): {details}")]
    ApiStatus {
        status: reqwest::StatusCode,
        details: String,
    },

    #[error("Failed to parse Unsplash photo metadata: {0}")]
    InvalidMetadata(#[source] serde_json::Error),

    #[error("Failed to download wallpaper image: {0}")]
    ImageDownload(#[source] reqwest::Error),

    #[error("Image download failed with HTTP {0}")]
    ImageDownloadStatus(reqwest::StatusCode),

    #[error("Downloaded wallpaper image was empty")]
    EmptyImage,

    #[error("Wallpaper cache I/O error: {0}")]
    WallpaperCacheIo(#[source] std::io::Error),

    #[error("Wallpaper file does not exist: {}", .0.display())]
    WallpaperPathMissing(PathBuf),

    #[error("Failed to update cosmic-bg configuration: {0}")]
    CosmicBgConfigIo(#[source] std::io::Error),

    #[error("Background task failed: {0}")]
    BackgroundTaskJoin(#[from] tokio::task::JoinError),

    #[error("Failed to serialize settings to RON: {0}")]
    ConfigSerialize(#[from] ron::Error),

    #[error("Failed to write settings file: {0}")]
    ConfigIo(#[source] std::io::Error),

    #[error("{0}")]
    CliUsage(String),
}
