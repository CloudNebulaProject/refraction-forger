use std::path::PathBuf;
use std::time::Duration;

use ssh2::Session;
use tracing::info;

use vm_manager::image::ImageManager;
use vm_manager::traits::Hypervisor;
use vm_manager::types::{CloudInitConfig, NetworkConfig, SshConfig, VmHandle, VmSpec};
use vm_manager::RouterHypervisor;

use crate::config::BuilderConfig;
use crate::error::BuilderError;

/// An active builder VM session with SSH connectivity.
pub struct BuilderSession {
    pub hypervisor: RouterHypervisor,
    pub handle: VmHandle,
    pub ssh_session: Session,
    pub ssh_port: u16,
}

impl BuilderSession {
    /// Start a builder VM: resolve image, generate SSH keys, create + boot VM, connect SSH.
    pub async fn start(config: &BuilderConfig) -> Result<Self, BuilderError> {
        info!(image = %config.image, vcpus = config.vcpus, memory_mb = config.memory_mb, disk_gb = config.disk_gb, "Starting builder VM");

        // 1. Resolve builder image
        let image_path = resolve_builder_image(&config.image).await?;

        // 2. Generate ephemeral SSH keypair
        let (pub_key, priv_pem) = generate_ssh_keypair()?;

        // 3. Build cloud-config with builder user + injected pubkey + disk growth
        // growpart + resize_rootfs ensure the root partition expands to fill the
        // resized overlay disk (cloud images ship with tiny 2GB roots).
        let cloud_config = format!(
            r#"#cloud-config
users:
  - name: builder
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    ssh_authorized_keys:
      - {pub_key}

growpart:
  mode: auto
  devices:
    - /

resize_rootfs: true
"#
        );

        let ssh_config = SshConfig {
            user: "builder".to_string(),
            public_key: Some(pub_key),
            private_key_path: None,
            private_key_pem: Some(priv_pem),
        };

        // 4. Create VmSpec with user-mode networking (no root needed on host)
        let vm_name = format!("forger-builder-{}", uuid_short());
        let spec = VmSpec {
            name: vm_name.clone(),
            image_path: image_path.clone(),
            vcpus: config.vcpus,
            memory_mb: config.memory_mb,
            disk_gb: Some(config.disk_gb),
            network: NetworkConfig::User,
            cloud_init: Some(CloudInitConfig {
                user_data: cloud_config.into_bytes(),
                instance_id: Some(vm_name.clone()),
                hostname: Some("builder".to_string()),
            }),
            ssh: Some(ssh_config.clone()),
        };

        // 5. Prepare + start VM
        let hypervisor = RouterHypervisor::new(None, None);

        let handle = hypervisor.prepare(&spec).await.map_err(|e| {
            BuilderError::VmLifecycle {
                phase: "prepare".into(),
                detail: e.to_string(),
            }
        })?;

        let handle = hypervisor.start(&handle).await.map_err(|e| {
            BuilderError::VmLifecycle {
                phase: "start".into(),
                detail: e.to_string(),
            }
        })?;

        // 6. Connect SSH with retry (user-mode networking uses host port forwarding)
        let ssh_port = handle.ssh_host_port.unwrap_or(22);
        let ssh_ip = "127.0.0.1";

        info!(port = ssh_port, "Waiting for SSH connection to builder VM");

        let ssh_session = vm_manager::ssh::connect_with_retry(
            ssh_ip,
            ssh_port,
            &ssh_config,
            Duration::from_secs(120),
        )
        .await
        .map_err(|e| BuilderError::VmLifecycle {
            phase: "ssh_connect".into(),
            detail: e.to_string(),
        })?;

        info!("Builder VM ready");

        Ok(Self {
            hypervisor,
            handle,
            ssh_session,
            ssh_port,
        })
    }

    /// Tear down the builder VM, destroying all resources.
    pub async fn teardown(self) -> Result<(), BuilderError> {
        info!(name = %self.handle.name, "Tearing down builder VM");

        // Drop SSH session first
        drop(self.ssh_session);

        self.hypervisor
            .destroy(self.handle)
            .await
            .map_err(|e| BuilderError::VmLifecycle {
                phase: "destroy".into(),
                detail: e.to_string(),
            })?;

        Ok(())
    }
}

/// Resolve builder image from OCI reference, URL, or local path.
async fn resolve_builder_image(image: &str) -> Result<PathBuf, BuilderError> {
    let mgr = ImageManager::new();

    if image.starts_with("oci://") {
        let reference = image.strip_prefix("oci://").unwrap();
        mgr.pull_oci(reference, None)
            .await
            .map_err(|e| BuilderError::ImageResolveFailed {
                detail: format!("OCI pull {reference}: {e}"),
            })
    } else if image.starts_with("http://") || image.starts_with("https://") {
        mgr.pull(image, None)
            .await
            .map_err(|e| BuilderError::ImageResolveFailed {
                detail: format!("download {image}: {e}"),
            })
    } else {
        // Local path
        let path = PathBuf::from(image);
        if !path.exists() {
            return Err(BuilderError::ImageResolveFailed {
                detail: format!("local image not found: {}", path.display()),
            });
        }
        Ok(path)
    }
}

fn generate_ssh_keypair() -> Result<(String, String), BuilderError> {
    use ssh_key::{Algorithm, LineEnding, PrivateKey, rand_core::OsRng};

    let sk = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).map_err(|e| {
        BuilderError::KeygenFailed {
            detail: format!("Ed25519 key generation: {e}"),
        }
    })?;

    let pub_openssh = sk.public_key().to_openssh().map_err(|e| {
        BuilderError::KeygenFailed {
            detail: format!("serialize public key: {e}"),
        }
    })?;

    let priv_pem = sk.to_openssh(LineEnding::LF).map_err(|e| {
        BuilderError::KeygenFailed {
            detail: format!("serialize private key: {e}"),
        }
    })?;

    Ok((pub_openssh, priv_pem.to_string()))
}

fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{:x}", ts & 0xFFFF_FFFF)
}
