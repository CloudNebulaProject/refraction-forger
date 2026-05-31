use std::path::Path;

use spec_parser::schema::Target;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// A prepared QCOW2 disk image, ready for Phase 1 population.
#[derive(Debug)]
pub enum PreparedQcow2 {
    Ext4(super::qcow2_ext4::PreparedExt4),
    Zfs(super::qcow2_zfs::PreparedZfs),
}

impl PreparedQcow2 {
    /// The path where the target filesystem is mounted; Phase 1 populates here.
    pub fn root_mount(&self) -> &Path {
        match self {
            PreparedQcow2::Ext4(p) => p.root_mount(),
            PreparedQcow2::Zfs(p) => p.root_mount(),
        }
    }
}

/// Phase 2 prepare: create disk image, partition/format, mount target filesystem.
///
/// Dispatches to ext4 or ZFS based on `target.filesystem`.
pub async fn prepare_qcow2(
    target: &Target,
    output_dir: &Path,
    runner: &dyn ToolRunner,
    output_name: Option<&str>,
) -> Result<PreparedQcow2, ForgeError> {
    match target.filesystem.as_deref().unwrap_or("zfs") {
        "zfs" => {
            let prepared =
                super::qcow2_zfs::prepare_zfs(target, output_dir, runner, output_name).await?;
            Ok(PreparedQcow2::Zfs(prepared))
        }
        "ext4" => {
            let prepared =
                super::qcow2_ext4::prepare_ext4(target, output_dir, runner, output_name).await?;
            Ok(PreparedQcow2::Ext4(prepared))
        }
        other => Err(ForgeError::UnsupportedFilesystem {
            fs_type: other.to_string(),
            target: target.name.clone(),
        }),
    }
}

/// Phase 2 finalize: install bootloader, unmount.
pub async fn finalize_qcow2(
    prepared: &PreparedQcow2,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    match prepared {
        PreparedQcow2::Ext4(p) => super::qcow2_ext4::finalize_ext4(p, runner).await,
        PreparedQcow2::Zfs(p) => super::qcow2_zfs::finalize_zfs(p, runner).await,
    }
}

/// Cleanup: detach loopback, convert raw→qcow2 (if `convert`), remove raw file.
///
/// Always call this, even if earlier phases failed.
pub async fn cleanup_qcow2(
    prepared: PreparedQcow2,
    convert: bool,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    match prepared {
        PreparedQcow2::Ext4(p) => super::qcow2_ext4::cleanup_ext4(p, convert, runner).await,
        PreparedQcow2::Zfs(p) => super::qcow2_zfs::cleanup_zfs(p, convert, runner).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spec_parser::schema::{Target, TargetKind};

    fn make_target(fs: Option<&str>) -> Target {
        Target {
            name: "test".to_string(),
            kind: TargetKind::Qcow2,
            disk_size: Some("2G".to_string()),
            bootloader: Some("uefi".to_string()),
            filesystem: fs.map(|s| s.to_string()),
            push_to: None,
            entrypoint: None,
            environment: None,
            pool: None,
        }
    }

    #[test]
    fn test_unsupported_filesystem_error() {
        let target = make_target(Some("btrfs"));
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let tmpdir = tempfile::tempdir().unwrap();

            use crate::tools::{ToolOutput, ToolRunner};
            use std::future::Future;
            use std::pin::Pin;

            struct FailRunner;
            impl ToolRunner for FailRunner {
                fn run<'a>(
                    &'a self,
                    _program: &'a str,
                    _args: &'a [&'a str],
                ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ForgeError>> + Send + 'a>>
                {
                    Box::pin(async {
                        Err(ForgeError::Qcow2Build {
                            step: "test".to_string(),
                            detail: "not expected to be called".to_string(),
                        })
                    })
                }
            }

            prepare_qcow2(&target, tmpdir.path(), &FailRunner, None).await
        });

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ForgeError::UnsupportedFilesystem { .. }));
    }

    #[test]
    fn test_default_filesystem_is_zfs() {
        let target = make_target(None);
        assert_eq!(target.filesystem.as_deref().unwrap_or("zfs"), "zfs");
    }
}
