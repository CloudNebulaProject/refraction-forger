use crate::schema::ImageSpec;

/// Filter an `ImageSpec` so that only blocks matching the active profiles are
/// retained. Blocks with no `if` condition are always included. Blocks whose
/// `if` value matches one of the active profile names are included. All others
/// are removed.
pub fn apply_profiles(mut spec: ImageSpec, active_profiles: &[String]) -> ImageSpec {
    spec.packages
        .retain(|p| profile_matches(p.r#if.as_deref(), active_profiles));

    spec.customizations
        .retain(|c| profile_matches(c.r#if.as_deref(), active_profiles));

    spec.overlays
        .retain(|o| profile_matches(o.r#if.as_deref(), active_profiles));

    spec
}

fn profile_matches(condition: Option<&str>, active_profiles: &[String]) -> bool {
    match condition {
        None => true,
        Some(name) => active_profiles.iter().any(|p| p == name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn test_unconditional_blocks_always_included() {
        let kdl = r#"
            metadata name="test" version="1.0.0"
            repositories {
                publisher name="main" origin="http://example.com"
            }
            packages {
                package "always/included"
            }
            packages if="desktop" {
                package "desktop/only"
            }
        "#;

        let spec = parse(kdl).unwrap();
        let filtered = apply_profiles(spec, &[]);

        assert_eq!(filtered.packages.len(), 1);
        assert_eq!(filtered.packages[0].packages[0].name, "always/included");
    }

    #[test]
    fn test_matching_profile_included() {
        let kdl = r#"
            metadata name="test" version="1.0.0"
            repositories {
                publisher name="main" origin="http://example.com"
            }
            packages {
                package "always/included"
            }
            packages if="desktop" {
                package "desktop/only"
            }
            packages if="server" {
                package "server/only"
            }
        "#;

        let spec = parse(kdl).unwrap();
        let filtered = apply_profiles(spec, &["desktop".to_string()]);

        assert_eq!(filtered.packages.len(), 2);
        assert_eq!(filtered.packages[0].packages[0].name, "always/included");
        assert_eq!(filtered.packages[1].packages[0].name, "desktop/only");
    }

    #[test]
    fn test_overlays_and_customizations_filtered() {
        let kdl = r#"
            metadata name="test" version="1.0.0"
            repositories {
                publisher name="main" origin="http://example.com"
            }
            overlays {
                ensure-dir "/always" owner="root" group="root" mode="755"
            }
            overlays if="dev" {
                ensure-dir "/dev-only" owner="root" group="root" mode="755"
            }
            customization {
                user "admin"
            }
            customization if="dev" {
                user "developer"
            }
        "#;

        let spec = parse(kdl).unwrap();
        let filtered = apply_profiles(spec, &[]);

        assert_eq!(filtered.overlays.len(), 1);
        assert_eq!(filtered.customizations.len(), 1);
    }
}
