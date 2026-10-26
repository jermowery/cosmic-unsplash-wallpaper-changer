use crate::config::{ImageResolution, LastPhotoInfo, UnsplashSettings};
use crate::wallpaper::{prune_old_wallpapers, set_cosmic_wallpaper, wallpaper_storage_dir};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

const UNSPLASH_RANDOM_ENDPOINT: &str = "https://api.unsplash.com/photos/random";

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
    pub small: String,
}

#[derive(Debug, Deserialize)]
pub struct PhotoLinks {
    pub html: String,
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
/// Returns an error string if the base endpoint URL cannot be parsed.
pub fn build_random_photo_url(settings: &UnsplashSettings) -> Result<Url, String> {
    let mut url =
        Url::parse(UNSPLASH_RANDOM_ENDPOINT).map_err(|e| format!("Invalid endpoint URL: {e}"))?;

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

/// Selects the image download URL for the requested `ImageResolution`.
pub fn select_download_url(photo: &UnsplashPhoto, resolution: ImageResolution) -> String {
    match resolution {
        ImageResolution::Full => photo.urls.full.clone(),
        ImageResolution::Regular1080p => photo.urls.regular.clone(),
        ImageResolution::Raw => photo.urls.raw.clone(),
        ImageResolution::Uhd4K => {
            let sep = if photo.urls.raw.contains('?') {
                '&'
            } else {
                '?'
            };
            format!(
                "{}{}w=3840&h=2160&fit=crop&q=90&fm=jpg",
                photo.urls.raw, sep
            )
        }
        ImageResolution::Qhd1440p => {
            let sep = if photo.urls.raw.contains('?') {
                '&'
            } else {
                '?'
            };
            format!(
                "{}{}w=2560&h=1440&fit=crop&q=85&fm=jpg",
                photo.urls.raw, sep
            )
        }
    }
}

fn build_http_client(access_key: &str) -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    let auth_val = HeaderValue::from_str(&format!("Client-ID {access_key}"))
        .map_err(|_| "Invalid characters in Unsplash Access Key".to_string())?;
    headers.insert(AUTHORIZATION, auth_val);
    headers.insert("Accept-Version", HeaderValue::from_static("v1"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("cosmic-applet-unsplash/0.1.0"),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to initialize HTTP client: {e}"))
}

async fn request_random_photo(client: &reqwest::Client, url: Url) -> Result<UnsplashPhoto, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network error contacting Unsplash API: {e}"))?;

    let status = response.status();
    let body_text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Unsplash API response: {e}"))?;

    if !status.is_success() {
        if let Ok(err_resp) = serde_json::from_str::<UnsplashErrorResponse>(&body_text) {
            if let Some(errors) = err_resp.errors {
                if !errors.is_empty() {
                    return Err(format!(
                        "Unsplash API error ({status}): {}",
                        errors.join("; ")
                    ));
                }
            }
        }
        return Err(format!("Unsplash API returned HTTP {status}: {body_text}"));
    }

    serde_json::from_str(&body_text)
        .map_err(|e| format!("Failed to parse Unsplash photo metadata: {e}"))
}

/// Fetches a random photo from Unsplash according to `settings`, downloads the image,
/// and applies it as the COSMIC desktop wallpaper.
///
/// # Errors
/// Returns an error string if the API key is missing, the HTTP request fails,
/// the image cannot be saved to disk, or `cosmic-bg` configuration cannot be updated.
pub async fn fetch_and_apply_wallpaper(
    settings: UnsplashSettings,
) -> Result<LastPhotoInfo, String> {
    let access_key = settings.effective_access_key();
    if access_key.is_empty() {
        return Err("Please enter your Unsplash Access Key in the settings above.".to_string());
    }

    let url = build_random_photo_url(&settings)?;
    let client = build_http_client(&access_key)?;
    let photo = request_random_photo(&client, url).await?;

    // Per Unsplash API guidelines, trigger the download_location endpoint when a photo is used.
    if let Some(download_loc) = &photo.links.download_location {
        let _ = client.get(download_loc).send().await;
    }

    let download_url = select_download_url(&photo, settings.resolution);
    let image_resp = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download wallpaper image: {e}"))?;

    if !image_resp.status().is_success() {
        return Err(format!(
            "Image download failed with HTTP {}",
            image_resp.status()
        ));
    }

    let image_bytes = image_resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read image bytes: {e}"))?;

    if image_bytes.is_empty() {
        return Err("Downloaded image was empty".to_string());
    }

    let storage_dir = wallpaper_storage_dir();
    tokio::fs::create_dir_all(&storage_dir)
        .await
        .map_err(|e| format!("Failed to create wallpaper cache directory: {e}"))?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let safe_id: String = photo
        .id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let file_name = format!("unsplash-{safe_id}-{timestamp}.jpg");
    let file_path = storage_dir.join(file_name);

    tokio::fs::write(&file_path, &image_bytes)
        .await
        .map_err(|e| format!("Failed to save wallpaper image to disk: {e}"))?;

    // Apply the downloaded file to cosmic-bg
    set_cosmic_wallpaper(&file_path, settings.scaling_mode)?;

    // Keep the cache folder tidy (retain the 10 most recent wallpapers)
    prune_old_wallpapers(&storage_dir, 10);

    let description = photo
        .description
        .or(photo.alt_description)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(LastPhotoInfo {
        id: photo.id,
        photographer: photo.user.name,
        username: photo.user.username,
        description,
        width: photo.width,
        height: photo.height,
        file_path: file_path.to_string_lossy().to_string(),
    })
}
