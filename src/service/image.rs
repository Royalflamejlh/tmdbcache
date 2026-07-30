//! On-disk image cache.
//!
//! TMDB artwork is fetched once and written under
//! `<imageCache>/<size>/<filename>`, then served from disk. This is what keeps a
//! browsing session from hammering TMDB, and what makes the library usable when
//! TMDB is unreachable.

use std::path::{Path, PathBuf};

use super::AppState;
use crate::error::{AppError, Result};

/// Served when the caller does not pick a size.
const DEFAULT_SIZE: &str = "original";

/// Extensions TMDB serves artwork in.
const ALLOWED_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "svg"];

/// Validates a TMDB image path and reduces it to a bare filename.
///
/// TMDB paths are always a single segment like `/abc123.jpg`. Anything with a
/// directory separator, a parent reference or an unexpected extension is
/// rejected rather than sanitised, so a crafted `imagePath` cannot escape the
/// cache directory.
fn safe_filename(image_path: &str) -> Result<String> {
    let candidate = image_path.trim().trim_start_matches('/');

    if candidate.is_empty() {
        return Err(AppError::BadRequest("imagePath must not be blank".into()));
    }
    if candidate.contains('/') || candidate.contains('\\') || candidate.contains("..") {
        return Err(AppError::BadRequest(format!(
            "imagePath must be a single filename, got {image_path:?}"
        )));
    }
    if !candidate
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(AppError::BadRequest(format!(
            "imagePath contains unsupported characters: {image_path:?}"
        )));
    }

    let extension = Path::new(candidate)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(AppError::BadRequest(format!(
            "unsupported image extension: {image_path:?}"
        )));
    }

    Ok(candidate.to_string())
}

/// Validates a TMDB size token (`original`, `w500`, `h632`, …).
fn safe_size(size: Option<&str>) -> Result<String> {
    let size = size.map(str::trim).filter(|s| !s.is_empty());
    let Some(size) = size else {
        return Ok(DEFAULT_SIZE.to_string());
    };

    if size == "original" {
        return Ok(size.to_string());
    }
    let valid = matches!(size.as_bytes().first(), Some(b'w' | b'h'))
        && size.len() > 1
        && size[1..].chars().all(|c| c.is_ascii_digit());
    if !valid {
        return Err(AppError::BadRequest(format!(
            "backdropSize must be 'original' or like 'w500', got {size:?}"
        )));
    }
    Ok(size.to_string())
}

fn content_type_for(filename: &str) -> String {
    mime_guess::from_path(filename)
        .first_raw()
        .unwrap_or("image/jpeg")
        .to_string()
}

/// An image ready to be served, with the header value to send alongside it.
pub struct CachedImage {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

/// Returns the requested image, downloading and caching it on a miss.
pub async fn get_image(
    state: &AppState,
    image_path: &str,
    size: Option<&str>,
) -> Result<CachedImage> {
    let filename = safe_filename(image_path)?;
    let size = safe_size(size)?;

    let dir = state.cfg.image_dir().join(&size);
    let path = dir.join(&filename);

    if let Ok(bytes) = tokio::fs::read(&path).await {
        if !bytes.is_empty() {
            return Ok(CachedImage {
                content_type: content_type_for(&filename),
                bytes,
            });
        }
        // A zero-length file is a leftover from an interrupted write; refetch.
        tracing::debug!(?path, "discarding empty cached image");
    }

    let (bytes, content_type) = state.tmdb.download_image(&size, &filename).await?;

    if let Err(err) = write_atomically(&dir, &filename, &bytes).await {
        // A cache write failure must not fail the request.
        tracing::warn!(?path, error = %err, "could not write image to cache");
    }

    Ok(CachedImage {
        bytes,
        content_type,
    })
}

/// Writes to a temporary file then renames, so a concurrent reader never sees a
/// partially written image.
async fn write_atomically(dir: &Path, filename: &str, bytes: &[u8]) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dir).await?;

    let final_path = dir.join(filename);
    let temp_path: PathBuf = dir.join(format!(".{filename}.{}.tmp", std::process::id()));

    tokio::fs::write(&temp_path, bytes).await?;
    match tokio::fs::rename(&temp_path, &final_path).await {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_tmdb_paths() {
        assert_eq!(safe_filename("/abc123.jpg").unwrap(), "abc123.jpg");
        assert_eq!(safe_filename("abc123.jpg").unwrap(), "abc123.jpg");
        assert_eq!(safe_filename("/a-b_c.PNG").unwrap(), "a-b_c.PNG");
    }

    #[test]
    fn rejects_traversal_and_nesting() {
        for path in [
            "../../etc/passwd",
            "/../secret.jpg",
            "nested/dir.jpg",
            "..\\windows.jpg",
            "",
            "   ",
        ] {
            assert!(
                safe_filename(path).is_err(),
                "{path:?} should have been rejected"
            );
        }
    }

    #[test]
    fn rejects_unexpected_extensions() {
        assert!(safe_filename("payload.sh").is_err());
        assert!(safe_filename("noextension").is_err());
    }

    #[test]
    fn size_tokens_are_validated() {
        assert_eq!(safe_size(None).unwrap(), "original");
        assert_eq!(safe_size(Some("")).unwrap(), "original");
        assert_eq!(safe_size(Some("w500")).unwrap(), "w500");
        assert_eq!(safe_size(Some("h632")).unwrap(), "h632");

        assert!(safe_size(Some("../..")).is_err());
        assert!(safe_size(Some("w")).is_err());
        assert!(safe_size(Some("large")).is_err());
        assert!(safe_size(Some("w500/../..")).is_err());
    }
}
