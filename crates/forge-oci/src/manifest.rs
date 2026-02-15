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
    // Build the diff_ids for the rootfs (uncompressed layer digests aren't tracked here,
    // so we use the compressed digest -- in a full implementation you'd track both)
    let diff_ids: Vec<String> = layers.iter().map(|l| l.digest.clone()).collect();

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
