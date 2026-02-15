pub mod profile;
pub mod resolve;
pub mod schema;

use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum ParseError {
    #[error("Failed to parse KDL spec")]
    #[diagnostic(
        help("Check the KDL syntax in your spec file"),
        code(spec_parser::kdl_parse)
    )]
    KdlError {
        detail: String,
    },
}

impl From<knuffel::Error> for ParseError {
    fn from(err: knuffel::Error) -> Self {
        ParseError::KdlError {
            detail: err.to_string(),
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
}
