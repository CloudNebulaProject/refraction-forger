use std::path::PathBuf;

use miette::{Context, IntoDiagnostic};

/// Validate a spec file by parsing it and resolving all includes.
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

    let _resolved = spec_parser::resolve::resolve(spec, spec_dir)
        .map_err(miette::Report::new)
        .wrap_err("Failed to resolve includes")?;

    println!("Spec is valid: {}", spec_path.display());
    Ok(())
}
