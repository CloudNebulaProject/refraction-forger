pub mod binary;
pub mod config;
pub mod detect;
pub mod error;
pub mod lifecycle;
pub mod push;
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
    builder_image: Option<&str>,
    skip_push: bool,
) -> Result<(), BuilderError> {
    let distro = DistroFamily::from_distro_str(spec.distro.as_deref());
    let mut config = BuilderConfig::resolve(spec.builder.as_ref(), &distro);

    // CLI --builder-image overrides the spec/default image
    if let Some(img) = builder_image {
        config.image = img.to_string();
    }

    let binary = binary::resolve_forger_binary(&distro).await?;

    info!("Starting builder VM for remote build");
    let session = lifecycle::BuilderSession::start(&config).await?;

    let result = run_build_in_session(&session, spec, &binary.path, spec_path, files_dir, output_dir, target, profiles, skip_push).await;

    // On failure, try to collect diagnostic info before teardown
    if let Err(ref e) = result {
        tracing::error!(error = %e, "Remote build failed — collecting diagnostics");
        collect_diagnostics(&session);
    }

    // Always teardown, even on error
    info!("Tearing down builder VM");
    if let Err(e) = session.teardown().await {
        tracing::warn!(error = %e, "Builder VM teardown failed (build result preserved)");
    }

    result
}

/// Verify the builder VM has working network connectivity (DNS + HTTP).
fn verify_network(session: &lifecycle::BuilderSession) -> Result<(), BuilderError> {
    info!("Verifying network connectivity in builder VM");

    // Check DNS resolution and HTTP connectivity
    let check_cmd = "cat /etc/resolv.conf && echo '---' && \
                      nslookup archive.ubuntu.com 2>&1 || host archive.ubuntu.com 2>&1 || \
                      getent hosts archive.ubuntu.com 2>&1 || echo 'DNS_FAILED'";

    let (net_stdout, _, _) =
        vm_manager::ssh::exec_streaming(&session.ssh_session, check_cmd, stdout(), stderr())
            .map_err(|e| BuilderError::TransferFailed {
                detail: format!("network check: {e}"),
            })?;

    if net_stdout.contains("DNS_FAILED") {
        tracing::warn!("DNS resolution failed in builder VM — attempting to fix resolv.conf");

        // Try to fix DNS by writing resolv.conf with SLIRP DNS server
        let fix_cmd = "echo 'nameserver 10.0.2.3' | sudo tee /etc/resolv.conf";
        let (_, _, exit_code) =
            vm_manager::ssh::exec_streaming(&session.ssh_session, fix_cmd, stdout(), stderr())
                .map_err(|e| BuilderError::TransferFailed {
                    detail: format!("fix resolv.conf: {e}"),
                })?;

        if exit_code != 0 {
            return Err(BuilderError::TransferFailed {
                detail: "failed to configure DNS in builder VM".to_string(),
            });
        }

        // Verify DNS now works
        let (verify_out, _, _) = vm_manager::ssh::exec_streaming(
            &session.ssh_session,
            "getent hosts archive.ubuntu.com 2>&1 || echo 'STILL_FAILED'",
            stdout(),
            stderr(),
        )
        .map_err(|e| BuilderError::TransferFailed {
            detail: format!("DNS verify: {e}"),
        })?;

        if verify_out.contains("STILL_FAILED") {
            return Err(BuilderError::TransferFailed {
                detail: "DNS resolution still failing after fix — check VM networking".to_string(),
            });
        }
    }

    info!("Network connectivity verified");
    Ok(())
}

