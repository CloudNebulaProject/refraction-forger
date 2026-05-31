use std::path::{Path, PathBuf};

use spec_parser::schema::DistroFamily;
use tracing::info;

use crate::error::BuilderError;

/// Resolved forger binary for use inside a builder VM.
#[derive(Debug)]
pub struct ResolvedBinary {
    pub path: PathBuf,
}

/// A configured source for the builder `forger` binary.
///
/// Parsed from a CLI flag, the `builder.binary` spec block, or the
/// `FORGER_BUILDER_BINARY` env var. The `{triple}` token (if present) is
/// substituted with the builder's Rust target triple at resolve time, before
/// any filesystem or network access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinarySource {
    /// Local file. Copied (not symlinked) into the cache.
    Path(PathBuf),

    /// `http://...` or `https://...`
    Url {
        url: String,
        sha256: Option<String>,
    },

    /// `oci://registry/repo:tag` or `oci://registry/repo@sha256:...`.
    /// Pulls a single-layer OCI artifact; the binary is the layer payload.
    Oci {
        reference: String,
        sha256: Option<String>,
    },
}

impl BinarySource {
    /// Parse a source string into a `BinarySource`.
    ///
    /// `sha256` is the optional content pin (applies to URL/OCI sources only).
    /// `base_dir` is the directory relative paths resolve against — the spec
    /// file's directory for spec-block sources, the current working directory
    /// for CLI/env sources. `{triple}` is preserved verbatim here and only
    /// substituted at resolve time.
    pub fn parse(
        source: &str,
        sha256: Option<&str>,
        base_dir: &Path,
    ) -> Result<Self, BuilderError> {
        let s = source.trim();
        if s.is_empty() {
            return Err(BuilderError::BinarySourceInvalid {
                input: source.to_string(),
                detail: "source is empty".to_string(),
            });
        }

        let sha = sha256.map(normalize_sha256).filter(|s| !s.is_empty());

        if let Some(reference) = s.strip_prefix("oci://") {
            if reference.is_empty() {
                return Err(BuilderError::BinarySourceInvalid {
                    input: source.to_string(),
                    detail: "oci:// reference is empty".to_string(),
                });
            }
            return Ok(BinarySource::Oci {
                reference: reference.to_string(),
                sha256: sha,
            });
        }

        if s.starts_with("http://") || s.starts_with("https://") {
            return Ok(BinarySource::Url {
                url: s.to_string(),
                sha256: sha,
            });
        }

        if s.starts_with('/') || s.starts_with("./") || s.starts_with("../") {
            // PathBuf::join replaces base_dir for absolute inputs and appends
            // for relative ones — exactly the semantics we want. Strip a leading
            // `./` so `./bin/forger` under `/foo` resolves to `/foo/bin/forger`
            // (not `/foo/./bin/forger`). `{triple}` (if present) survives as a
            // literal path component.
            let rel = s.strip_prefix("./").unwrap_or(s);
            return Ok(BinarySource::Path(base_dir.join(rel)));
        }

        Err(BuilderError::BinarySourceInvalid {
            input: source.to_string(),
            detail: "use a /path, ./relative/path, http(s):// URL, or oci:// reference".to_string(),
        })
    }
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
/// Priority (highest first):
///   1. `source` — the CLI flag or spec `builder.binary` block (chosen by the caller)
///   2. `FORGER_BUILDER_BINARY` env var
///   3. Dev fallback (cross-compiled binary in the workspace target dir)
///   4. Release fallback (GitHub release URL)
///
/// A configured source (1 or 2) that fails to resolve is an error — there is no
/// silent fall-through to the dev/release fallbacks once the user has asked for
/// something explicit.
pub async fn resolve_forger_binary(
    distro: &DistroFamily,
    source: Option<&BinarySource>,
) -> Result<ResolvedBinary, BuilderError> {
    let triple = target_triple(distro);

    if let Some(src) = source {
        return fetch_to_cache(src, triple).await;
    }

    if let Some(env_src) = env_source()? {
        return fetch_to_cache(&env_src, triple).await;
    }

    if is_dev_build() {
        return resolve_dev_binary(triple);
    }

    resolve_release_binary(triple).await
}

/// Read `FORGER_BUILDER_BINARY` (+ `_SHA256`) into a `BinarySource`, resolving
/// relative paths against the current working directory.
fn env_source() -> Result<Option<BinarySource>, BuilderError> {
    let raw = match std::env::var("FORGER_BUILDER_BINARY") {
        Ok(v) if !v.is_empty() => v,
        _ => return Ok(None),
    };
    let sha = std::env::var("FORGER_BUILDER_BINARY_SHA256")
        .ok()
        .filter(|s| !s.is_empty());
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    Ok(Some(BinarySource::parse(&raw, sha.as_deref(), &cwd)?))
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
    let cache_dir = builder_cache_dir();

    let cached_path = cache_dir.join(format!("forger-{triple}-v{version}"));

    if cached_path.exists() {
        info!(path = %cached_path.display(), "Using cached forger binary");
        return Ok(ResolvedBinary { path: cached_path });
    }

    let url = release_url(version, triple);
    info!(%url, "Downloading forger binary for builder VM");

    let bytes = fetch_url(&url).await?;

    write_cache_atomic(&cache_dir, &cached_path, &bytes, &url)?;

    Ok(ResolvedBinary { path: cached_path })
}

fn release_url(version: &str, triple: &str) -> String {
    format!(
        "https://github.com/CloudNebulaProject/refraction-forger/releases/download/v{version}/forger-{triple}"
    )
}

/// Builder-binary cache directory: `${XDG_CACHE_HOME or ~/.cache}/forger/builder-binaries/`.
fn builder_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("forger")
        .join("builder-binaries")
}

