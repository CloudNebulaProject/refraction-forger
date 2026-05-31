use std::path::{Path, PathBuf};

use forge_engine::output::{OutputHandler, OutputMode};
use miette::{Context, IntoDiagnostic};

/// Push an OCI Image Layout or QCOW2 artifact to a registry.
pub async fn run(
    image_dir: &PathBuf,
    reference: &str,
    auth_file: Option<&PathBuf>,
    artifact: bool,
    output_mode: OutputMode,
) -> miette::Result<()> {
    let output = OutputHandler::new(output_mode);
    let auth = resolve_auth(auth_file, reference)?;

    // Determine if we need insecure registries (localhost)
    let insecure = if reference.starts_with("localhost") || reference.starts_with("127.0.0.1") {
        let host_port = reference.split('/').next().unwrap_or("");
        vec![host_port.to_string()]
    } else {
        vec![]
    };

    if artifact {
        return push_artifact(image_dir, reference, &auth, &insecure, &output).await;
    }

    push_oci_layout(image_dir, reference, &auth, &insecure, &output).await
}

/// Push a QCOW2 file directly as an OCI artifact.
async fn push_artifact(
    qcow2_path: &PathBuf,
    reference: &str,
    auth: &forge_oci::registry::AuthConfig,
    insecure: &[String],
    output: &OutputHandler,
) -> miette::Result<()> {
    output.phase_start(&format!("Pushing QCOW2 artifact → {reference}"));
    output.info(&format!("Reading {}", qcow2_path.display()));

    let qcow2_data = std::fs::read(qcow2_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read QCOW2 file: {}", qcow2_path.display()))?;

    let size_mb = qcow2_data.len() as f64 / 1_048_576.0;
    output.info(&format!("Image size: {size_mb:.1} MB"));

    let name = qcow2_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image")
        .to_string();

    let metadata = forge_oci::artifact::Qcow2Metadata {
        name,
        version: "latest".to_string(),
        architecture: "amd64".to_string(),
        os: "linux".to_string(),
        description: None,
    };

    output.info("Uploading to registry...");

    let manifest_url =
        forge_oci::artifact::push_qcow2_artifact(reference, qcow2_data, &metadata, auth, insecure)
            .await
            .map_err(miette::Report::new)
            .wrap_err("Artifact push failed")?;

    output.phase_done(&format!("Pushed → {manifest_url}"));
    Ok(())
}

/// Push an OCI Image Layout to a registry.
async fn push_oci_layout(
    image_dir: &Path,
    reference: &str,
    auth: &forge_oci::registry::AuthConfig,
    insecure: &[String],
    output: &OutputHandler,
) -> miette::Result<()> {
    output.phase_start(&format!("Pushing OCI image → {reference}"));

    // Read the OCI Image Layout index.json
    let index_path = image_dir.join("index.json");
    let index_content = std::fs::read_to_string(&index_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read OCI index: {}", index_path.display()))?;

    let index: serde_json::Value =
        serde_json::from_str(&index_content).into_diagnostic()?;

    let manifests = index["manifests"]
        .as_array()
        .ok_or_else(|| miette::miette!("Invalid OCI index: missing manifests array"))?;

    if manifests.is_empty() {
        return Err(miette::miette!("OCI index contains no manifests"));
    }

    // Read the manifest
    let manifest_digest = manifests[0]["digest"]
        .as_str()
        .ok_or_else(|| miette::miette!("Invalid manifest entry: missing digest"))?;

    let digest_hex = manifest_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| miette::miette!("Unsupported digest algorithm: {manifest_digest}"))?;

    let manifest_path = image_dir.join("blobs/sha256").join(digest_hex);
    let manifest_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&manifest_path)
            .into_diagnostic()
            .wrap_err("Failed to read manifest blob")?,
    )
    .into_diagnostic()?;

    // Read config blob
    let config_digest = manifest_json["config"]["digest"]
        .as_str()
        .ok_or_else(|| miette::miette!("Missing config digest in manifest"))?;
    let config_hex = config_digest.strip_prefix("sha256:").unwrap_or(config_digest);
    let config_json = std::fs::read(image_dir.join("blobs/sha256").join(config_hex))
        .into_diagnostic()
        .wrap_err("Failed to read config blob")?;

    // Read layer blobs
    let layers_json = manifest_json["layers"]
        .as_array()
        .ok_or_else(|| miette::miette!("Missing layers in manifest"))?;

    output.info(&format!("Uploading {} layer(s)...", layers_json.len()));

    let mut layers = Vec::new();
    for layer_desc in layers_json {
        let layer_digest = layer_desc["digest"]
            .as_str()
            .ok_or_else(|| miette::miette!("Missing layer digest"))?;
        let layer_hex = layer_digest
            .strip_prefix("sha256:")
            .unwrap_or(layer_digest);

        let layer_data = std::fs::read(image_dir.join("blobs/sha256").join(layer_hex))
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read layer blob: {layer_digest}"))?;

        // Decompress to get uncompressed size and digest for diff_id
        let mut decoder = flate2::read::GzDecoder::new(layer_data.as_slice());
        let mut uncompressed = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut uncompressed)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to decompress layer: {layer_digest}"))?;

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&uncompressed);
        let uncompressed_digest = format!("sha256:{}", hex::encode(hasher.finalize()));

        layers.push(forge_oci::tar_layer::LayerBlob {
            data: layer_data,
            digest: layer_digest.to_string(),
            uncompressed_digest,
            uncompressed_size: uncompressed.len() as u64,
        });
    }

    let manifest_url =
        forge_oci::registry::push_image(reference, layers, config_json, auth, insecure)
            .await
            .map_err(miette::Report::new)
            .wrap_err("Push failed")?;

    output.phase_done(&format!("Pushed → {manifest_url}"));
    Ok(())
}

