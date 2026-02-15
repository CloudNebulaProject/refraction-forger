use std::path::PathBuf;

use miette::{Context, IntoDiagnostic};
use tracing::info;

/// Push an OCI Image Layout to a registry.
pub async fn run(
    image_dir: &PathBuf,
    reference: &str,
    auth_file: Option<&PathBuf>,
) -> miette::Result<()> {
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

        layers.push(forge_oci::tar_layer::LayerBlob {
            data: layer_data,
            digest: layer_digest.to_string(),
            uncompressed_size: 0, // Not tracked in layout
        });
    }

    // Determine auth
    let auth = if let Some(auth_path) = auth_file {
        let auth_content = std::fs::read_to_string(auth_path)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read auth file: {}", auth_path.display()))?;

        let auth_json: serde_json::Value =
            serde_json::from_str(&auth_content).into_diagnostic()?;

        if let Some(token) = auth_json["token"].as_str() {
            forge_oci::registry::AuthConfig::Bearer {
                token: token.to_string(),
            }
        } else if let (Some(user), Some(pass)) = (
            auth_json["username"].as_str(),
            auth_json["password"].as_str(),
        ) {
            forge_oci::registry::AuthConfig::Basic {
                username: user.to_string(),
                password: pass.to_string(),
            }
        } else {
            forge_oci::registry::AuthConfig::Anonymous
        }
    } else {
        forge_oci::registry::AuthConfig::Anonymous
    };

    // Determine if we need insecure registries (localhost)
    let insecure = if reference.starts_with("localhost") || reference.starts_with("127.0.0.1") {
        let host_port = reference.split('/').next().unwrap_or("");
        vec![host_port.to_string()]
    } else {
        vec![]
    };

    info!(reference, "Pushing OCI image to registry");

    let manifest_url =
        forge_oci::registry::push_image(reference, layers, config_json, &auth, &insecure)
            .await
            .map_err(miette::Report::new)
            .wrap_err("Push failed")?;

    println!("Pushed: {manifest_url}");
    Ok(())
}