/// Fetch a configured source into the real builder-binary cache.
async fn fetch_to_cache(src: &BinarySource, triple: &str) -> Result<ResolvedBinary, BuilderError> {
    fetch_into(src, triple, &builder_cache_dir()).await
}

/// Resolve a configured source into `cache_dir`, fetching + verifying + caching
/// as needed. Split out from [`fetch_to_cache`] so tests can inject a cache dir.
async fn fetch_into(
    src: &BinarySource,
    triple: &str,
    cache_dir: &Path,
) -> Result<ResolvedBinary, BuilderError> {
    // Substitute {triple} into the source string; this is the value used for
    // fetching, error messages, and (when unpinned) the cache key.
    let (display, sha256): (String, Option<String>) = match src {
        BinarySource::Path(path) => (substitute_triple(&path.to_string_lossy(), triple), None),
        BinarySource::Url { url, sha256 } => (substitute_triple(url, triple), sha256.clone()),
        BinarySource::Oci { reference, sha256 } => {
            (substitute_triple(reference, triple), sha256.clone())
        }
    };

    let cache_path = cache_dir.join(cache_file_name(triple, &display, sha256.as_deref()));

    std::fs::create_dir_all(cache_dir).map_err(|e| BuilderError::BinaryDownloadFailed {
        url: display.clone(),
        detail: format!("creating cache dir: {e}"),
    })?;

    // Cache-hit handling.
    if cache_path.exists() {
        match &sha256 {
            Some(expected) => {
                if file_sha256(&cache_path)? == *expected {
                    info!(path = %cache_path.display(), "Using cached forger binary (sha256 verified)");
                    return Ok(ResolvedBinary { path: cache_path });
                }
                tracing::warn!(
                    path = %cache_path.display(),
                    "cached forger binary failed sha256 verification — re-fetching"
                );
                std::fs::remove_file(&cache_path).ok();
            }
            None => {
                info!(path = %cache_path.display(), "Using cached forger binary");
                return Ok(ResolvedBinary { path: cache_path });
            }
        }
    }

    // Fetch the bytes.
    let bytes = match src {
        BinarySource::Path(_) => {
            let p = PathBuf::from(&display);
            info!(path = %p.display(), "Copying local forger binary into cache");
            std::fs::read(&p).map_err(|e| BuilderError::BinaryDownloadFailed {
                url: display.clone(),
                detail: format!("reading local binary: {e}"),
            })?
        }
        BinarySource::Url { .. } => fetch_url(&display).await?,
        BinarySource::Oci { .. } => fetch_oci(&display).await?,
    };

    // Verify against the pin (if any), or warn that none was supplied.
    if let Some(expected) = &sha256 {
        let got = bytes_sha256(&bytes);
        if got != *expected {
            return Err(BuilderError::BinarySha256Mismatch {
                url: display,
                expected: expected.clone(),
                got,
            });
        }
    } else if !matches!(src, BinarySource::Path(_)) {
        info!("forger binary fetched without sha256 pin; consider adding one");
    }

    write_cache_atomic(cache_dir, &cache_path, &bytes, &display)?;
    info!(path = %cache_path.display(), "Cached forger binary");
    Ok(ResolvedBinary { path: cache_path })
}

