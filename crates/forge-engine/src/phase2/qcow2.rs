use std::path::Path;

use spec_parser::schema::Target;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// Build a QCOW2 VM image, dispatching to the appropriate filesystem backend.
///
/// - `"zfs"` (default): ZFS pool with boot environment
/// - `"ext4"`: GPT+EFI+ext4 with GRUB bootloader
pub async fn build_qcow2(
    target: &Target,
    staging_root: &Path,
    output_dir: &Path,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    match target.filesystem.as_deref().unwrap_or("zfs") {
        "zfs" => {
            super::qcow2_zfs::build_qcow2_zfs(target, staging_root, output_dir, runner).await
        }
        "ext4" => {
            super::qcow2_ext4::build_qcow2_ext4(target, staging_root, output_dir, runner).await
        }
        other => Err(ForgeError::UnsupportedFilesystem {
            fs_type: other.to_string(),
            target: target.name.clone(),
        }),
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
            // We can't actually run the build, but we can test the dispatcher logic
            // by checking the error for an unsupported filesystem
            let tmpdir = tempfile::tempdir().unwrap();
            let staging = tempfile::tempdir().unwrap();

            // Create a mock runner that always succeeds
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

            build_qcow2(&target, staging.path(), tmpdir.path(), &FailRunner).await
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
