use std::path::Path;

use spec_parser::schema::Target;
use tracing::info;

use crate::error::ForgeError;

/// Build an OCI container image from the staged rootfs.
pub fn build_oci(
    target: &Target,
    staging_root: &Path,
    output_dir: &Path,
) -> Result<(), ForgeError> {
    info!("Building OCI container image");

    // Create the tar.gz layer from staging
    let layer =
        forge_oci::tar_layer::create_layer(staging_root).map_err(|e| ForgeError::OciBuild(e.to_string()))?;

    info!(
        digest = %layer.digest,
        size = layer.data.len(),
        "Layer created"
    );

    // Build image options from target spec
    let mut options = forge_oci::manifest::ImageOptions::default();

    if let Some(ref ep) = target.entrypoint {
        options.entrypoint = Some(vec![ep.command.clone()]);
    }

    if let Some(ref env) = target.environment {
        options.env = env
            .vars
            .iter()
            .map(|v| format!("{}={}", v.key, v.value))
            .collect();
    }

    // Build manifest and config
    let (config_json, manifest_json) = forge_oci::manifest::build_manifest(&[layer.clone()], &options)
        .map_err(|e| ForgeError::OciBuild(e.to_string()))?;

    // Write OCI Image Layout
    let oci_output = output_dir.join(format!("{}-oci", target.name));
    forge_oci::layout::write_oci_layout(&oci_output, &[layer], &config_json, &manifest_json)
        .map_err(|e| ForgeError::OciBuild(e.to_string()))?;

    info!(path = %oci_output.display(), "OCI Image Layout written");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use spec_parser::schema::{Entrypoint, Environment, EnvVar, TargetKind};
    use tempfile::TempDir;

    fn make_target(name: &str, entrypoint: Option<Entrypoint>, env: Option<Environment>) -> Target {
        Target {
            name: name.to_string(),
            kind: TargetKind::Oci,
            disk_size: None,
            bootloader: None,
            entrypoint,
            environment: env,
            pool: None,
        }
    }

    #[test]
    fn test_build_oci_produces_layout() {
        let staging = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        // Create some files in staging
        std::fs::create_dir_all(staging.path().join("etc")).unwrap();
        std::fs::write(staging.path().join("etc/hostname"), "test-vm\n").unwrap();
        std::fs::write(staging.path().join("etc/motd"), "Welcome\n").unwrap();

        let target = make_target("container", None, None);
        build_oci(&target, staging.path(), output.path()).unwrap();

        let oci_dir = output.path().join("container-oci");
        assert!(oci_dir.exists());
        assert!(oci_dir.join("oci-layout").exists());
        assert!(oci_dir.join("index.json").exists());
        assert!(oci_dir.join("blobs/sha256").is_dir());
    }

    #[test]
    fn test_build_oci_with_entrypoint_and_env() {
        let staging = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        std::fs::write(staging.path().join("hello.txt"), "hi").unwrap();

        let target = make_target(
            "app",
            Some(Entrypoint {
                command: "/bin/sh".to_string(),
            }),
            Some(Environment {
                vars: vec![EnvVar {
                    key: "PATH".to_string(),
                    value: "/bin:/usr/bin".to_string(),
                }],
            }),
        );

        build_oci(&target, staging.path(), output.path()).unwrap();

        let oci_dir = output.path().join("app-oci");
        assert!(oci_dir.join("oci-layout").exists());

        // Verify index.json is valid
        let index: serde_json::Value = serde_json::from_slice(
            &std::fs::read(oci_dir.join("index.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(index["schemaVersion"], 2);
    }

    #[test]
    fn test_build_oci_empty_staging() {
        let staging = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();

        let target = make_target("minimal", None, None);
        build_oci(&target, staging.path(), output.path()).unwrap();

        assert!(output.path().join("minimal-oci/oci-layout").exists());
    }
}
