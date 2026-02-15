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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tar_layer;
    use tempfile::TempDir;

    fn make_test_layer() -> LayerBlob {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("test.txt"), "test content").unwrap();
        tar_layer::create_layer(tmp.path()).unwrap()
    }

    #[test]
    fn test_write_oci_layout_creates_structure() {
        let output = TempDir::new().unwrap();
        let layer = make_test_layer();

        let config_json = b"{\"test\": \"config\"}";
        let manifest_json = b"{\"test\": \"manifest\"}";

        write_oci_layout(output.path(), &[layer], config_json, manifest_json).unwrap();

        // Verify directory structure
        assert!(output.path().join("oci-layout").exists());
        assert!(output.path().join("index.json").exists());
        assert!(output.path().join("blobs/sha256").is_dir());
    }

    #[test]
    fn test_oci_layout_file_content() {
        let output = TempDir::new().unwrap();
        let layer = make_test_layer();
        let config_json = b"{}";
        let manifest_json = b"{}";

        write_oci_layout(output.path(), &[layer], config_json, manifest_json).unwrap();

        let oci_layout: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output.path().join("oci-layout")).unwrap())
                .unwrap();
        assert_eq!(oci_layout["imageLayoutVersion"], "1.0.0");
    }

    #[test]
    fn test_index_json_references_manifest() {
        let output = TempDir::new().unwrap();
        let layer = make_test_layer();
        let config_json = b"{\"config\": true}";
        let manifest_json = b"{\"manifest\": true}";

        write_oci_layout(output.path(), &[layer], config_json, manifest_json).unwrap();

        let index: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output.path().join("index.json")).unwrap())
                .unwrap();

        assert_eq!(index["schemaVersion"], 2);
        let manifests = index["manifests"].as_array().unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(
            manifests[0]["mediaType"],
            "application/vnd.oci.image.manifest.v1+json"
        );
        let digest = manifests[0]["digest"].as_str().unwrap();
        assert!(digest.starts_with("sha256:"));
        assert_eq!(manifests[0]["size"], manifest_json.len());
    }

    #[test]
    fn test_layer_blob_written_to_blobs_dir() {
        let output = TempDir::new().unwrap();
        let layer = make_test_layer();
        let digest_hex = layer.digest.strip_prefix("sha256:").unwrap().to_string();
        let expected_data = layer.data.clone();

        write_oci_layout(output.path(), &[layer], b"{}", b"{}").unwrap();

        let blob_path = output.path().join("blobs/sha256").join(&digest_hex);
        assert!(blob_path.exists(), "Layer blob not found at {}", blob_path.display());
        assert_eq!(std::fs::read(&blob_path).unwrap(), expected_data);
    }

    #[test]
    fn test_config_blob_written_with_correct_digest() {
        let output = TempDir::new().unwrap();
        let layer = make_test_layer();
        let config_json = b"{\"architecture\":\"amd64\"}";

        let mut hasher = Sha256::new();
        hasher.update(config_json);
        let config_digest_hex = hex::encode(hasher.finalize());

        write_oci_layout(output.path(), &[layer], config_json, b"{}").unwrap();

        let config_blob_path = output.path().join("blobs/sha256").join(&config_digest_hex);
        assert!(config_blob_path.exists());
        assert_eq!(std::fs::read(&config_blob_path).unwrap(), config_json);
    }

    #[test]
    fn test_multiple_layers() {
        let output = TempDir::new().unwrap();

        let tmp1 = TempDir::new().unwrap();
        std::fs::write(tmp1.path().join("a.txt"), "aaa").unwrap();
        let layer1 = tar_layer::create_layer(tmp1.path()).unwrap();

        let tmp2 = TempDir::new().unwrap();
        std::fs::write(tmp2.path().join("b.txt"), "bbb").unwrap();
        let layer2 = tar_layer::create_layer(tmp2.path()).unwrap();

        let digest1 = layer1.digest.strip_prefix("sha256:").unwrap().to_string();
        let digest2 = layer2.digest.strip_prefix("sha256:").unwrap().to_string();

        write_oci_layout(output.path(), &[layer1, layer2], b"{}", b"{}").unwrap();

        assert!(output.path().join("blobs/sha256").join(&digest1).exists());
        assert!(output.path().join("blobs/sha256").join(&digest2).exists());
    }
}
