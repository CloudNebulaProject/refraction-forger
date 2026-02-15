use spec_parser::schema::{BuilderNode, DistroFamily};

/// Resolved builder VM configuration with defaults applied.
#[derive(Debug, Clone)]
pub struct BuilderConfig {
    /// OCI reference, URL, or local path to the builder VM image.
    pub image: String,
    /// Number of virtual CPUs for the builder VM.
    pub vcpus: u16,
    /// Memory in MB for the builder VM.
    pub memory_mb: u64,
}

impl BuilderConfig {
    /// Resolve a BuilderConfig from the optional spec node and distro family.
    /// Spec values take precedence; convention defaults fill the rest.
    pub fn resolve(spec_builder: Option<&BuilderNode>, distro: &DistroFamily) -> Self {
        let default_image = Self::default_image(distro);

        match spec_builder {
            Some(node) => Self {
                image: node.image.clone().unwrap_or(default_image),
                vcpus: node.vcpus.unwrap_or(2),
                memory_mb: node.memory.unwrap_or(2048),
            },
            None => Self {
                image: default_image,
                vcpus: 2,
                memory_mb: 2048,
            },
        }
    }

    fn default_image(distro: &DistroFamily) -> String {
        match distro {
            DistroFamily::OmniOS => {
                "oci://ghcr.io/cloudnebulaproject/omnios-builder:latest".to_string()
            }
            DistroFamily::Ubuntu => {
                "oci://ghcr.io/cloudnebulaproject/ubuntu-builder:latest".to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_for_ubuntu() {
        let config = BuilderConfig::resolve(None, &DistroFamily::Ubuntu);
        assert!(config.image.contains("ubuntu-builder"));
        assert_eq!(config.vcpus, 2);
        assert_eq!(config.memory_mb, 2048);
    }

    #[test]
    fn defaults_for_omnios() {
        let config = BuilderConfig::resolve(None, &DistroFamily::OmniOS);
        assert!(config.image.contains("omnios-builder"));
    }

    #[test]
    fn spec_overrides_defaults() {
        let node = BuilderNode {
            image: Some("oci://custom/image:v1".to_string()),
            vcpus: Some(4),
            memory: Some(4096),
        };
        let config = BuilderConfig::resolve(Some(&node), &DistroFamily::Ubuntu);
        assert_eq!(config.image, "oci://custom/image:v1");
        assert_eq!(config.vcpus, 4);
        assert_eq!(config.memory_mb, 4096);
    }

    #[test]
    fn partial_spec_fills_remaining_with_defaults() {
        let node = BuilderNode {
            image: None,
            vcpus: Some(8),
            memory: None,
        };
        let config = BuilderConfig::resolve(Some(&node), &DistroFamily::Ubuntu);
        assert!(config.image.contains("ubuntu-builder"));
        assert_eq!(config.vcpus, 8);
        assert_eq!(config.memory_mb, 2048);
    }
}
