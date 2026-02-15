use std::path::PathBuf;

use miette::{Context, IntoDiagnostic};

/// Inspect a spec file: parse, resolve includes, apply profiles, and print the result.
pub fn run(spec_path: &PathBuf, profiles: &[String]) -> miette::Result<()> {
    let kdl_content = std::fs::read_to_string(spec_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read spec file: {}", spec_path.display()))?;

    let spec = spec_parser::parse(&kdl_content)
        .map_err(miette::Report::new)
        .wrap_err("Failed to parse spec")?;

    let spec_dir = spec_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    let resolved = spec_parser::resolve::resolve(spec, spec_dir)
        .map_err(miette::Report::new)
        .wrap_err("Failed to resolve includes")?;

    let filtered = spec_parser::profile::apply_profiles(resolved, profiles);

    println!("Image: {} v{}", filtered.metadata.name, filtered.metadata.version);

    if let Some(ref desc) = filtered.metadata.description {
        println!("Description: {desc}");
    }

    if let Some(ref distro) = filtered.distro {
        println!("Distro: {distro}");
    }

    println!("\nRepositories:");
    for pub_entry in &filtered.repositories.publishers {
        println!("  {} -> {}", pub_entry.name, pub_entry.origin);
    }
    for mirror in &filtered.repositories.apt_mirrors {
        print!("  apt-mirror: {} suite={}", mirror.url, mirror.suite);
        if let Some(ref components) = mirror.components {
            print!(" components={components}");
        }
        println!();
    }

    if let Some(ref inc) = filtered.incorporation {
        println!("\nIncorporation: {inc}");
    }

    if let Some(ref variants) = filtered.variants {
        println!("\nVariants:");
        for var in &variants.vars {
            println!("  {} = {}", var.name, var.value);
        }
    }

    if let Some(ref certs) = filtered.certificates {
        println!("\nCertificates:");
        for ca in &certs.ca {
            println!("  CA: publisher={} certfile={}", ca.publisher, ca.certfile);
        }
    }

    let total_packages: usize = filtered.packages.iter().map(|pl| pl.packages.len()).sum();
    println!("\nPackages ({total_packages} total):");
    for pl in &filtered.packages {
        if let Some(ref cond) = pl.r#if {
            println!("  [if={cond}]");
        }
        for pkg in &pl.packages {
            println!("    {}", pkg.name);
        }
    }

    if !filtered.customizations.is_empty() {
        println!("\nCustomizations:");
        for c in &filtered.customizations {
            if let Some(ref cond) = c.r#if {
                println!("  [if={cond}]");
            }
            for user in &c.users {
                println!("    user: {}", user.name);
            }
        }
    }

    let total_overlays: usize = filtered.overlays.iter().map(|o| o.actions.len()).sum();
    println!("\nOverlays ({total_overlays} actions):");
    for overlay_block in &filtered.overlays {
        if let Some(ref cond) = overlay_block.r#if {
            println!("  [if={cond}]");
        }
        for action in &overlay_block.actions {
            match action {
                spec_parser::schema::OverlayAction::File(f) => {
                    println!(
                        "    file: {} <- {}",
                        f.destination,
                        f.source.as_deref().unwrap_or("(empty)")
                    );
                }
                spec_parser::schema::OverlayAction::Devfsadm(_) => {
                    println!("    devfsadm");
                }
                spec_parser::schema::OverlayAction::EnsureDir(d) => {
                    println!("    ensure-dir: {}", d.path);
                }
                spec_parser::schema::OverlayAction::RemoveFiles(r) => {
                    let target = r
                        .file
                        .as_deref()
                        .or(r.dir.as_deref())
                        .or(r.pattern.as_deref())
                        .unwrap_or("?");
                    println!("    remove: {target}");
                }
                spec_parser::schema::OverlayAction::EnsureSymlink(s) => {
                    println!("    symlink: {} -> {}", s.path, s.target);
                }
                spec_parser::schema::OverlayAction::Shadow(s) => {
                    println!("    shadow: {} (hash set)", s.username);
                }
            }
        }
    }

    if !filtered.targets.is_empty() {
        println!("\nTargets:");
        for target in &filtered.targets {
            print!("  {} ({})", target.name, target.kind);
            if let Some(ref size) = target.disk_size {
                print!(" disk_size={size}");
            }
            if let Some(ref bl) = target.bootloader {
                print!(" bootloader={bl}");
            }
            if let Some(ref fs) = target.filesystem {
                print!(" filesystem={fs}");
            }
            if let Some(ref push) = target.push_to {
                print!(" push-to={push}");
            }
            println!();
        }
    }

    Ok(())
}
