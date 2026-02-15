// thiserror/miette derive macros generate code that triggers false-positive unused_assignments
#![allow(unused_assignments)]

pub mod error;
pub mod phase1;
pub mod phase2;
pub mod tools;

use std::path::Path;

use error::ForgeError;
use spec_parser::schema::{ImageSpec, Target, TargetKind};
use tools::ToolRunner;
use tracing::info;

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
}

impl<'a> BuildContext<'a> {
    /// Build a specific target by name, or all targets if name is None.
    pub async fn build(&self, target_name: Option<&str>) -> Result<(), ForgeError> {
        let targets = self.select_targets(target_name)?;

        std::fs::create_dir_all(self.output_dir)?;

        for target in targets {
            info!(target = %target.name, kind = %target.kind, "Building target");

            match target.kind {
                TargetKind::Qcow2 => {
                    self.build_qcow2(target).await?;
                }
                _ => {
                    // OCI/Artifact: Phase 1 creates its own staging dir, Phase 2 packs from it
                    let phase1_result =
                        phase1::execute(self.spec, self.files_dir, self.runner).await?;

                    phase2::execute(
                        target,
                        &phase1_result.staging_root,
                        self.files_dir,
                        self.output_dir,
                    )
                    .await?;
                }
            }

            info!(target = %target.name, "Target built successfully");
        }

        Ok(())
    }

    /// QCOW2 build: prepare disk → populate rootfs directly → finalize → cleanup.
    async fn build_qcow2(&self, target: &Target) -> Result<(), ForgeError> {
        // Phase 2 prepare: create disk image, partition/format, mount target filesystem
        let prepared =
            phase2::qcow2::prepare_qcow2(target, self.output_dir, self.runner).await?;

        // Phase 1: populate directly into the mounted target filesystem
        let phase1_result =
            phase1::execute_into(self.spec, self.files_dir, self.runner, prepared.root_mount())
                .await;

        // Phase 2 finalize: install bootloader, unmount (only if Phase 1 succeeded)
        let finalize_result = if phase1_result.is_ok() {
            phase2::qcow2::finalize_qcow2(&prepared, self.runner).await
        } else {
            Ok(())
        };

        // Cleanup always runs: detach loopback, convert if everything succeeded
        let convert = phase1_result.is_ok() && finalize_result.is_ok();
        let cleanup_result =
            phase2::qcow2::cleanup_qcow2(prepared, convert, self.runner).await;

        // Propagate errors in order of occurrence
        phase1_result?;
        finalize_result?;
        cleanup_result?;

        // Auto-push to OCI registry if configured
        phase2::push_qcow2_if_configured(target, self.output_dir).await?;

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
