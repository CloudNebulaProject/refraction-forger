use std::path::PathBuf;

use miette::{Context, IntoDiagnostic};

/// List available targets from a spec file.
pub fn run(spec_path: &PathBuf) -> miette::Result<()> {
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

    let targets = forge_engine::list_targets(&resolved);

    if targets.is_empty() {
        println!("No targets defined in spec.");
        return Ok(());
    }

    println!("Available targets:");
    for (name, kind) in targets {
        println!("  {name} ({kind})");
    }

    Ok(())
}
