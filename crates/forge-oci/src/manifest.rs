use miette::Diagnostic;
use oci_spec::image::{
    ConfigBuilder, DescriptorBuilder, ImageConfigurationBuilder, ImageManifestBuilder,
    MediaType, RootFsBuilder, Sha256Digest,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::tar_layer::LayerBlob;

#[derive(Debug, Error, Diagnostic)]
pub enum ManifestError {
    #[error("Failed to build OCI image configuration")]
    #[diagnostic(help("This is likely a bug in the manifest builder"))]
    ConfigBuild(String),

    #[error("Failed to build OCI image manifest")]
    #[diagnostic(help("This is likely a bug in the manifest builder"))]
    ManifestBuild(String),

    #[error("Failed to serialize OCI manifest to JSON")]
    Serialize(#[source] serde_json::Error),
}

/// Options for building an OCI image configuration.
pub struct ImageOptions {
    pub os: String,
    pub architecture: String,
    pub entrypoint: Option<Vec<String>>,
    pub env: Vec<String>,
}

impl Default for ImageOptions {
    fn default() -> Self {
        Self {
            os: "solaris".to_string(),
            architecture: "amd64".to_string(),
            entrypoint: None,
            env: Vec::new(),
        }
    }
}

/// Build the OCI image configuration JSON and image manifest from a set of layers.
///
/// Returns `(config_json, manifest_json)`.
pub fn build_manifest(
    layers: &[LayerBlob],
    options: &ImageOptions,
) -> Result<(Vec<u8>, Vec<u8>), ManifestError> {
    // diff_ids must be uncompressed layer digests per OCI image spec
    let diff_ids: Vec<String> = layers.iter().map(|l| l.uncompressed_digest.clone()).collect();

    let rootfs = RootFsBuilder::default()
        .typ("layers")
        .diff_ids(diff_ids)
        .build()
        .map_err(|e| ManifestError::ConfigBuild(e.to_string()))?;

    let mut config_builder = ImageConfigurationBuilder::default()
        .os(options.os.as_str())
        .architecture(options.architecture.as_str())
        .rootfs(rootfs);

    // Build a config block with optional entrypoint/env
    let mut inner_config_builder = ConfigBuilder::default();
    if let Some(ref ep) = options.entrypoint {
        inner_config_builder = inner_config_builder.entrypoint(ep.clone());
    }
    if !options.env.is_empty() {
        inner_config_builder = inner_config_builder.env(options.env.clone());
    }
    let inner_config = inner_config_builder
        .build()
        .map_err(|e| ManifestError::ConfigBuild(e.to_string()))?;
    config_builder = config_builder.config(inner_config);

    let image_config = config_builder
        .build()
        .map_err(|e| ManifestError::ConfigBuild(e.to_string()))?;

    let config_json =
        serde_json::to_vec_pretty(&image_config).map_err(ManifestError::Serialize)?;

    let mut config_hasher = Sha256::new();
    config_hasher.update(&config_json);
    let config_digest = format!("sha256:{}", hex::encode(config_hasher.finalize()));

    let config_sha_digest: Sha256Digest = config_digest
        .strip_prefix("sha256:")
        .unwrap_or(&config_digest)
        .parse()
        .map_err(|e: oci_spec::OciSpecError| ManifestError::ConfigBuild(e.to_string()))?;

    let config_descriptor = DescriptorBuilder::default()
        .media_type(MediaType::ImageConfig)
        .size(config_json.len() as u64)
        .digest(config_sha_digest)
        .build()
        .map_err(|e| ManifestError::ConfigBuild(e.to_string()))?;

    // Build layer descriptors
    let layer_descriptors: Vec<_> = layers
        .iter()
        .map(|layer| {
            let layer_sha: Sha256Digest = layer
                .digest
                .strip_prefix("sha256:")
                .unwrap_or(&layer.digest)
                .parse()
                .map_err(|e: oci_spec::OciSpecError| {
                    ManifestError::ManifestBuild(e.to_string())
                })?;

            DescriptorBuilder::default()
                .media_type(MediaType::ImageLayerGzip)
                .size(layer.data.len() as u64)
                .digest(layer_sha)
                .build()
                .map_err(|e| ManifestError::ManifestBuild(e.to_string()))
        })
        .collect::<Result<_, _>>()?;

    let manifest = ImageManifestBuilder::default()
        .schema_version(2u32)
        .media_type(MediaType::ImageManifest)
        .config(config_descriptor)
        .layers(layer_descriptors)
        .build()
        .map_err(|e| ManifestError::ManifestBuild(e.to_string()))?;

    let manifest_json = serde_json::to_vec_pretty(&manifest).map_err(ManifestError::Serialize)?;

    Ok((config_json, manifest_json))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tar_layer;
    use tempfile::TempDir;

    fn make_test_layer() -> LayerBlob {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("hello.txt"), "hello").unwrap();
        tar_layer::create_layer(tmp.path()).unwrap()
    }

    #[test]
    fn test_build_manifest_default_options() {
        let layer = make_test_layer();
        let (config_json, manifest_json) =
            build_manifest(&[layer], &ImageOptions::default()).unwrap();

        let config: serde_json::Value = serde_json::from_slice(&config_json).unwrap();
        // Default is OmniOS → "solaris" in OCI terms
        assert_eq!(config["os"], "solaris");
        assert_eq!(config["architecture"], "amd64");

        let manifest: serde_json::Value = serde_json::from_slice(&manifest_json).unwrap();
        assert_eq!(manifest["schemaVersion"], 2);
        assert_eq!(manifest["layers"].as_array().unwrap().len(), 1);
        assert_eq!(
            manifest["layers"][0]["mediaType"],
            "application/vnd.oci.image.layer.v1.tar+gzip"
        );
    }

    #[test]
    fn test_build_manifest_with_entrypoint_and_env() {
        let layer = make_test_layer();
        let options = ImageOptions {
            os: "linux".to_string(),
            architecture: "arm64".to_string(),
            entrypoint: Some(vec!["/bin/sh".to_string(), "-c".to_string()]),
            env: vec!["PATH=/bin:/usr/bin".to_string(), "HOME=/root".to_string()],
        };

        let (config_json, _manifest_json) = build_manifest(&[layer], &options).unwrap();

        let config: serde_json::Value = serde_json::from_slice(&config_json).unwrap();
        assert_eq!(config["os"], "linux");
        assert_eq!(config["architecture"], "arm64");
        let ep = config["config"]["Entrypoint"].as_array().unwrap();
        assert_eq!(ep.len(), 2);
        assert_eq!(ep[0], "/bin/sh");
        let env = config["config"]["Env"].as_array().unwrap();
        assert_eq!(env.len(), 2);
        assert_eq!(env[0], "PATH=/bin:/usr/bin");
    }

    #[test]
    fn test_build_manifest_multiple_layers() {
        let tmp1 = TempDir::new().unwrap();
        std::fs::write(tmp1.path().join("a.txt"), "aaa").unwrap();
        let layer1 = tar_layer::create_layer(tmp1.path()).unwrap();

        let tmp2 = TempDir::new().unwrap();
        std::fs::write(tmp2.path().join("b.txt"), "bbb").unwrap();
        let layer2 = tar_layer::create_layer(tmp2.path()).unwrap();

        let (config_json, manifest_json) =
            build_manifest(&[layer1, layer2], &ImageOptions::default()).unwrap();

        let config: serde_json::Value = serde_json::from_slice(&config_json).unwrap();
        let diff_ids = config["rootfs"]["diff_ids"].as_array().unwrap();
        assert_eq!(diff_ids.len(), 2);

        let manifest: serde_json::Value = serde_json::from_slice(&manifest_json).unwrap();
        assert_eq!(manifest["layers"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_build_manifest_config_has_valid_digest_in_manifest() {
        let layer = make_test_layer();
        let (config_json, manifest_json) =
            build_manifest(&[layer], &ImageOptions::default()).unwrap();

        let manifest: serde_json::Value = serde_json::from_slice(&manifest_json).unwrap();
        let config_digest = manifest["config"]["digest"].as_str().unwrap();
        assert!(config_digest.starts_with("sha256:"));

        // Verify digest matches actual config content
        let mut hasher = Sha256::new();
        hasher.update(&config_json);
        let expected = format!("sha256:{}", hex::encode(hasher.finalize()));
        assert_eq!(config_digest, expected);
    }

    #[test]
    fn test_build_manifest_no_entrypoint() {
        let layer = make_test_layer();
        let options = ImageOptions {
            entrypoint: None,
            env: Vec::new(),
            ..Default::default()
        };

        let (config_json, _) = build_manifest(&[layer], &options).unwrap();
        let config: serde_json::Value = serde_json::from_slice(&config_json).unwrap();

        // Config block should still exist but without entrypoint
        assert!(config["config"]["Entrypoint"].is_null());
    }
}
