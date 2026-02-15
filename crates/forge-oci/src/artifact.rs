use miette::Diagnostic;
use oci_client::client::{ClientConfig, ClientProtocol, Config, ImageLayer};
use oci_client::{Client, Reference};
use thiserror::Error;
use tracing::info;

use crate::registry::AuthConfig;

pub const QCOW2_CONFIG_MEDIA_TYPE: &str = "application/vnd.cloudnebula.qcow2.config.v1+json";
pub const QCOW2_LAYER_MEDIA_TYPE: &str = "application/vnd.cloudnebula.qcow2.layer.v1";

#[derive(Debug, Error, Diagnostic)]
pub enum ArtifactError {
    #[error("Invalid OCI reference: {reference}")]
    #[diagnostic(help(
        "Use the format <registry>/<repository>:<tag>, e.g. ghcr.io/org/image:v1"
    ))]
    InvalidReference {
        reference: String,
        #[source]
        source: oci_client::ParseError,
    },

    #[error("Failed to push QCOW2 artifact to registry: {detail}")]
    #[diagnostic(help(
        "Check registry URL, credentials, and network connectivity. For ghcr.io, ensure GITHUB_TOKEN is set."
    ))]
    PushFailed { detail: String },

    #[error("Failed to pull QCOW2 artifact from registry: {detail}")]
    #[diagnostic(help(
        "Check registry URL, credentials, and network connectivity. For ghcr.io, ensure GITHUB_TOKEN is set."
    ))]
    PullFailed { detail: String },
}

/// Metadata for a QCOW2 artifact pushed as an OCI artifact.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Qcow2Metadata {
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub os: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Push a QCOW2 file as an OCI artifact to a registry.
///
/// Uses ORAS-compatible custom media types so the artifact can be pulled
/// with `oras pull` or our own `pull_qcow2_artifact`.
pub async fn push_qcow2_artifact(
    reference_str: &str,
    qcow2_data: Vec<u8>,
    metadata: &Qcow2Metadata,
    auth: &AuthConfig,
    insecure_registries: &[String],
) -> Result<String, ArtifactError> {
    let reference: Reference = reference_str
        .parse()
        .map_err(|e| ArtifactError::InvalidReference {
            reference: reference_str.to_string(),
            source: e,
        })?;

    let client_config = ClientConfig {
        protocol: if insecure_registries.is_empty() {
            ClientProtocol::Https
        } else {
            ClientProtocol::HttpsExcept(insecure_registries.to_vec())
        },
        ..Default::default()
    };

    let client = Client::new(client_config);
    let registry_auth = auth.to_registry_auth();

    // Config blob is the JSON metadata
    let config_json =
        serde_json::to_vec(metadata).map_err(|e| ArtifactError::PushFailed {
            detail: format!("failed to serialize metadata: {e}"),
        })?;

    // Build the OCI layer with annotations
    let mut annotations = std::collections::BTreeMap::new();
    annotations.insert(
        "org.opencontainers.image.title".to_string(),
        format!("{}.qcow2", metadata.name),
    );

    let layer = ImageLayer::new(qcow2_data, QCOW2_LAYER_MEDIA_TYPE.to_string(), Some(annotations));

    let config = Config::new(config_json, QCOW2_CONFIG_MEDIA_TYPE.to_string(), None);

    let image_manifest =
        oci_client::manifest::OciImageManifest::build(&[layer.clone()], &config, None);

    info!(
        reference = %reference,
        name = %metadata.name,
        "Pushing QCOW2 artifact to registry"
    );

    let response = client
        .push(
            &reference,
            &[layer],
            config,
            &registry_auth,
            Some(image_manifest),
        )
        .await
        .map_err(|e| ArtifactError::PushFailed {
            detail: e.to_string(),
        })?;

    info!(
        manifest_url = %response.manifest_url,
        "QCOW2 artifact pushed successfully"
    );

    Ok(response.manifest_url)
}

/// Pull a QCOW2 file from an OCI artifact registry.
pub async fn pull_qcow2_artifact(
    reference_str: &str,
    auth: &AuthConfig,
    insecure_registries: &[String],
) -> Result<Vec<u8>, ArtifactError> {
    let reference: Reference = reference_str
        .parse()
        .map_err(|e| ArtifactError::InvalidReference {
            reference: reference_str.to_string(),
            source: e,
        })?;

    let client_config = ClientConfig {
        protocol: if insecure_registries.is_empty() {
            ClientProtocol::Https
        } else {
            ClientProtocol::HttpsExcept(insecure_registries.to_vec())
        },
        ..Default::default()
    };

    let client = Client::new(client_config);
    let registry_auth = auth.to_registry_auth();

    info!(reference = %reference, "Pulling QCOW2 artifact from registry");

    let image_data = client
        .pull(
            &reference,
            &registry_auth,
            vec![QCOW2_LAYER_MEDIA_TYPE, "application/octet-stream"],
        )
        .await
        .map_err(|e| ArtifactError::PullFailed {
            detail: e.to_string(),
        })?;

    let layer = image_data
        .layers
        .into_iter()
        .next()
        .ok_or_else(|| ArtifactError::PullFailed {
            detail: "artifact contains no layers".to_string(),
        })?;

    info!(
        reference = %reference,
        size_bytes = layer.data.len(),
        "QCOW2 artifact pulled successfully"
    );

    Ok(layer.data)
}

/// Resolve authentication for ghcr.io from GITHUB_TOKEN environment variable.
pub fn resolve_ghcr_auth() -> AuthConfig {
    match std::env::var("GITHUB_TOKEN") {
        Ok(token) => AuthConfig::Basic {
            username: "_token".to_string(),
            password: token,
        },
        Err(_) => AuthConfig::Anonymous,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qcow2_metadata_serialization() {
        let metadata = Qcow2Metadata {
            name: "ubuntu-rust-ci".to_string(),
            version: "0.1.0".to_string(),
            architecture: "amd64".to_string(),
            os: "linux".to_string(),
            description: Some("Ubuntu CI image with Rust".to_string()),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("ubuntu-rust-ci"));
        assert!(json.contains("amd64"));
        assert!(json.contains("Ubuntu CI image with Rust"));
    }

    #[test]
    fn test_qcow2_metadata_without_description() {
        let metadata = Qcow2Metadata {
            name: "test".to_string(),
            version: "1.0".to_string(),
            architecture: "amd64".to_string(),
            os: "linux".to_string(),
            description: None,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(!json.contains("description"));
    }

    #[test]
    fn test_media_type_constants() {
        assert_eq!(
            QCOW2_CONFIG_MEDIA_TYPE,
            "application/vnd.cloudnebula.qcow2.config.v1+json"
        );
        assert_eq!(
            QCOW2_LAYER_MEDIA_TYPE,
            "application/vnd.cloudnebula.qcow2.layer.v1"
        );
    }
}
