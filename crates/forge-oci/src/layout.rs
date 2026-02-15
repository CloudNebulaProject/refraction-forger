use std::path::Path;

use miette::Diagnostic;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::tar_layer::LayerBlob;

#[derive(Debug, Error, Diagnostic)]
pub enum LayoutError {
    #[error("Failed to create OCI layout directory: {path}")]
    #[diagnostic(help("Ensure the parent directory exists and is writable"))]
    CreateDir {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write OCI layout file: {path}")]
    #[diagnostic(help("Check disk space and permissions"))]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to build OCI manifest")]
    ManifestError(#[from] crate::manifest::ManifestError),

    #[error("Failed to serialize OCI layout JSON")]
    Serialize(#[from] serde_json::Error),
}

/// Write an OCI Image Layout directory at `output_dir`.
///
/// Structure:
/// ```text
/// output_dir/
///   oci-layout
///   index.json
///   blobs/
///     sha256/
///       <config-digest>
///       <layer-digest>...
///       <manifest-digest>
/// ```
pub fn write_oci_layout(
    output_dir: &Path,
    layers: &[LayerBlob],
    config_json: &[u8],
    manifest_json: &[u8],
) -> Result<(), LayoutError> {
    let blobs_dir = output_dir.join("blobs").join("sha256");
    std::fs::create_dir_all(&blobs_dir).map_err(|e| LayoutError::CreateDir {
        path: blobs_dir.display().to_string(),
        source: e,
    })?;

    // Write oci-layout
    let oci_layout = serde_json::json!({
        "imageLayoutVersion": "1.0.0"
    });
    write_file(
        &output_dir.join("oci-layout"),
        serde_json::to_vec_pretty(&oci_layout)?.as_slice(),
    )?;

    // Write layer blobs
    for layer in layers {
        let digest_hex = layer
            .digest
            .strip_prefix("sha256:")
            .unwrap_or(&layer.digest);
        write_file(&blobs_dir.join(digest_hex), &layer.data)?;
    }

    // Write config blob
    let mut config_hasher = Sha256::new();
    config_hasher.update(config_json);
    let config_digest_hex = hex::encode(config_hasher.finalize());
    write_file(&blobs_dir.join(&config_digest_hex), config_json)?;

    // Write manifest blob
    let mut manifest_hasher = Sha256::new();
    manifest_hasher.update(manifest_json);
    let manifest_digest_hex = hex::encode(manifest_hasher.finalize());
    write_file(&blobs_dir.join(&manifest_digest_hex), manifest_json)?;

    // Write index.json
    let index = serde_json::json!({
        "schemaVersion": 2,
        "manifests": [
            {
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": format!("sha256:{manifest_digest_hex}"),
                "size": manifest_json.len()
            }
        ]
    });
    write_file(
        &output_dir.join("index.json"),
        serde_json::to_vec_pretty(&index)?.as_slice(),
    )?;

    Ok(())
}

fn write_file(path: &Path, data: &[u8]) -> Result<(), LayoutError> {
    std::fs::write(path, data).map_err(|e| LayoutError::WriteFile {
        path: path.display().to_string(),
        source: e,
    })
}
