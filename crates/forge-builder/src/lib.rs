pub mod binary;
pub mod config;
pub mod detect;
pub mod error;
pub mod lifecycle;
pub mod transfer;

use std::io::{stderr, stdout};
use std::path::Path;

use spec_parser::schema::{DistroFamily, ImageSpec};
use tracing::info;

use crate::config::BuilderConfig;
use crate::error::BuilderError;

/// Run a forger build inside a builder VM.
///
/// This is the top-level orchestrator that:
/// 1. Resolves builder VM configuration from the spec
/// 2. Resolves the correct forger binary for the target OS
/// 3. Spins up an ephemeral builder VM
/// 4. Uploads inputs (binary, spec, files)
/// 5. Runs the build via SSH
/// 6. Downloads output artifacts
/// 7. Tears down the VM (always, even on error)
pub async fn run_in_builder(
    spec: &ImageSpec,
    spec_path: &Path,
    files_dir: &Path,
    output_dir: &Path,
    target: Option<&str>,
    profiles: &[String],
) -> Result<(), BuilderError> {
    let distro = DistroFamily::from_distro_str(spec.distro.as_deref());
    let config = BuilderConfig::resolve(spec.builder.as_ref(), &distro);
    let binary = binary::resolve_forger_binary(&distro).await?;

    info!("Starting builder VM for remote build");
    let session = lifecycle::BuilderSession::start(&config).await?;

    let result = run_build_in_session(&session, &binary.path, spec_path, files_dir, output_dir, target, profiles).await;

    // Always teardown, even on error
    info!("Tearing down builder VM");
    if let Err(e) = session.teardown().await {
        tracing::warn!(error = %e, "Builder VM teardown failed (build result preserved)");
    }

    result
}

async fn run_build_in_session(
    session: &lifecycle::BuilderSession,
    binary_path: &Path,
    spec_path: &Path,
    files_dir: &Path,
    output_dir: &Path,
    target: Option<&str>,
    profiles: &[String],
) -> Result<(), BuilderError> {
    // Upload inputs
    transfer::upload_build_inputs(session, binary_path, spec_path, files_dir)?;

    // Build the remote command
    let mut cmd = String::from(
        "sudo /tmp/forger-build/forger build -s /tmp/forger-build/spec.kdl -o /tmp/forger-build/output/ --local",
    );

    if let Some(t) = target {
        cmd.push_str(&format!(" -t {t}"));
    }

    for p in profiles {
        cmd.push_str(&format!(" -p {p}"));
    }

    info!(cmd = %cmd, "Running build in builder VM");

    // Stream output to the user's terminal
    let (_, _, exit_code) =
        vm_manager::ssh::exec_streaming(&session.ssh_session, &cmd, stdout(), stderr())
            .map_err(|e| BuilderError::TransferFailed {
                detail: format!("remote exec: {e}"),
            })?;

    if exit_code != 0 {
        return Err(BuilderError::RemoteBuildFailed { exit_code });
    }

    // Download artifacts
    transfer::download_artifacts(session, output_dir)?;

    info!(output = %output_dir.display(), "Build artifacts downloaded successfully");
    Ok(())
}