/// Install required build tools inside the builder VM based on distro.
fn install_build_deps(
    session: &lifecycle::BuilderSession,
    spec: &ImageSpec,
) -> Result<(), BuilderError> {
    let distro = DistroFamily::from_distro_str(spec.distro.as_deref());

    let install_cmd = match distro {
        DistroFamily::Ubuntu => {
            "sudo DEBIAN_FRONTEND=noninteractive apt-get update -qq && \
             sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
             debootstrap qemu-utils parted dosfstools e2fsprogs grub-efi-amd64-bin mount"
        }
        DistroFamily::OmniOS => {
            // OmniOS builder images should already have pkg tools; install qemu-img if missing
            "sudo pkg install -q system/qemu/img || true"
        }
    };

    info!("Installing build dependencies in builder VM");

    let (_, _, exit_code) =
        vm_manager::ssh::exec_streaming(&session.ssh_session, install_cmd, stdout(), stderr())
            .map_err(|e| BuilderError::TransferFailed {
                detail: format!("install build deps: {e}"),
            })?;

    if exit_code != 0 {
        return Err(BuilderError::TransferFailed {
            detail: format!("build dependency installation failed with exit code {exit_code}"),
        });
    }

    Ok(())
}

/// Collect diagnostic information from the builder VM after a failed build.
fn collect_diagnostics(session: &lifecycle::BuilderSession) {
    tracing::warn!("--- Builder VM Diagnostics ---");

    // Try to find and print debootstrap log (debootstrap writes errors here, not stderr)
    let diag_cmd = "echo '=== debootstrap log ===' && \
                    sudo find /tmp -name 'debootstrap.log' -exec cat {} \\; 2>/dev/null && \
                    echo '=== resolv.conf ===' && \
                    cat /etc/resolv.conf 2>/dev/null && \
                    echo '=== disk space ===' && \
                    df -h / /tmp 2>/dev/null";

    match vm_manager::ssh::exec_streaming(
        &session.ssh_session,
        diag_cmd,
        stdout(),
        stderr(),
    ) {
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "Failed to collect diagnostics"),
    }

    tracing::warn!("--- End Diagnostics ---");
}

async fn run_build_in_session(
    session: &lifecycle::BuilderSession,
    spec: &ImageSpec,
    binary_path: &Path,
    spec_path: &Path,
    files_dir: &Path,
    output_dir: &Path,
    target: Option<&str>,
    profiles: &[String],
    skip_push: bool,
) -> Result<(), BuilderError> {
    // Verify network connectivity (DNS) before doing anything
    verify_network(session)?;

    // Install build dependencies in the builder VM
    install_build_deps(session, spec)?;

    // Upload inputs
    transfer::upload_build_inputs(session, binary_path, spec_path, files_dir)?;

    // Build the remote command — always pass --skip-push so the VM never attempts
    // to push to the registry (it lacks GITHUB_TOKEN); the host handles pushing.
    let mut cmd = String::from(
        "sudo /tmp/forger-build/forger build -s /tmp/forger-build/spec.kdl -o /tmp/forger-build/output/ --local --skip-push",
    );

    if let Some(t) = target {
        cmd.push_str(&format!(" -t {t}"));
    }

    for p in profiles {
        cmd.push_str(&format!(" -p {p}"));
    }

    info!(cmd = %cmd, "Running build in builder VM");

    // Stream output to the user's terminal
    let (build_stdout, build_stderr, exit_code) =
        vm_manager::ssh::exec_streaming(&session.ssh_session, &cmd, stdout(), stderr())
            .map_err(|e| BuilderError::TransferFailed {
                detail: format!("remote exec: {e}"),
            })?;

    if exit_code != 0 {
        // Include captured output in the error for better diagnostics
        let detail = if !build_stderr.is_empty() {
            build_stderr
        } else if !build_stdout.is_empty() {
            // debootstrap and some tools write to stdout, not stderr
            let lines: Vec<&str> = build_stdout.lines().rev().take(20).collect();
            lines.into_iter().rev().collect::<Vec<_>>().join("\n")
        } else {
            String::new()
        };
        return Err(BuilderError::RemoteBuildFailed { exit_code, detail });
    }

    // Download artifacts
    transfer::download_artifacts(session, output_dir)?;

    info!(output = %output_dir.display(), "Build artifacts downloaded successfully");

    // Host-side push: push QCOW2 outputs from the host where GITHUB_TOKEN is available
    if !skip_push {
        push::push_qcow2_outputs(spec, output_dir).await?;
    }

    Ok(())
}