/// Fetch bytes over HTTP(S).
async fn fetch_url(url: &str) -> Result<Vec<u8>, BuilderError> {
    info!(%url, "Downloading forger binary");
    let response = reqwest::get(url)
        .await
        .map_err(|e| BuilderError::BinaryDownloadFailed {
            url: url.to_string(),
            detail: e.to_string(),
        })?;

    if !response.status().is_success() {
        return Err(BuilderError::BinaryDownloadFailed {
            url: url.to_string(),
            detail: format!("HTTP {}", response.status()),
        });
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| BuilderError::BinaryDownloadFailed {
            url: url.to_string(),
            detail: format!("reading response body: {e}"),
        })?;

    Ok(bytes.to_vec())
}

/// Acceptable layer media types for a forger OCI artifact.
const OCI_BINARY_MEDIA_TYPE: &str = "application/vnd.refraction.forger.binary.v1";
const OCI_OCTET_STREAM: &str = "application/octet-stream";

/// Pull the single-layer payload from an OCI artifact reference.
async fn fetch_oci(reference_str: &str) -> Result<Vec<u8>, BuilderError> {
    use oci_client::client::{ClientConfig, ClientProtocol};
    use oci_client::manifest::OciManifest;
    use oci_client::{Client, Reference};

    let reference: Reference =
        reference_str
            .parse()
            .map_err(|e| BuilderError::BinarySourceInvalid {
                input: format!("oci://{reference_str}"),
                detail: format!("invalid OCI reference: {e}"),
            })?;
    let registry = reference.registry().to_string();

    // Use plain HTTP for local/test registries; HTTPS everywhere else.
    let protocol = if registry.starts_with("localhost") || registry.starts_with("127.0.0.1") {
        ClientProtocol::Http
    } else {
        ClientProtocol::Https
    };
    let client = Client::new(ClientConfig {
        protocol,
        ..Default::default()
    });

    let auth = oci_auth(&registry);

    let (manifest, _digest) = client
        .pull_manifest(&reference, &auth)
        .await
        .map_err(|e| BuilderError::OciPullFailed {
            reference: reference_str.to_string(),
            detail: e.to_string(),
        })?;

    let image = match manifest {
        OciManifest::Image(m) => m,
        OciManifest::ImageIndex(idx) => {
            // A multi-platform index isn't a forger artifact — reject and point
            // the user at per-platform {triple} tags.
            return Err(BuilderError::OciMalformed {
                reference: reference_str.to_string(),
                layer_count: idx.manifests.len(),
            });
        }
    };

    if image.layers.len() != 1 {
        return Err(BuilderError::OciMalformed {
            reference: reference_str.to_string(),
            layer_count: image.layers.len(),
        });
    }

    let layer = &image.layers[0];
    if layer.media_type != OCI_BINARY_MEDIA_TYPE && layer.media_type != OCI_OCTET_STREAM {
        return Err(BuilderError::OciUnsupportedLayer {
            reference: reference_str.to_string(),
            media_type: layer.media_type.clone(),
        });
    }

    let mut buf: Vec<u8> = Vec::new();
    client
        .pull_blob(&reference, layer, &mut buf)
        .await
        .map_err(|e| BuilderError::OciPullFailed {
            reference: reference_str.to_string(),
            detail: format!("pulling layer blob: {e}"),
        })?;

    Ok(buf)
}

/// Resolve OCI registry auth: pre-resolved bearer token, then docker
/// config.json basic auth, else anonymous.
fn oci_auth(registry: &str) -> oci_client::secrets::RegistryAuth {
    use oci_client::secrets::RegistryAuth;

    if let Ok(token) = std::env::var("FORGER_OCI_BEARER") {
        if !token.is_empty() {
            return RegistryAuth::Bearer(token);
        }
    }

    if let Some((user, pass)) = docker_config_auth(registry) {
        return RegistryAuth::Basic(user, pass);
    }

    RegistryAuth::Anonymous
}

