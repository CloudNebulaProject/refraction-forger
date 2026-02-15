use std::path::PathBuf;

use spec_parser::schema::DistroFamily;
use tracing::info;

use crate::error::BuilderError;

/// Resolved forger binary for use inside a builder VM.
pub struct ResolvedBinary {
    pub path: PathBuf,
}

/// Map a distro family to the Rust target triple needed inside the builder VM.
pub fn target_triple(distro: &DistroFamily) -> &'static str {
    match distro {
        DistroFamily::OmniOS => "x86_64-unknown-illumos",
        DistroFamily::Ubuntu => "x86_64-unknown-linux-gnu",
    }
}

/// Detect whether the current executable is a dev build (running from cargo target dir).
pub fn is_dev_build() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.contains("/target/")))
        .unwrap_or(false)
}

/// Find the workspace root by walking up from the current exe looking for a workspace Cargo.toml.
fn find_workspace_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?;

    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            // Check if it's a workspace root (contains [workspace])
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return Some(dir.to_path_buf());
                }
            }
        }
        dir = dir.parent()?;
    }
}

/// Resolve the forger binary path for the given distro.
///
/// In dev mode: looks for cross-compiled binary in the workspace target directory.
/// In release mode: downloads from GitHub releases (cached locally).
pub async fn resolve_forger_binary(distro: &DistroFamily) -> Result<ResolvedBinary, BuilderError> {
    let triple = target_triple(distro);

    if is_dev_build() {
        resolve_dev_binary(triple)
    } else {
        resolve_release_binary(triple).await
    }
}

fn resolve_dev_binary(triple: &str) -> Result<ResolvedBinary, BuilderError> {
    let workspace_root = find_workspace_root().ok_or_else(|| BuilderError::BinaryNotFound {
        target_triple: triple.to_string(),
        path: "<workspace root not found>".to_string(),
    })?;

    let binary_path = workspace_root
        .join("target")
        .join(triple)
        .join("release")
        .join("forger");

    if !binary_path.exists() {
        return Err(BuilderError::BinaryNotFound {
            target_triple: triple.to_string(),
            path: binary_path.display().to_string(),
        });
    }

    info!(path = %binary_path.display(), triple, "Using dev cross-compiled forger binary");
    Ok(ResolvedBinary { path: binary_path })
}

async fn resolve_release_binary(triple: &str) -> Result<ResolvedBinary, BuilderError> {
    let version = env!("CARGO_PKG_VERSION");
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("forger")
        .join("builder-binaries");

    let cached_path = cache_dir.join(format!("forger-{triple}-v{version}"));

    if cached_path.exists() {
        info!(path = %cached_path.display(), "Using cached forger binary");
        return Ok(ResolvedBinary { path: cached_path });
    }

    let url = release_url(version, triple);
    info!(%url, "Downloading forger binary for builder VM");

    let response = reqwest::get(&url).await.map_err(|e| {
        BuilderError::BinaryDownloadFailed {
            url: url.clone(),
            detail: e.to_string(),
        }
    })?;

    if !response.status().is_success() {
        return Err(BuilderError::BinaryDownloadFailed {
            url,
            detail: format!("HTTP {}", response.status()),
        });
    }

    let bytes = response.bytes().await.map_err(|e| {
        BuilderError::BinaryDownloadFailed {
            url: url.clone(),
            detail: format!("reading response body: {e}"),
        }
    })?;

    std::fs::create_dir_all(&cache_dir).map_err(|e| BuilderError::BinaryDownloadFailed {
        url: url.clone(),
        detail: format!("creating cache dir: {e}"),
    })?;

    std::fs::write(&cached_path, &bytes).map_err(|e| BuilderError::BinaryDownloadFailed {
        url: url.clone(),
        detail: format!("writing cached binary: {e}"),
    })?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&cached_path, perms).map_err(|e| {
            BuilderError::BinaryDownloadFailed {
                url,
                detail: format!("chmod: {e}"),
            }
        })?;
    }

    Ok(ResolvedBinary { path: cached_path })
}

fn release_url(version: &str, triple: &str) -> String {
    format!(
        "https://github.com/CloudNebulaProject/refraction-forger/releases/download/v{version}/forger-{triple}"
    )
}

/// Check if a path looks like a cross-compiled forger binary exists for this target.
pub fn dev_binary_path(triple: &str) -> Option<PathBuf> {
    let workspace_root = find_workspace_root()?;
    let path = workspace_root
        .join("target")
        .join(triple)
        .join("release")
        .join("forger");
    path.exists().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_triple_mapping() {
        assert_eq!(target_triple(&DistroFamily::OmniOS), "x86_64-unknown-illumos");
        assert_eq!(target_triple(&DistroFamily::Ubuntu), "x86_64-unknown-linux-gnu");
    }

    #[test]
    fn release_url_construction() {
        let url = release_url("0.1.0", "x86_64-unknown-linux-gnu");
        assert_eq!(
            url,
            "https://github.com/CloudNebulaProject/refraction-forger/releases/download/v0.1.0/forger-x86_64-unknown-linux-gnu"
        );
    }

    #[test]
    fn dev_detection_heuristic() {
        // In test context, the binary is under target/
        let result = is_dev_build();
        // When running under `cargo test`, the binary IS in target/
        assert!(result);
    }
}