/// Resolve authentication from an auth file or environment.
///
/// Auth file formats:
///   - `{"username": "user", "password": "pass"}` → Basic auth
///   - `{"token": "pat-value"}` → Basic auth with token (username from reference)
///   - `{"token": "pat-value", "username": "user"}` → Basic auth with explicit username
///
/// Fallback: `GITHUB_TOKEN` env var for ghcr.io, otherwise anonymous.
fn resolve_auth(
    auth_file: Option<&PathBuf>,
    reference: &str,
) -> miette::Result<forge_oci::registry::AuthConfig> {
    if let Some(auth_path) = auth_file {
        let auth_content = std::fs::read_to_string(auth_path)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read auth file: {}", auth_path.display()))?;

        let auth_json: serde_json::Value =
            serde_json::from_str(&auth_content).into_diagnostic()?;

        // Explicit username + password → Basic auth
        if let (Some(user), Some(pass)) = (
            auth_json["username"].as_str(),
            auth_json["password"].as_str(),
        ) {
            return Ok(forge_oci::registry::AuthConfig::Basic {
                username: user.to_string(),
                password: pass.to_string(),
            });
        }

        // Token-based: use as Basic auth password (most registries including
        // Forgejo, Gitea, and GHCR accept PATs via Basic auth)
        if let Some(token) = auth_json["token"].as_str() {
            let username = auth_json["username"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| infer_username_from_reference(reference));

            return Ok(forge_oci::registry::AuthConfig::Basic {
                username,
                password: token.to_string(),
            });
        }

        Ok(forge_oci::registry::AuthConfig::Anonymous)
    } else {
        // Try GITHUB_TOKEN for ghcr.io
        Ok(forge_oci::artifact::resolve_ghcr_auth())
    }
}

/// Infer a username from the registry reference for token-based auth.
/// Most registries accept any non-empty username with a PAT as password.
fn infer_username_from_reference(_reference: &str) -> String {
    "forger".to_string()
}