/// Extract `auths[<registry>].auth` (base64 `user:pass`) from the docker
/// config.json. Credential helpers (credsStore / credHelpers) are out of scope.
fn docker_config_auth(registry: &str) -> Option<(String, String)> {
    use base64::Engine;

    let config_path = std::env::var("DOCKER_CONFIG")
        .ok()
        .map(|d| PathBuf::from(d).join("config.json"))
        .or_else(|| dirs::home_dir().map(|h| h.join(".docker").join("config.json")))?;

    let content = std::fs::read_to_string(&config_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let auths = json.get("auths")?.as_object()?;

    let entry = auths
        .get(registry)
        .or_else(|| auths.get(&format!("https://{registry}")))
        .or_else(|| auths.get(&format!("https://{registry}/v1/")))?;

    let b64 = entry.get("auth")?.as_str()?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    let pair = String::from_utf8(decoded).ok()?;
    let (user, pass) = pair.split_once(':')?;
    Some((user.to_string(), pass.to_string()))
}

/// Cache filename for a source.
///
/// Content-addressed when a sha256 is pinned; otherwise keyed by a sha1 of the
/// post-`{triple}`-substitution source string (best-effort dedupe + cache
/// invalidation when the source changes).
fn cache_file_name(triple: &str, substituted_source: &str, sha256: Option<&str>) -> String {
    match sha256 {
        Some(hex) => format!("forger-{triple}-sha256-{hex}"),
        None => format!("forger-{triple}-{}", sha1_hex(substituted_source)),
    }
}

/// Atomically write `bytes` to `cache_path` via a uniquely-named tempfile in the
/// same directory, chmod 0755, then rename over the destination.
fn write_cache_atomic(
    cache_dir: &Path,
    cache_path: &Path,
    bytes: &[u8],
    display: &str,
) -> Result<(), BuilderError> {
    use std::io::Write;

    let mut tmp = tempfile::NamedTempFile::new_in(cache_dir).map_err(|e| {
        BuilderError::BinaryDownloadFailed {
            url: display.to_string(),
            detail: format!("creating cache tempfile: {e}"),
        }
    })?;
    tmp.write_all(bytes)
        .map_err(|e| BuilderError::BinaryDownloadFailed {
            url: display.to_string(),
            detail: format!("writing cache tempfile: {e}"),
        })?;
    tmp.flush()
        .map_err(|e| BuilderError::BinaryDownloadFailed {
            url: display.to_string(),
            detail: format!("flushing cache tempfile: {e}"),
        })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o755)).map_err(
            |e| BuilderError::BinaryDownloadFailed {
                url: display.to_string(),
                detail: format!("chmod cache tempfile: {e}"),
            },
        )?;
    }

    tmp.persist(cache_path)
        .map_err(|e| BuilderError::BinaryDownloadFailed {
            url: display.to_string(),
            detail: format!("persisting cache file: {e}"),
        })?;

    Ok(())
}

/// Substitute the `{triple}` token in a source string.
fn substitute_triple(source: &str, triple: &str) -> String {
    source.replace("{triple}", triple)
}

/// Normalize a user-supplied sha256: drop an optional `sha256:` prefix, lowercase.
fn normalize_sha256(s: &str) -> String {
    let t = s.trim();
    t.strip_prefix("sha256:").unwrap_or(t).to_ascii_lowercase()
}

/// Hex sha256 of a byte slice.
fn bytes_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Hex sha256 of a file's contents.
fn file_sha256(path: &Path) -> Result<String, BuilderError> {
    let bytes = std::fs::read(path).map_err(|e| BuilderError::BinaryDownloadFailed {
        url: path.display().to_string(),
        detail: format!("reading cache entry for verification: {e}"),
    })?;
    Ok(bytes_sha256(&bytes))
}

