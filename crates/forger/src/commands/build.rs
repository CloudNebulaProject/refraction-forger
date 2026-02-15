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
    local: bool,
    use_builder: bool,
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

    // Check if we need a builder VM
    #[cfg(feature = "builder")]
    {
        let needs = forge_builder::detect::needs_builder(&filtered, target, local);
        if needs || use_builder {
            info!("Delegating build to builder VM");
            forge_builder::run_in_builder(
                &filtered,
                spec_path,
                &files_dir,
                output_dir,
                target,
                profiles,
            )
            .await
            .map_err(miette::Report::new)
            .wrap_err("Builder VM build failed")?;

            println!("Build complete. Output: {}", output_dir.display());
            return Ok(());
        }
    }

    // Suppress unused variable warnings when builder feature is disabled
    #[cfg(not(feature = "builder"))]
    {
        let _ = (local, use_builder);
    }

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
