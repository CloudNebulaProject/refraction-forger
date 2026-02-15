use std::path::PathBuf;

use forge_engine::tools::SystemToolRunner;
use forge_engine::BuildContext;
use miette::{Context, IntoDiagnostic};
use tracing::info;

/// Build an image from a spec file.
pub async fn run(
    spec_path: &PathBuf,
    target: Option<&str>,
    profiles: &[String],
    output_dir: &PathBuf,
) -> miette::Result<()> {
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

    // Determine files directory (images/files/ relative to spec)
    let files_dir = spec_dir.join("files");

    let runner = SystemToolRunner;

    let ctx = BuildContext {
        spec: &filtered,
        files_dir: &files_dir,
        output_dir,
        runner: &runner,
    };

    info!(
        spec = %spec_path.display(),
        output = %output_dir.display(),
        "Starting build"
    );

    ctx.build(target)
        .await
        .map_err(miette::Report::new)
        .wrap_err("Build failed")?;

    println!("Build complete. Output: {}", output_dir.display());
    Ok(())
}