/// Hex sha1 of a string (used for non-content-addressed cache keys).
fn sha1_hex(s: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
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
        // `is_dev_build()` is a path heuristic: true iff the running exe lives
        // under a `/target/` directory. Assert consistency with the exe path
        // rather than a hardcoded `true`, so a custom CARGO_TARGET_DIR (e.g.
        // `~/.cache/cargo-target`) doesn't spuriously fail the test.
        let exe = std::env::current_exe().unwrap();
        let expected = exe.to_string_lossy().contains("/target/");
        assert_eq!(is_dev_build(), expected);
    }

    // --- BinarySource::parse ---------------------------------------------

    #[test]
    fn parse_oci_strips_prefix() {
        let src =
            BinarySource::parse("oci://ghcr.io/org/forger:tag", Some("ABC"), Path::new("/")).unwrap();
        assert_eq!(
            src,
            BinarySource::Oci {
                reference: "ghcr.io/org/forger:tag".to_string(),
                sha256: Some("abc".to_string()),
            }
        );
    }

    #[test]
    fn parse_http_and_https() {
        let http = BinarySource::parse("http://h/forger", None, Path::new("/")).unwrap();
        assert_eq!(
            http,
            BinarySource::Url {
                url: "http://h/forger".to_string(),
                sha256: None
            }
        );
        let https = BinarySource::parse("https://h/forger", None, Path::new("/")).unwrap();
        assert!(matches!(https, BinarySource::Url { .. }));
    }

    #[test]
    fn parse_absolute_path() {
        let src = BinarySource::parse("/opt/forger", None, Path::new("/spec/dir")).unwrap();
        assert_eq!(src, BinarySource::Path(PathBuf::from("/opt/forger")));
    }

    #[test]
    fn parse_relative_path_uses_base_dir() {
        // Matches the spec example: `./bin/forger` at `/tmp/foo` -> `/tmp/foo/bin/forger`.
        let src = BinarySource::parse("./bin/forger", None, Path::new("/tmp/foo")).unwrap();
        assert_eq!(src, BinarySource::Path(PathBuf::from("/tmp/foo/bin/forger")));
    }

    #[test]
    fn parse_rejects_unprefixed() {
        let err = BinarySource::parse("forger", None, Path::new("/")).unwrap_err();
        assert!(matches!(err, BuilderError::BinarySourceInvalid { .. }));
    }

    #[test]
    fn parse_rejects_empty() {
        let err = BinarySource::parse("   ", None, Path::new("/")).unwrap_err();
        assert!(matches!(err, BuilderError::BinarySourceInvalid { .. }));
    }

    #[test]
    fn parse_rejects_empty_oci_ref() {
        let err = BinarySource::parse("oci://", None, Path::new("/")).unwrap_err();
        assert!(matches!(err, BuilderError::BinarySourceInvalid { .. }));
    }

    #[test]
    fn sha256_normalization_strips_prefix_and_lowercases() {
        let src = BinarySource::parse("https://h/f", Some("sha256:DEAD"), Path::new("/")).unwrap();
        match src {
            BinarySource::Url { sha256, .. } => assert_eq!(sha256.as_deref(), Some("dead")),
            _ => panic!("expected Url"),
        }
    }

    // --- {triple} substitution -------------------------------------------

    #[test]
    fn triple_substitution() {
        assert_eq!(
            substitute_triple("http://h/forger-{triple}", "x86_64-unknown-linux-gnu"),
            "http://h/forger-x86_64-unknown-linux-gnu"
        );
        assert_eq!(substitute_triple("http://h/forger", "t"), "http://h/forger");
    }

    // --- cache key derivation --------------------------------------------

    #[test]
    fn cache_key_content_addressed_when_pinned() {
        assert_eq!(
            cache_file_name("triple", "http://h/forger", Some("abc123")),
            "forger-triple-sha256-abc123"
        );
    }

    #[test]
    fn cache_key_is_deterministic_and_source_sensitive() {
        let a = cache_file_name("t", "http://h/forger", None);
        let a2 = cache_file_name("t", "http://h/forger", None);
        let b = cache_file_name("t", "http://h/other", None);
        let c = cache_file_name("u", "http://h/forger", None);
        assert_eq!(a, a2, "same inputs -> same key");
        assert_ne!(a, b, "different source -> different key");
        assert_ne!(a, c, "different triple -> different key");
    }

    #[test]
    fn cache_key_uses_sha1_of_source() {
        // sha1("http://h/forger") computed independently.
        let expected = sha1_hex("http://h/forger");
        assert_eq!(
            cache_file_name("t", "http://h/forger", None),
            format!("forger-t-{expected}")
        );
        assert_eq!(expected.len(), 40, "sha1 hex is 40 chars");
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TRIPLE: &str = "x86_64-unknown-linux-gnu";

    fn cache_entries(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("forger-"))
            .collect()
    }

    // --- HTTP -------------------------------------------------------------

    #[tokio::test]
    async fn http_fetch_substitutes_triple_caches_and_skips_second_request() {
        let server = MockServer::start().await;
        let payload = vec![7u8; 1024];

        // The request path must contain the *resolved* triple, not `{triple}`.
        Mock::given(method("GET"))
            .and(path(format!("/forger-{TRIPLE}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
            .expect(1) // second resolve is a cache hit -> no HTTP
            .mount(&server)
            .await;

        let cache = tempfile::tempdir().unwrap();
        let src = BinarySource::Url {
            url: format!("{}/forger-{{triple}}", server.uri()),
            sha256: None,
        };

        let r1 = fetch_into(&src, TRIPLE, cache.path()).await.unwrap();
        assert_eq!(std::fs::read(&r1.path).unwrap(), payload);

        let r2 = fetch_into(&src, TRIPLE, cache.path()).await.unwrap();
        assert_eq!(r1.path, r2.path, "second resolve returns the same cache entry");
        // `expect(1)` is asserted when `server` drops.
    }

    #[tokio::test]
    async fn http_sha256_verifies_on_match() {
        let server = MockServer::start().await;
        let payload = b"the-real-binary".to_vec();
        let good = bytes_sha256(&payload);

        Mock::given(method("GET"))
            .and(path("/f"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
            .mount(&server)
            .await;

        let cache = tempfile::tempdir().unwrap();
        let src = BinarySource::Url {
            url: format!("{}/f", server.uri()),
            sha256: Some(good.clone()),
        };

        let r = fetch_into(&src, TRIPLE, cache.path()).await.unwrap();
        assert_eq!(std::fs::read(&r.path).unwrap(), payload);
        assert_eq!(
            r.path.file_name().unwrap().to_string_lossy(),
            format!("forger-{TRIPLE}-sha256-{good}")
        );
    }

    #[tokio::test]
    async fn http_sha256_mismatch_is_not_cached() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/f"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"wrong".to_vec()))
            .mount(&server)
            .await;

        let cache = tempfile::tempdir().unwrap();
        let src = BinarySource::Url {
            url: format!("{}/f", server.uri()),
            sha256: Some("deadbeef".to_string()),
        };

        let err = fetch_into(&src, TRIPLE, cache.path()).await.unwrap_err();
        assert!(matches!(err, BuilderError::BinarySha256Mismatch { .. }));
        assert!(
            cache_entries(cache.path()).is_empty(),
            "mismatched download must not populate the cache"
        );
    }

    #[tokio::test]
    async fn http_404_configured_source_errors_without_fallthrough() {
        // A configured source that 404s is an error, not a fall-through to the
        // dev/release fallbacks. resolve_forger_binary short-circuits on the
        // configured source, so no release-URL request is ever made.
        let server = MockServer::start().await;
        // No mock mounted -> wiremock responds 404 to everything.
        let src = BinarySource::Url {
            url: format!("{}/missing", server.uri()),
            sha256: None,
        };

        let err = resolve_forger_binary(&DistroFamily::Ubuntu, Some(&src))
            .await
            .unwrap_err();
        assert!(matches!(err, BuilderError::BinaryDownloadFailed { .. }));
    }

    #[tokio::test]
    async fn http_cache_poisoning_is_detected_and_refetched() {
        let server = MockServer::start().await;
        let payload = b"correct-binary".to_vec();
        let good = bytes_sha256(&payload);

        Mock::given(method("GET"))
            .and(path("/f"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
            .mount(&server)
            .await;

        let cache = tempfile::tempdir().unwrap();
        // Pre-populate the content-addressed entry with WRONG bytes.
        let entry = cache.path().join(format!("forger-{TRIPLE}-sha256-{good}"));
        std::fs::write(&entry, b"poisoned").unwrap();

        let src = BinarySource::Url {
            url: format!("{}/f", server.uri()),
            sha256: Some(good.clone()),
        };

        let r = fetch_into(&src, TRIPLE, cache.path()).await.unwrap();
        assert_eq!(
            std::fs::read(&r.path).unwrap(),
            payload,
            "poisoned entry must be replaced with the verified bytes"
        );
    }

    // --- local path -------------------------------------------------------

    #[tokio::test]
    async fn path_source_copies_and_survives_source_deletion() {
        let cache = tempfile::tempdir().unwrap();
        let srcdir = tempfile::tempdir().unwrap();
        let srcfile = srcdir.path().join("forger");
        std::fs::write(&srcfile, b"local-bin").unwrap();

        let src = BinarySource::Path(srcfile.clone());

        let r1 = fetch_into(&src, TRIPLE, cache.path()).await.unwrap();
        assert_eq!(std::fs::read(&r1.path).unwrap(), b"local-bin");

        // Delete the source; the cache must still serve valid bytes (copy, not symlink).
        std::fs::remove_file(&srcfile).unwrap();
        let r2 = fetch_into(&src, TRIPLE, cache.path()).await.unwrap();
        assert_eq!(std::fs::read(&r2.path).unwrap(), b"local-bin");
    }

    // --- env var ----------------------------------------------------------

    #[tokio::test]
    async fn env_var_source_is_parsed_and_overrides_dev_fallback() {
        // This is the ONLY test that mutates FORGER_BUILDER_BINARY / XDG_CACHE_HOME,
        // so it is race-free even under parallel execution. It covers the
        // "env-var source" bullet end to end: parsing + sha normalization,
        // {triple} substitution, sha256 verification, and the fact that the env
        // var beats the dev/release fallbacks.
        let server = MockServer::start().await;
        let payload = vec![3u8; 256];
        Mock::given(method("GET"))
            .and(path(format!("/forger-{TRIPLE}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
            .mount(&server)
            .await;

        let cache = tempfile::tempdir().unwrap();
        let sha = bytes_sha256(&payload);

        // SAFETY: see the comment above — no other test touches these vars.
        unsafe {
            std::env::set_var(
                "FORGER_BUILDER_BINARY",
                format!("{}/forger-{{triple}}", server.uri()),
            );
            std::env::set_var("FORGER_BUILDER_BINARY_SHA256", sha.to_uppercase());
            std::env::set_var("XDG_CACHE_HOME", cache.path());
        }

        // (a) parsing + sha normalization
        let parsed = env_source().unwrap().unwrap();
        // (b) priority: the env var resolves (and verifies) even though the dev
        // fallback would otherwise apply.
        let resolved = resolve_forger_binary(&DistroFamily::Ubuntu, None).await;

        unsafe {
            std::env::remove_var("FORGER_BUILDER_BINARY");
            std::env::remove_var("FORGER_BUILDER_BINARY_SHA256");
            std::env::remove_var("XDG_CACHE_HOME");
        }

        assert_eq!(
            parsed,
            BinarySource::Url {
                url: format!("{}/forger-{{triple}}", server.uri()),
                sha256: Some(sha.clone()),
            }
        );
        let resolved = resolved.expect("env-var source should resolve");
        assert_eq!(std::fs::read(&resolved.path).unwrap(), payload);
    }

    // --- OCI --------------------------------------------------------------

    fn registry_host(server: &MockServer) -> String {
        server.uri().strip_prefix("http://").unwrap().to_string()
    }

    async fn mount_manifest(server: &MockServer, name: &str, tag: &str, body: String) {
        Mock::given(method("GET"))
            .and(path(format!("/v2/{name}/manifests/{tag}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
                    .set_body_string(body),
            )
            .mount(server)
            .await;
    }

    fn config_descriptor() -> serde_json::Value {
        // "{}" config blob; never fetched by the resolver.
        let cfg = b"{}";
        json!({
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": format!("sha256:{}", bytes_sha256(cfg)),
            "size": cfg.len(),
        })
    }

    #[tokio::test]
    async fn oci_single_layer_artifact_pulls_payload() {
        let server = MockServer::start().await;
        let payload = vec![42u8; 2048];
        let digest = format!("sha256:{}", bytes_sha256(&payload));

        let manifest = json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": config_descriptor(),
            "layers": [{
                "mediaType": OCI_BINARY_MEDIA_TYPE,
                "digest": digest,
                "size": payload.len(),
            }],
        })
        .to_string();

        mount_manifest(&server, "forger", "v1", manifest).await;
        Mock::given(method("GET"))
            .and(path(format!("/v2/forger/blobs/{digest}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
            .mount(&server)
            .await;

        let cache = tempfile::tempdir().unwrap();
        let src = BinarySource::Oci {
            reference: format!("{}/forger:v1", registry_host(&server)),
            sha256: None,
        };

        let r = fetch_into(&src, TRIPLE, cache.path()).await.unwrap();
        assert_eq!(std::fs::read(&r.path).unwrap(), payload);
    }

    #[tokio::test]
    async fn oci_multi_layer_is_malformed() {
        let server = MockServer::start().await;
        let manifest = json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": config_descriptor(),
            "layers": [
                {"mediaType": OCI_BINARY_MEDIA_TYPE, "digest": "sha256:aa", "size": 1},
                {"mediaType": OCI_BINARY_MEDIA_TYPE, "digest": "sha256:bb", "size": 1},
            ],
        })
        .to_string();
        mount_manifest(&server, "forger", "v1", manifest).await;

        let cache = tempfile::tempdir().unwrap();
        let src = BinarySource::Oci {
            reference: format!("{}/forger:v1", registry_host(&server)),
            sha256: None,
        };

        let err = fetch_into(&src, TRIPLE, cache.path()).await.unwrap_err();
        assert!(matches!(
            err,
            BuilderError::OciMalformed { layer_count: 2, .. }
        ));
    }

    #[tokio::test]
    async fn oci_unsupported_layer_media_type_is_rejected() {
        let server = MockServer::start().await;
        let manifest = json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": config_descriptor(),
            "layers": [{
                "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                "digest": "sha256:cc",
                "size": 1,
            }],
        })
        .to_string();
        mount_manifest(&server, "forger", "v1", manifest).await;

        let cache = tempfile::tempdir().unwrap();
        let src = BinarySource::Oci {
            reference: format!("{}/forger:v1", registry_host(&server)),
            sha256: None,
        };

        let err = fetch_into(&src, TRIPLE, cache.path()).await.unwrap_err();
        match err {
            BuilderError::OciUnsupportedLayer { media_type, .. } => {
                assert_eq!(media_type, "application/vnd.oci.image.layer.v1.tar+gzip");
            }
            other => panic!("expected OciUnsupportedLayer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn oci_sha256_mismatch_is_not_cached() {
        let server = MockServer::start().await;
        let payload = vec![9u8; 512];
        let digest = format!("sha256:{}", bytes_sha256(&payload));

        let manifest = json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": config_descriptor(),
            "layers": [{
                "mediaType": OCI_BINARY_MEDIA_TYPE,
                "digest": digest,
                "size": payload.len(),
            }],
        })
        .to_string();
        mount_manifest(&server, "forger", "v1", manifest).await;
        Mock::given(method("GET"))
            .and(path(format!("/v2/forger/blobs/{digest}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
            .mount(&server)
            .await;

        let cache = tempfile::tempdir().unwrap();
        let src = BinarySource::Oci {
            reference: format!("{}/forger:v1", registry_host(&server)),
            // Pin a sha that does not match the (valid) layer payload.
            sha256: Some(bytes_sha256(b"something-else")),
        };

        let err = fetch_into(&src, TRIPLE, cache.path()).await.unwrap_err();
        assert!(matches!(err, BuilderError::BinarySha256Mismatch { .. }));
        assert!(cache_entries(cache.path()).is_empty());
    }

    #[tokio::test]
    async fn oci_octet_stream_layer_is_accepted() {
        let server = MockServer::start().await;
        let payload = vec![5u8; 777];
        let digest = format!("sha256:{}", bytes_sha256(&payload));

        let manifest = json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": config_descriptor(),
            "layers": [{
                "mediaType": "application/octet-stream",
                "digest": digest,
                "size": payload.len(),
            }],
        })
        .to_string();
        mount_manifest(&server, "forger", "v1", manifest).await;
        Mock::given(method("GET"))
            .and(path(format!("/v2/forger/blobs/{digest}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
            .mount(&server)
            .await;

        let cache = tempfile::tempdir().unwrap();
        let src = BinarySource::Oci {
            reference: format!("{}/forger:v1", registry_host(&server)),
            sha256: None,
        };

        let r = fetch_into(&src, TRIPLE, cache.path()).await.unwrap();
        assert_eq!(std::fs::read(&r.path).unwrap(), payload);
    }

    #[tokio::test]
    async fn oci_reference_substitutes_triple_before_pull() {
        // `{triple}` in an OCI reference must be substituted before the registry
        // call: the manifest is served under the *resolved* tag, so a literal
        // `{triple}` request would 404.
        let server = MockServer::start().await;
        let payload = vec![8u8; 321];
        let digest = format!("sha256:{}", bytes_sha256(&payload));

        let manifest = json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": config_descriptor(),
            "layers": [{
                "mediaType": OCI_BINARY_MEDIA_TYPE,
                "digest": digest,
                "size": payload.len(),
            }],
        })
        .to_string();
        // Tag is the resolved triple, not the literal token.
        mount_manifest(&server, "forger", TRIPLE, manifest).await;
        Mock::given(method("GET"))
            .and(path(format!("/v2/forger/blobs/{digest}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
            .mount(&server)
            .await;

        let cache = tempfile::tempdir().unwrap();
        let src = BinarySource::Oci {
            reference: format!("{}/forger:{{triple}}", registry_host(&server)),
            sha256: None,
        };

        let r = fetch_into(&src, TRIPLE, cache.path()).await.unwrap();
        assert_eq!(std::fs::read(&r.path).unwrap(), payload);
    }
}
