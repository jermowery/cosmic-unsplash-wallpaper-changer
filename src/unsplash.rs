use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use serde::Deserialize;
use url::Url;

use crate::config::{ImageResolution, LastPhotoInfo, UnsplashSettings};
use crate::error::AppError;
use crate::wallpaper::{prune_old_wallpapers, set_cosmic_wallpaper, wallpaper_storage_dir};

const UNSPLASH_RANDOM_ENDPOINT: &str = "https://api.unsplash.com/photos/random";
const USER_AGENT_VALUE: &str = concat!(
    "cosmic-ext-applet-random-wallpaper/",
    env!("CARGO_PKG_VERSION")
);
const MAX_CACHED_WALLPAPERS: usize = 10;

#[derive(Debug, Deserialize)]
pub struct UnsplashPhoto {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub description: Option<String>,
    pub alt_description: Option<String>,
    pub urls: PhotoUrls,
    pub links: PhotoLinks,
    pub user: PhotoUser,
}

#[derive(Debug, Deserialize)]
pub struct PhotoUrls {
    pub raw: String,
    pub full: String,
    pub regular: String,
}

#[derive(Debug, Deserialize)]
pub struct PhotoLinks {
    pub download_location: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PhotoUser {
    pub name: String,
    pub username: String,
}

#[derive(Debug, Deserialize)]
struct UnsplashErrorResponse {
    errors: Option<Vec<String>>,
}

/// Builds the Unsplash `/photos/random` request URL with all configured parameters.
///
/// # Errors
/// Returns [`AppError::InvalidEndpointUrl`] if the base endpoint URL cannot be parsed.
pub fn build_random_photo_url(settings: &UnsplashSettings) -> Result<Url, AppError> {
    let mut url = Url::parse(UNSPLASH_RANDOM_ENDPOINT)?;

    {
        let mut pairs = url.query_pairs_mut();

        let query = settings.query.trim();
        if !query.is_empty() {
            pairs.append_pair("query", query);
        }

        let topics = settings.topics.trim();
        if !topics.is_empty() {
            pairs.append_pair("topics", topics);
        }

        let collections = settings.collections.trim();
        if !collections.is_empty() {
            pairs.append_pair("collections", collections);
        }

        let username = settings.username.trim().trim_start_matches('@');
        if !username.is_empty() {
            pairs.append_pair("username", username);
        }

        if let Some(orientation) = settings.orientation.api_param() {
            pairs.append_pair("orientation", orientation);
        }

        pairs.append_pair("content_filter", settings.content_filter.api_param());

        if settings.featured {
            pairs.append_pair("featured", "true");
        }
    }

    Ok(url)
}

/// Selects the image download URL for the requested [`ImageResolution`].
pub fn select_download_url(photo: &UnsplashPhoto, resolution: ImageResolution) -> String {
    match resolution {
        ImageResolution::Full => photo.urls.full.clone(),
        ImageResolution::Regular1080p => photo.urls.regular.clone(),
        ImageResolution::Raw => photo.urls.raw.clone(),
        ImageResolution::Uhd4K => append_imgix_params(&photo.urls.raw, 3840, 2160, 90),
        ImageResolution::Qhd1440p => append_imgix_params(&photo.urls.raw, 2560, 1440, 85),
    }
}

fn append_imgix_params(raw_url: &str, width: u32, height: u32, quality: u8) -> String {
    let sep = if raw_url.contains('?') { '&' } else { '?' };
    format!("{raw_url}{sep}w={width}&h={height}&fit=crop&q={quality}&fm=jpg")
}

/// Returns a shared [`reqwest::Client`] so TLS sessions and connection pools are reused
/// across wallpaper fetches without leaking API `Authorization` headers to CDN hosts.
fn shared_http_client() -> Result<&'static reqwest::Client, AppError> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }

    let built = reqwest::Client::builder()
        .user_agent(USER_AGENT_VALUE)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(AppError::HttpClientInit)?;

