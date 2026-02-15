use crate::error::ForgeError;
use crate::tools::ToolRunner;
use tracing::info;

/// Create a GPT partition table with an EFI system partition and a root partition.
///
/// Returns the partition device paths as (efi_part, root_part).
/// Assumes the device is a loopback device like `/dev/loopN`.
pub async fn create_gpt_efi_root(
    runner: &dyn ToolRunner,
    device: &str,
) -> Result<(String, String), ForgeError> {
    info!(device, "Creating GPT partition table with EFI + root");

    // Zap any existing partition table
    runner.run("sgdisk", &["--zap-all", device]).await?;

    // Create EFI partition (512M, type EF00) and root partition (remainder, type 8300)
    runner
        .run(
            "sgdisk",
            &[
                "-n", "1:0:+512M",
                "-t", "1:EF00",
                "-n", "2:0:0",
                "-t", "2:8300",
                device,
            ],
        )
        .await?;

    let efi_part = format!("{device}p1");
    let root_part = format!("{device}p2");
    Ok((efi_part, root_part))
}

/// Format a partition as FAT32.
pub async fn mkfs_fat32(runner: &dyn ToolRunner, device: &str) -> Result<(), ForgeError> {
    info!(device, "Formatting as FAT32");
    runner.run("mkfs.fat", &["-F", "32", device]).await?;
    Ok(())
}

/// Format a partition as ext4.
pub async fn mkfs_ext4(runner: &dyn ToolRunner, device: &str) -> Result<(), ForgeError> {
    info!(device, "Formatting as ext4");
    runner.run("mkfs.ext4", &["-F", device]).await?;
    Ok(())
}

/// Mount a device at the given mountpoint.
pub async fn mount(
    runner: &dyn ToolRunner,
    device: &str,
    mountpoint: &str,
) -> Result<(), ForgeError> {
    info!(device, mountpoint, "Mounting");
    runner.run("mount", &[device, mountpoint]).await?;
    Ok(())
}

/// Unmount a mountpoint.
pub async fn umount(runner: &dyn ToolRunner, mountpoint: &str) -> Result<(), ForgeError> {
    info!(mountpoint, "Unmounting");
    runner.run("umount", &[mountpoint]).await?;
    Ok(())
}

/// Bind-mount a source path into the target.
pub async fn bind_mount(
    runner: &dyn ToolRunner,
    source: &str,
    target: &str,
) -> Result<(), ForgeError> {
    info!(source, target, "Bind-mounting");
    runner.run("mount", &["--bind", source, target]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ToolOutput, ToolRunner};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;

    struct MockToolRunner {
        calls: Mutex<Vec<(String, Vec<String>)>>,
    }

    impl MockToolRunner {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<(String, Vec<String>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl ToolRunner for MockToolRunner {
        fn run<'a>(
            &'a self,
            program: &'a str,
            args: &'a [&'a str],
        ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ForgeError>> + Send + 'a>> {
            self.calls.lock().unwrap().push((
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
            ));
            Box::pin(async {
                Ok(ToolOutput {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 0,
                })
            })
        }
    }

    #[tokio::test]
    async fn test_create_gpt_efi_root_args() {
        let runner = MockToolRunner::new();
        let (efi, root) = create_gpt_efi_root(&runner, "/dev/loop0").await.unwrap();

        assert_eq!(efi, "/dev/loop0p1");
        assert_eq!(root, "/dev/loop0p2");

        let calls = runner.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "sgdisk");
        assert_eq!(calls[0].1, vec!["--zap-all", "/dev/loop0"]);
        assert_eq!(calls[1].0, "sgdisk");
        assert!(calls[1].1.contains(&"-n".to_string()));
        assert!(calls[1].1.contains(&"1:0:+512M".to_string()));
    }

    #[tokio::test]
    async fn test_mkfs_ext4() {
        let runner = MockToolRunner::new();
        mkfs_ext4(&runner, "/dev/loop0p2").await.unwrap();

        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "mkfs.ext4");
        assert_eq!(calls[0].1, vec!["-F", "/dev/loop0p2"]);
    }
}
