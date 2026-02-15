use std::collections::HashSet;
use std::path::{Path, PathBuf};

use miette::Diagnostic;
use thiserror::Error;

use crate::schema::ImageSpec;

#[derive(Debug, Error, Diagnostic)]
pub enum ResolveError {
    #[error("Failed to read spec file: {path}")]
    #[diagnostic(help("Ensure the file exists and is readable"))]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse included spec: {path}")]
    #[diagnostic(help("Check the KDL syntax in the included file"))]
    ParseInclude {
        path: String,
        #[source]
        source: crate::ParseError,
    },

    #[error("Circular include detected: {path}")]
    #[diagnostic(
        help("The include chain forms a cycle. Remove the circular reference."),
        code(spec_parser::circular_include)
    )]
    CircularInclude { path: String },

    #[error("Failed to resolve base spec: {path}")]
    #[diagnostic(help("Ensure the base spec path is correct and the file exists"))]
    ResolveBase {
        path: String,
        #[source]
        source: Box<ResolveError>,
    },
}

/// Resolve all includes and base references in an `ImageSpec`, producing a
/// fully merged spec. The `spec_dir` is the directory containing the root spec
/// file, used to resolve relative paths.
pub fn resolve(spec: ImageSpec, spec_dir: &Path) -> Result<ImageSpec, ResolveError> {
    let mut visited = HashSet::new();
    resolve_inner(spec, spec_dir, &mut visited)
}

