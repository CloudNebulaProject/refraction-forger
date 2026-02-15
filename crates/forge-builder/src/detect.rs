use spec_parser::schema::{DistroFamily, ImageSpec, TargetKind};

/// Determine whether the current host needs a builder VM to build this spec.
///
/// Returns `true` when:
/// - Any matching target is QCOW2 and we're not running as root (needs losetup/mount/chroot)
/// - The distro is OmniOS and we're not on illumos (needs pkg)
pub fn needs_builder(spec: &ImageSpec, target_name: Option<&str>, force_local: bool) -> bool {
    if force_local {
        return false;
    }

    let distro = DistroFamily::from_distro_str(spec.distro.as_deref());
    let is_root = unsafe { libc::geteuid() == 0 };
    let is_illumos = cfg!(target_os = "illumos");

    let has_qcow2 = spec.targets.iter().any(|t| {
        let name_matches = target_name.is_none() || target_name == Some(t.name.as_str());
        name_matches && t.kind == TargetKind::Qcow2
    });

    (has_qcow2 && !is_root) || (distro == DistroFamily::OmniOS && !is_illumos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spec(distro: Option<&str>, targets: &[(&str, &str)]) -> ImageSpec {
        let kdl = {
            let mut s = String::new();
            s.push_str(&format!(
                "metadata name=\"test\" version=\"0.1.0\"\n"
            ));
            if let Some(d) = distro {
                s.push_str(&format!("distro \"{d}\"\n"));
            }
            s.push_str("repositories {}\n");
            for (name, kind) in targets {
                s.push_str(&format!("target \"{name}\" kind=\"{kind}\" {{\n"));
                if *kind == "qcow2" {
                    s.push_str("  disk-size \"10G\"\n");
                }
                s.push_str("}\n");
            }
            s
        };
        spec_parser::parse(&kdl).unwrap()
    }

    #[test]
    fn force_local_always_returns_false() {
        let spec = make_spec(Some("ubuntu-22.04"), &[("vm", "qcow2")]);
        assert!(!needs_builder(&spec, None, true));
    }

    #[test]
    fn no_qcow2_targets_no_builder_needed() {
        let spec = make_spec(Some("ubuntu-22.04"), &[("img", "oci")]);
        // On Linux (not illumos), Ubuntu OCI targets don't need a builder
        assert!(!needs_builder(&spec, None, false));
    }

    #[test]
    fn qcow2_without_root_needs_builder() {
        let spec = make_spec(Some("ubuntu-22.04"), &[("vm", "qcow2")]);
        // This test is meaningful when run as non-root (CI, developer machines)
        let is_root = unsafe { libc::geteuid() == 0 };
        assert_eq!(needs_builder(&spec, None, false), !is_root);
    }

    #[test]
    fn omnios_on_linux_needs_builder() {
        let spec = make_spec(Some("omnios"), &[("img", "artifact")]);
        let is_illumos = cfg!(target_os = "illumos");
        assert_eq!(needs_builder(&spec, None, false), !is_illumos);
    }

    #[test]
    fn target_filter_respected() {
        let spec = make_spec(
            Some("ubuntu-22.04"),
            &[("vm", "qcow2"), ("img", "oci")],
        );
        // When targeting only "img" (OCI), no qcow2 → no builder needed for that reason
        let is_root = unsafe { libc::geteuid() == 0 };
        // Only the OmniOS check or root check matters; "img" is OCI so qcow2 check is false
        assert_eq!(needs_builder(&spec, Some("img"), false), false || !is_root && false);
        // Actually: has_qcow2 is false (target "img" is oci), distro is Ubuntu not OmniOS
        assert!(!needs_builder(&spec, Some("img"), false));
    }
}