    let _ = CLIENT.set(built);
    CLIENT.get().ok_or_else(|| {
        AppError::WallpaperCacheIo(std::io::Error::other("HTTP client initialization race"))
    })
}

fn api_headers(access_key: &str) -> Result<HeaderMap, AppError> {
    let mut headers = HeaderMap::with_capacity(3);
    let auth_val = HeaderValue::from_str(&format!("Client-ID {access_key}"))
        .map_err(|_| AppError::InvalidAccessKeyHeader)?;
    headers.insert(AUTHORIZATION, auth_val);
    headers.insert("Accept-Version", HeaderValue::from_static("v1"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    Ok(headers)
}

async fn request_random_photo(
    client: &reqwest::Client,
    url: Url,
    headers: HeaderMap,
) -> Result<UnsplashPhoto, AppError> {
    let response = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(AppError::ApiRequest)?;

    let status = response.status();
    let body_text = response.text().await.map_err(AppError::ApiRequest)?;

    if !status.is_success() {
        let details = serde_json::from_str::<UnsplashErrorResponse>(&body_text)
            .ok()
            .and_then(|resp| resp.errors)
            .filter(|errs| !errs.is_empty())
            .map_or(body_text, |errs| errs.join("; "));
        return Err(AppError::ApiStatus { status, details });
    }

    serde_json::from_str(&body_text).map_err(AppError::InvalidMetadata)
}

/// Fetches a random photo from Unsplash according to `settings`, downloads the image,
/// and applies it as the COSMIC desktop wallpaper.
///
/// # Errors
/// Returns [`AppError`] if the API key is missing, the HTTP request fails,
/// the image cannot be saved to disk, or `cosmic-bg` configuration cannot be updated.
pub async fn fetch_and_apply_wallpaper(
    settings: &UnsplashSettings,
) -> Result<LastPhotoInfo, AppError> {
    let access_key = settings
        .effective_access_key()
        .ok_or(AppError::MissingAccessKey)?;

    let url = build_random_photo_url(settings)?;
    let client = shared_http_client()?;
    let headers = api_headers(&access_key)?;
    let photo = request_random_photo(client, url, headers.clone()).await?;

    // Per Unsplash API guidelines, trigger the download_location endpoint when a photo is used.
    if let Some(download_loc) = &photo.links.download_location {
        let _ = client.get(download_loc).headers(headers).send().await;
    }

    // Download the image from the CDN without forwarding our API Authorization header.
    let download_url = select_download_url(&photo, settings.resolution);
    let image_resp = client
        .get(&download_url)
        .send()
        .await
        .map_err(AppError::ImageDownload)?;

    if !image_resp.status().is_success() {
        return Err(AppError::ImageDownloadStatus(image_resp.status()));
    }

    let image_bytes = image_resp.bytes().await.map_err(AppError::ImageDownload)?;
    if image_bytes.is_empty() {
        return Err(AppError::EmptyImage);
    }

    let storage_dir = wallpaper_storage_dir();
    tokio::fs::create_dir_all(&storage_dir)
        .await
        .map_err(AppError::WallpaperCacheIo)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let safe_id: String = photo
        .id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let file_path = storage_dir.join(format!("unsplash-{safe_id}-{timestamp}.jpg"));

    tokio::fs::write(&file_path, &image_bytes)
        .await
        .map_err(AppError::WallpaperCacheIo)?;

    // Offload synchronous atomic file renames and directory pruning to the blocking pool
    // so we never block the UI async executor thread.
    let scaling_mode = settings.scaling_mode;
    let apply_path = file_path.clone();
    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        set_cosmic_wallpaper(&apply_path, scaling_mode)?;
        prune_old_wallpapers(&storage_dir, MAX_CACHED_WALLPAPERS);
        Ok(())
    })
    .await??;

    let description = photo
        .description
        .or(photo.alt_description)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());

    Ok(LastPhotoInfo {
        id: photo.id,
        photographer: photo.user.name,
        username: photo.user.username,
        description,
        width: photo.width,
        height: photo.height,
        file_path,
    })
}
