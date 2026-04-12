// thiserror/miette derive macros generate code that triggers false-positive unused_assignments
#![allow(unused_assignments)]

pub mod error;
pub mod output;
pub mod phase1;
pub mod phase2;
pub mod tools;

use std::path::Path;

use error::ForgeError;
use output::OutputHandler;
use spec_parser::schema::{DistroFamily, ImageSpec, Target, TargetKind};
use tools::ToolRunner;

/// Context for running a build.
pub struct BuildContext<'a> {
    /// The resolved and profile-filtered image spec.
    pub spec: &'a ImageSpec,
    /// Directory containing overlay source files (images/files/).
    pub files_dir: &'a Path,
    /// Output directory for build artifacts.
    pub output_dir: &'a Path,
    /// Tool runner for executing external commands.
    pub runner: &'a dyn ToolRunner,
    /// Skip OCI registry push after build (host-side push handles it instead).
    pub skip_push: bool,
    /// Output handler for progress display.
    pub output: OutputHandler,
}

impl<'a> BuildContext<'a> {
    /// Build a specific target by name, or all targets if name is None.
    pub async fn build(&self, target_name: Option<&str>) -> Result<(), ForgeError> {
        let targets = self.select_targets(target_name)?;

        std::fs::create_dir_all(self.output_dir)?;

        for target in targets {
            self.output
                .phase_start(&format!("Building target '{}' ({})", target.name, target.kind));

            match target.kind {
                TargetKind::Qcow2 => {
                    self.build_qcow2(target).await?;
                }
                _ => {
                    self.output.phase_start("Phase 1: rootfs assembly");
                    let phase1_result =
                        phase1::execute(self.spec, self.files_dir, self.runner).await?;
                    self.output.phase_done("Phase 1: rootfs assembly");

                    let distro = DistroFamily::from_distro_str(self.spec.distro.as_deref());
                    self.output
                        .phase_start(&format!("Phase 2: {} target production", target.kind));
                    phase2::execute(
                        target,
                        &phase1_result.staging_root,
                        self.files_dir,
                        self.output_dir,
                        &distro,
                    )
                    .await?;
                    self.output
                        .phase_done(&format!("Phase 2: {} target production", target.kind));
                }
            }

            self.output
                .phase_done(&format!("Target '{}' built successfully", target.name));
        }

        Ok(())
    }

    /// QCOW2 build: prepare disk → populate rootfs directly → finalize → cleanup.
    async fn build_qcow2(&self, target: &Target) -> Result<(), ForgeError> {
        self.output.phase_start("Phase 2 prepare: disk image setup");
        let prepared =
            phase2::qcow2::prepare_qcow2(target, self.output_dir, self.runner).await?;
        self.output.phase_done("Phase 2 prepare: disk image setup");

        self.output.phase_start("Phase 1: rootfs assembly into target");
        let phase1_result =
            phase1::execute_into(self.spec, self.files_dir, self.runner, prepared.root_mount())
                .await;
        if phase1_result.is_ok() {
            self.output.phase_done("Phase 1: rootfs assembly into target");
        }

        self.output.phase_start("Phase 2 finalize: bootloader & unmount");
        let finalize_result = if phase1_result.is_ok() {
            phase2::qcow2::finalize_qcow2(&prepared, self.runner).await
        } else {
            Ok(())
        };
        if finalize_result.is_ok() {
            self.output.phase_done("Phase 2 finalize: bootloader & unmount");
        }

        self.output.phase_start("Cleanup: detach & convert");
        let convert = phase1_result.is_ok() && finalize_result.is_ok();
        let cleanup_result =
            phase2::qcow2::cleanup_qcow2(prepared, convert, self.runner).await;
        self.output.phase_done("Cleanup: detach & convert");

        // Propagate errors in order of occurrence
        phase1_result?;
        finalize_result?;
        cleanup_result?;

        // Auto-push to OCI registry if configured (skipped when host-side push handles it)
        if !self.skip_push {
            let distro = DistroFamily::from_distro_str(self.spec.distro.as_deref());
            phase2::push_qcow2_if_configured(
                target,
                self.output_dir,
                &distro,
                &self.spec.metadata.version,
            )
            .await?;
        }

        Ok(())
    }

    fn select_targets(&self, target_name: Option<&str>) -> Result<Vec<&Target>, ForgeError> {
        match target_name {
            Some(name) => {
                let target = self
                    .spec
                    .targets
                    .iter()
                    .find(|t| t.name == name)
                    .ok_or_else(|| {
                        let available = self
                            .spec
                            .targets
                            .iter()
                            .map(|t| t.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        ForgeError::TargetNotFound {
                            name: name.to_string(),
                            available,
                        }
                    })?;
                Ok(vec![target])
            }
            None => Ok(self.spec.targets.iter().collect()),
        }
    }
}

/// List available targets from a spec.
pub fn list_targets(spec: &ImageSpec) -> Vec<(&str, &TargetKind)> {
    spec.targets
        .iter()
        .map(|t| (t.name.as_str(), &t.kind))
        .collect()
}
