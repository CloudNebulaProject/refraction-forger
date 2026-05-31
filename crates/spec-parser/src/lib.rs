// thiserror/miette derive macros generate code that triggers false-positive unused_assignments
#![allow(unused_assignments)]

pub mod profile;
pub mod resolve;
pub mod schema;

use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum ParseError {
    #[error("Failed to parse KDL spec: {detail}")]
    #[diagnostic(
        help("Check the KDL syntax in your spec file"),
        code(spec_parser::kdl_parse)
    )]
    KdlError { detail: String },
}

impl From<knuffel::Error> for ParseError {
    fn from(err: knuffel::Error) -> Self {
        ParseError::KdlError {
            detail: format!("{err:?}"),
        }
    }
}

pub fn parse(kdl: &str) -> Result<schema::ImageSpec, ParseError> {
    knuffel::parse("image.kdl", kdl).map_err(ParseError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_example() {
        let kdl = r#"
            metadata name="my-image" version="1.0.0" description="A test image"

            base "path/to/base.tar.gz"
            build-host "path/to/build-vm.qcow2"

            repositories {
                publisher name="test-pub" origin="http://pkg.test.com"
            }

            incorporation "pkg:/test/incorporation"

            packages {
                package "system/kernel"
            }

            packages if="desktop" {
                package "desktop/gnome"
            }

            customization {
                user "admin"
            }

            overlays {
                file source="local/file" destination="/remote/file"
            }

            target "vm" kind="qcow2" {
                disk-size "20G"
                bootloader "grub"
            }

            target "container" kind="oci" {
                entrypoint command="/bin/sh"
                environment {
                    set "PATH" "/bin:/usr/bin"
                }
            }
        "#;

        let spec = parse(kdl).expect("Failed to parse KDL");
        assert_eq!(spec.metadata.name, "my-image");
        assert_eq!(spec.base, Some("path/to/base.tar.gz".to_string()));
        assert_eq!(
            spec.build_host,
            Some("path/to/build-vm.qcow2".to_string())
        );
        assert_eq!(spec.repositories.publishers.len(), 1);
        assert_eq!(spec.packages.len(), 2);
        assert_eq!(spec.targets.len(), 2);

        let vm_target = &spec.targets[0];
        assert_eq!(vm_target.name, "vm");
        assert_eq!(vm_target.kind, schema::TargetKind::Qcow2);
        assert_eq!(vm_target.disk_size, Some("20G".to_string()));
        assert_eq!(vm_target.bootloader, Some("grub".to_string()));

        let container_target = &spec.targets[1];
        assert_eq!(container_target.name, "container");
        assert_eq!(container_target.kind, schema::TargetKind::Oci);
        assert_eq!(
            container_target.entrypoint.as_ref().unwrap().command,
            "/bin/sh"
        );
    }

    #[test]
    fn test_parse_variants_and_certificates() {
        let kdl = r#"
            metadata name="test" version="0.1.0"
            repositories {
                publisher name="omnios" origin="https://pkg.omnios.org/bloody/core/"
            }
            variants {
                set name="opensolaris.zone" value="global"
            }
            certificates {
                ca publisher="omnios" certfile="omniosce-ca.cert.pem"
            }
        "#;

        let spec = parse(kdl).expect("Failed to parse KDL");
        let variants = spec.variants.unwrap();
        assert_eq!(variants.vars.len(), 1);
        assert_eq!(variants.vars[0].name, "opensolaris.zone");
        assert_eq!(variants.vars[0].value, "global");

        let certs = spec.certificates.unwrap();
        assert_eq!(certs.ca.len(), 1);
        assert_eq!(certs.ca[0].publisher, "omnios");
    }

    #[test]
    fn test_parse_ubuntu_spec() {
        let kdl = r#"
            metadata name="ubuntu-ci" version="0.1.0"
            distro "ubuntu-22.04"
            repositories {
                apt-mirror "http://archive.ubuntu.com/ubuntu" suite="jammy" components="main universe"
            }
            packages {
                package "build-essential"
                package "curl"
            }
            target "qcow2" kind="qcow2" {
                disk-size "8G"
                bootloader "grub"
                filesystem "ext4"
                push-to "ghcr.io/cloudnebulaproject/ubuntu-rust:latest"
            }
        "#;

        let spec = parse(kdl).expect("Failed to parse Ubuntu spec");
        assert_eq!(spec.distro, Some("ubuntu-22.04".to_string()));
        assert_eq!(spec.repositories.apt_mirrors.len(), 1);
        let mirror = &spec.repositories.apt_mirrors[0];
        assert_eq!(mirror.url, "http://archive.ubuntu.com/ubuntu");
        assert_eq!(mirror.suite, "jammy");
        assert_eq!(mirror.components, Some("main universe".to_string()));

        let target = &spec.targets[0];
        assert_eq!(target.filesystem, Some("ext4".to_string()));
        assert_eq!(
            target.push_to,
            Some("ghcr.io/cloudnebulaproject/ubuntu-rust:latest".to_string())
        );
    }

    #[test]
    fn test_parse_omnios_spec_unchanged() {
        // Existing OmniOS specs should parse without errors (backward compat)
        let kdl = r#"
            metadata name="omnios-disk" version="0.0.1"
            repositories {
                publisher name="omnios" origin="https://pkg.omnios.org/bloody/core/"
            }
            packages {
                package "system/kernel"
            }
            target "vm" kind="qcow2" {
                disk-size "2000M"
                bootloader "uefi"
                pool {
                    property name="ashift" value="12"
                }
            }
        "#;

        let spec = parse(kdl).expect("Failed to parse OmniOS spec");
        assert_eq!(spec.distro, None);
        assert!(spec.repositories.apt_mirrors.is_empty());
        assert_eq!(spec.targets[0].filesystem, None);
        assert_eq!(spec.targets[0].push_to, None);

        // DistroFamily should default to OmniOS
        assert_eq!(
            schema::DistroFamily::from_distro_str(spec.distro.as_deref()),
            schema::DistroFamily::OmniOS
        );
    }

    #[test]
    fn test_distro_family_detection() {
        assert_eq!(
            schema::DistroFamily::from_distro_str(None),
            schema::DistroFamily::OmniOS
        );
        assert_eq!(
            schema::DistroFamily::from_distro_str(Some("omnios")),
            schema::DistroFamily::OmniOS
        );
        assert_eq!(
            schema::DistroFamily::from_distro_str(Some("ubuntu-22.04")),
            schema::DistroFamily::Ubuntu
        );
        assert_eq!(
            schema::DistroFamily::from_distro_str(Some("ubuntu-24.04")),
            schema::DistroFamily::Ubuntu
        );
    }

    #[test]
    fn test_parse_pool_properties() {
        let kdl = r#"
            metadata name="test" version="0.1.0"
            repositories {}
            target "disk" kind="qcow2" {
                disk-size "2000M"
                bootloader "uefi"
                pool {
                    property name="ashift" value="12"
                }
            }
        "#;

        let spec = parse(kdl).expect("Failed to parse KDL");
        let target = &spec.targets[0];
        let pool = target.pool.as_ref().unwrap();
        assert_eq!(pool.properties.len(), 1);
        assert_eq!(pool.properties[0].name, "ashift");
        assert_eq!(pool.properties[0].value, "12");
    }

    #[test]
    fn test_parse_builder_node_full() {
        let kdl = r#"
            metadata name="test" version="0.1.0"
            repositories {}
            builder {
                image "oci://ghcr.io/custom/builder:v1"
                vcpus 4
                memory 4096
                disk 50
            }
        "#;

        let spec = parse(kdl).expect("Failed to parse KDL");
        let builder = spec.builder.as_ref().unwrap();
        assert_eq!(builder.image.as_deref(), Some("oci://ghcr.io/custom/builder:v1"));
        assert_eq!(builder.vcpus, Some(4));
        assert_eq!(builder.memory, Some(4096));
        assert_eq!(builder.disk, Some(50));
    }

    #[test]
    fn test_parse_builder_node_partial() {
        let kdl = r#"
            metadata name="test" version="0.1.0"
            repositories {}
            builder {
                vcpus 8
            }
        "#;

        let spec = parse(kdl).expect("Failed to parse KDL");
        let builder = spec.builder.as_ref().unwrap();
        assert_eq!(builder.image, None);
        assert_eq!(builder.vcpus, Some(8));
        assert_eq!(builder.memory, None);
    }

    #[test]
    fn test_parse_builder_node_empty() {
        let kdl = r#"
            metadata name="test" version="0.1.0"
            repositories {}
            builder {
            }
        "#;

        let spec = parse(kdl).expect("Failed to parse KDL");
        let builder = spec.builder.as_ref().unwrap();
        assert_eq!(builder.image, None);
        assert_eq!(builder.vcpus, None);
        assert_eq!(builder.memory, None);
        assert_eq!(builder.disk, None);
    }

    #[test]
    fn test_parse_no_builder_node() {
        let kdl = r#"
            metadata name="test" version="0.1.0"
            repositories {}
        "#;

        let spec = parse(kdl).expect("Failed to parse KDL");
        assert!(spec.builder.is_none());
    }

    #[test]
    fn test_parse_builder_binary_with_sha256() {
        let kdl = r#"
            metadata name="test" version="0.1.0"
            repositories {}
            builder {
                image "https://cloud-images.ubuntu.com/jammy.img"
                binary "http://artifacts.lan/forger-{triple}" sha256="9a3fc0"
                vcpus 4
            }
        "#;

        let spec = parse(kdl).expect("Failed to parse KDL");
        let builder = spec.builder.as_ref().unwrap();
        let binary = builder.binary.as_ref().expect("binary block present");
        // {triple} token is preserved verbatim — no KDL-level interpolation.
        assert_eq!(binary.source, "http://artifacts.lan/forger-{triple}");
        assert_eq!(binary.sha256.as_deref(), Some("9a3fc0"));
        assert_eq!(builder.vcpus, Some(4));
    }

    #[test]
    fn test_parse_builder_binary_without_sha256() {
        let kdl = r#"
            metadata name="test" version="0.1.0"
            repositories {}
            builder {
                binary "oci://ghcr.io/myorg/forger:linux-gnu"
            }
        "#;

        let spec = parse(kdl).expect("Failed to parse KDL");
        let binary = spec.builder.as_ref().unwrap().binary.as_ref().unwrap();
        assert_eq!(binary.source, "oci://ghcr.io/myorg/forger:linux-gnu");
        assert_eq!(binary.sha256, None);
    }

    #[test]
    fn test_parse_builder_without_binary_is_backward_compatible() {
        // A builder block that omits `binary` parses with binary == None.
        let kdl = r#"
            metadata name="test" version="0.1.0"
            repositories {}
            builder {
                vcpus 2
            }
        "#;

        let spec = parse(kdl).expect("Failed to parse KDL");
        assert!(spec.builder.as_ref().unwrap().binary.is_none());
    }
}
