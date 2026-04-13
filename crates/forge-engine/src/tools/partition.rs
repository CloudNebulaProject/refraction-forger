use crate::error::ForgeError;
use crate::tools::ToolRunner;
use tracing::info;

/// Partition layout result for a dual BIOS+UEFI GPT disk.
pub struct GptPartitions {
    /// BIOS boot partition (1MB, type ef02) — holds GRUB i386-pc stage
    pub bios_part: String,
    /// EFI System Partition (512MB, type EF00) — holds GRUB x86_64-efi
    pub efi_part: String,
    /// Root partition (remainder, type 8300)
    pub root_part: String,
}

/// Create a GPT partition table with BIOS boot, EFI, and root partitions.
///
/// This layout supports both legacy BIOS and UEFI boot:
/// - Partition 1: 1MB BIOS boot (ef02) for `grub-install --target=i386-pc`
/// - Partition 2: 512MB EFI System Partition for `grub-install --target=x86_64-efi`
/// - Partition 3: Root filesystem (remainder)
///
/// Assumes the device is a loopback device like `/dev/loopN`.
pub async fn create_gpt_efi_root(
    runner: &dyn ToolRunner,
    device: &str,
) -> Result<GptPartitions, ForgeError> {
    info!(device, "Creating GPT partition table with BIOS boot + EFI + root");

    // Zap any existing partition table
    runner.run("sgdisk", &["--zap-all", device]).await?;

    // Create three partitions:
    // 1: 1MB BIOS boot partition (ef02) — required for GRUB i386-pc on GPT
    // 2: 512MB EFI System Partition (EF00)
    // 3: Root partition (remainder, type 8300)
    runner
        .run(
            "sgdisk",
            &[
                "-n", "1:0:+1M",
                "-t", "1:EF02",
                "-n", "2:0:+512M",
                "-t", "2:EF00",
                "-n", "3:0:0",
                "-t", "3:8300",
                device,
            ],
        )
        .await?;

    Ok(GptPartitions {
        bios_part: format!("{device}p1"),
        efi_part: format!("{device}p2"),
        root_part: format!("{device}p3"),
    })
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
        let parts = create_gpt_efi_root(&runner, "/dev/loop0").await.unwrap();

        assert_eq!(parts.bios_part, "/dev/loop0p1");
        assert_eq!(parts.efi_part, "/dev/loop0p2");
        assert_eq!(parts.root_part, "/dev/loop0p3");

        let calls = runner.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "sgdisk");
        assert_eq!(calls[0].1, vec!["--zap-all", "/dev/loop0"]);
        assert_eq!(calls[1].0, "sgdisk");
        // BIOS boot partition
        assert!(calls[1].1.contains(&"1:0:+1M".to_string()));
        assert!(calls[1].1.contains(&"1:EF02".to_string()));
        // EFI System Partition
        assert!(calls[1].1.contains(&"2:0:+512M".to_string()));
        assert!(calls[1].1.contains(&"2:EF00".to_string()));
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
