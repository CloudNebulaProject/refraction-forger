use miette::Diagnostic;
use oci_client::client::{ClientConfig, ClientProtocol, Config, ImageLayer};
use oci_client::manifest;
use oci_client::secrets::RegistryAuth;
use oci_client::{Client, Reference};
use thiserror::Error;
use tracing::info;

use crate::tar_layer::LayerBlob;

#[derive(Debug, Error, Diagnostic)]
pub enum RegistryError {
    #[error("Invalid image reference: {reference}")]
    #[diagnostic(help(
        "Use the format <registry>/<repository>:<tag>, e.g. ghcr.io/org/image:v1"
    ))]
    InvalidReference {
        reference: String,
        #[source]
        source: oci_client::ParseError,
    },

    #[error("Failed to push image to registry")]
    #[diagnostic(help("Check registry URL, credentials, and network connectivity"))]
    PushFailed(String),

    #[error("Failed to read auth file: {path}")]
    #[diagnostic(help("Ensure the auth file exists and contains valid JSON"))]
    AuthFileRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Authentication configuration for registry operations.
pub enum AuthConfig {
    Anonymous,
    Basic { username: String, password: String },
    Bearer { token: String },
}

impl AuthConfig {
    pub fn to_registry_auth(&self) -> RegistryAuth {
        match self {
            AuthConfig::Anonymous => RegistryAuth::Anonymous,
            AuthConfig::Basic { username, password } => {
                RegistryAuth::Basic(username.clone(), password.clone())
            }
            AuthConfig::Bearer { token } => RegistryAuth::Bearer(token.clone()),
        }
    }
}

/// Push an OCI image (layers + config + manifest) to a registry.
pub async fn push_image(
    reference_str: &str,
    layers: Vec<LayerBlob>,
    config_json: Vec<u8>,
    auth: &AuthConfig,
    insecure_registries: &[String],
) -> Result<String, RegistryError> {
    let reference: Reference = reference_str
        .parse()
        .map_err(|e| RegistryError::InvalidReference {
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

    let oci_layers: Vec<ImageLayer> = layers
        .into_iter()
        .map(|layer| {
            ImageLayer::new(
                layer.data,
                manifest::IMAGE_LAYER_GZIP_MEDIA_TYPE.to_string(),
                None,
            )
        })
        .collect();

    let config = Config::new(
        config_json,
        manifest::IMAGE_CONFIG_MEDIA_TYPE.to_string(),
        None,
    );

    let image_manifest =
        oci_client::manifest::OciImageManifest::build(&oci_layers, &config, None);

    info!(reference = %reference, "Pushing image to registry");

    let response = client
        .push(
            &reference,
            &oci_layers,
            config,
            &registry_auth,
            Some(image_manifest),
        )
        .await
        .map_err(|e| RegistryError::PushFailed(e.to_string()))?;

    info!(
        manifest_url = %response.manifest_url,
        "Image pushed successfully"
    );

    Ok(response.manifest_url)
}