fn resolve_inner(
    mut spec: ImageSpec,
    spec_dir: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<ImageSpec, ResolveError> {
    // Resolve base spec first (base is the "parent" we inherit from)
    if let Some(base_path) = spec.base.take() {
        let base_abs = resolve_path(spec_dir, &base_path);
        let canonical = base_abs
            .canonicalize()
            .map_err(|e| ResolveError::ReadFile {
                path: base_abs.display().to_string(),
                source: e,
            })?;

        if !visited.insert(canonical.clone()) {
            return Err(ResolveError::CircularInclude {
                path: canonical.display().to_string(),
            });
        }

        let base_content =
            std::fs::read_to_string(&canonical).map_err(|e| ResolveError::ReadFile {
                path: canonical.display().to_string(),
                source: e,
            })?;

        let base_spec = crate::parse(&base_content).map_err(|e| ResolveError::ParseInclude {
            path: canonical.display().to_string(),
            source: e,
        })?;

        let base_dir = canonical.parent().unwrap_or(spec_dir);
        let resolved_base = resolve_inner(base_spec, base_dir, visited).map_err(|e| {
            ResolveError::ResolveBase {
                path: canonical.display().to_string(),
                source: Box::new(e),
            }
        })?;

        spec = merge_base(resolved_base, spec);
    }

    // Resolve includes (siblings that contribute packages/overlays/customizations)
    let includes = std::mem::take(&mut spec.includes);
    for include in includes {
        let inc_abs = resolve_path(spec_dir, &include.path);
        let canonical = inc_abs
            .canonicalize()
            .map_err(|e| ResolveError::ReadFile {
                path: inc_abs.display().to_string(),
                source: e,
            })?;

        if !visited.insert(canonical.clone()) {
            return Err(ResolveError::CircularInclude {
                path: canonical.display().to_string(),
            });
        }

        let inc_content =
            std::fs::read_to_string(&canonical).map_err(|e| ResolveError::ReadFile {
                path: canonical.display().to_string(),
                source: e,
            })?;

        let inc_spec = crate::parse(&inc_content).map_err(|e| ResolveError::ParseInclude {
            path: canonical.display().to_string(),
            source: e,
        })?;

        let inc_dir = canonical.parent().unwrap_or(spec_dir);
        let resolved_inc = resolve_inner(inc_spec, inc_dir, visited)?;

        merge_include(&mut spec, resolved_inc);
    }

    Ok(spec)
}

/// Merge a base (parent) spec with the child. The child's values take
/// precedence; the base provides defaults.
fn merge_base(mut base: ImageSpec, child: ImageSpec) -> ImageSpec {
    // Metadata comes from the child
    base.metadata = child.metadata;

    // build_host: child overrides
    if child.build_host.is_some() {
        base.build_host = child.build_host;
    }

    // repositories: merge publishers from child into base (child publishers appended)
    for pub_entry in child.repositories.publishers {
        if !base
            .repositories
            .publishers
            .iter()
            .any(|p| p.name == pub_entry.name)
        {
            base.repositories.publishers.push(pub_entry);
        }
    }

    // incorporation: child overrides
    if child.incorporation.is_some() {
        base.incorporation = child.incorporation;
    }

    // variants: merge
    if let Some(child_variants) = child.variants {
        if let Some(ref mut base_variants) = base.variants {
            for var in child_variants.vars {
                if let Some(existing) = base_variants.vars.iter_mut().find(|v| v.name == var.name) {
                    existing.value = var.value;
                } else {
                    base_variants.vars.push(var);
                }
            }
        } else {
            base.variants = Some(child_variants);
        }
    }

    // certificates: merge
    if let Some(child_certs) = child.certificates {
        if let Some(ref mut base_certs) = base.certificates {
            base_certs.ca.extend(child_certs.ca);
        } else {
            base.certificates = Some(child_certs);
        }
    }

    // packages, customizations, overlays: child appended after base
    base.packages.extend(child.packages);
    base.customizations.extend(child.customizations);
    base.overlays.extend(child.overlays);

    // includes: already resolved, don't carry forward
    base.includes = Vec::new();

    // targets: child's targets replace base entirely
    if !child.targets.is_empty() {
        base.targets = child.targets;
    }

    base
}

/// Merge an included spec into the current spec. Includes contribute
/// packages, customizations, and overlays but not metadata/targets.
fn merge_include(spec: &mut ImageSpec, included: ImageSpec) {
    spec.packages.extend(included.packages);
    spec.customizations.extend(included.customizations);
    spec.overlays.extend(included.overlays);
}

fn resolve_path(base_dir: &Path, relative: &str) -> PathBuf {
    let path = Path::new(relative);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_with_include() {
        let tmp = TempDir::new().unwrap();

        let included_kdl = r#"
            metadata name="included" version="0.0.1"
            repositories {
                publisher name="extra" origin="http://extra.example.com"
            }
            packages {
                package "extra/pkg"
            }
            overlays {
                ensure-dir "/extra/dir" owner="root" group="root" mode="755"
            }
        "#;
        fs::write(tmp.path().join("included.kdl"), included_kdl).unwrap();

        let root_kdl = r#"
            metadata name="root" version="1.0.0"
            repositories {
                publisher name="main" origin="http://main.example.com"
            }
            include "included.kdl"
            packages {
                package "main/pkg"
            }
        "#;

        let spec = crate::parse(root_kdl).unwrap();
        let resolved = resolve(spec, tmp.path()).unwrap();

        assert_eq!(resolved.metadata.name, "root");
        assert_eq!(resolved.packages.len(), 2);
        assert_eq!(resolved.overlays.len(), 1);
    }

    #[test]
    fn test_resolve_with_base() {
        let tmp = TempDir::new().unwrap();

        let base_kdl = r#"
            metadata name="base" version="0.0.1"
            repositories {
                publisher name="core" origin="http://core.example.com"
            }
            packages {
                package "base/pkg"
            }
        "#;
        fs::write(tmp.path().join("base.kdl"), base_kdl).unwrap();

        let child_kdl = r#"
            metadata name="child" version="1.0.0"
            base "base.kdl"
            repositories {
                publisher name="extra" origin="http://extra.example.com"
            }
            packages {
                package "child/pkg"
            }
            target "vm" kind="qcow2" {
                disk-size "10G"
            }
        "#;

        let spec = crate::parse(child_kdl).unwrap();
        let resolved = resolve(spec, tmp.path()).unwrap();

        assert_eq!(resolved.metadata.name, "child");
        assert_eq!(resolved.repositories.publishers.len(), 2);
        assert_eq!(resolved.packages.len(), 2);
        assert_eq!(resolved.packages[0].packages[0].name, "base/pkg");
        assert_eq!(resolved.packages[1].packages[0].name, "child/pkg");
        assert_eq!(resolved.targets.len(), 1);
    }

    #[test]
    fn test_circular_include_detected() {
        let tmp = TempDir::new().unwrap();

        let a_kdl = r#"
            metadata name="a" version="0.0.1"
            repositories {}
            include "b.kdl"
        "#;
        let b_kdl = r#"
            metadata name="b" version="0.0.1"
            repositories {}
            include "a.kdl"
        "#;
        fs::write(tmp.path().join("a.kdl"), a_kdl).unwrap();
        fs::write(tmp.path().join("b.kdl"), b_kdl).unwrap();

        let spec = crate::parse(a_kdl).unwrap();
        let result = resolve(spec, tmp.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ResolveError::CircularInclude { .. }));
    }
}
