use std::path::{Path, PathBuf};

use spec_parser::schema::Target;
use tracing::info;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// State for a prepared ext4 QCOW2 disk image, ready for Phase 1 population.
#[derive(Debug)]
pub struct PreparedExt4 {
    pub raw_path: PathBuf,
    pub qcow2_path: PathBuf,
    pub device: String,
    pub efi_part: String,
    pub root_part: String,
    pub mount_dir: tempfile::TempDir,
}

impl PreparedExt4 {
    /// The path where the root partition is mounted; Phase 1 populates into this.
    pub fn root_mount(&self) -> &Path {
        self.mount_dir.path()
    }
}

/// Phase 2 prepare: create raw disk, partition, format, and mount the root partition.
///
/// Returns a `PreparedExt4` whose `root_mount()` is the mounted ext4 root —
/// Phase 1 should populate the rootfs directly into that directory.
pub async fn prepare_ext4(
    target: &Target,
    output_dir: &Path,
    runner: &dyn ToolRunner,
) -> Result<PreparedExt4, ForgeError> {
    let disk_size = target
        .disk_size
        .as_deref()
        .ok_or(ForgeError::MissingDiskSize)?;

    let raw_path = output_dir.join(format!("{}.raw", target.name));
    let qcow2_path = output_dir.join(format!("{}.qcow2", target.name));
    let raw_str = raw_path.to_str().unwrap();

    info!(disk_size, "Step 1: Creating raw disk image");
    crate::tools::qemu_img::create_raw(runner, raw_str, disk_size).await?;

    info!("Step 2: Attaching loopback device");
    let device = crate::tools::loopback::attach(runner, raw_str).await?;
    let _ = crate::tools::loopback::partprobe(runner, &device).await;

    info!(device = %device, "Step 3: Creating GPT partition table");
    let (efi_part, root_part) =
        crate::tools::partition::create_gpt_efi_root(runner, &device).await?;

    crate::tools::loopback::partprobe(runner, &device).await?;

    info!("Step 4: Formatting partitions");
    crate::tools::partition::mkfs_fat32(runner, &efi_part).await?;
    crate::tools::partition::mkfs_ext4(runner, &root_part).await?;

    let mount_dir = tempfile::tempdir().map_err(ForgeError::StagingSetup)?;
    let mount_str = mount_dir.path().to_str().unwrap();

    info!("Step 5: Mounting root partition at {}", mount_str);
    crate::tools::partition::mount(runner, &root_part, mount_str).await?;

    Ok(PreparedExt4 {
        raw_path,
        qcow2_path,
        device,
        efi_part,
        root_part,
        mount_dir,
    })
}

/// Phase 2 finalize: mount EFI, bind-mount pseudofs, install GRUB, unmount everything.
pub async fn finalize_ext4(
    prepared: &PreparedExt4,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    let mount_str = prepared.mount_dir.path().to_str().unwrap();

    info!("Finalize step 1: Mounting EFI partition");
    let efi_mount = prepared.mount_dir.path().join("boot/efi");
    std::fs::create_dir_all(&efi_mount)?;
    let efi_mount_str = efi_mount.to_str().unwrap();
    crate::tools::partition::mount(runner, &prepared.efi_part, efi_mount_str).await?;

    info!("Finalize step 2: Bind-mounting /dev, /proc, /sys");
    let dev_mount = format!("{mount_str}/dev");
    let proc_mount = format!("{mount_str}/proc");
    let sys_mount = format!("{mount_str}/sys");
    std::fs::create_dir_all(&dev_mount)?;
    std::fs::create_dir_all(&proc_mount)?;
    std::fs::create_dir_all(&sys_mount)?;
    crate::tools::partition::bind_mount(runner, "/dev", &dev_mount).await?;
    crate::tools::partition::bind_mount(runner, "/proc", &proc_mount).await?;
    crate::tools::partition::bind_mount(runner, "/sys", &sys_mount).await?;

    info!("Finalize step 3: Installing GRUB bootloader");
    runner
        .run(
            "chroot",
            &[
                mount_str,
                "/usr/sbin/grub-install",
                "--target=x86_64-efi",
                "--efi-directory=/boot/efi",
                "--no-nvram",
            ],
        )
        .await?;

    info!("Finalize step 4: Generating GRUB config");
    runner
        .run(
            "chroot",
            &[mount_str, "/usr/sbin/grub-mkconfig", "-o", "/boot/grub/grub.cfg"],
        )
        .await?;

    info!("Finalize step 5: Unmounting");
    crate::tools::partition::umount(runner, &sys_mount).await?;
    crate::tools::partition::umount(runner, &proc_mount).await?;
    crate::tools::partition::umount(runner, &dev_mount).await?;
    crate::tools::partition::umount(runner, efi_mount_str).await?;
    crate::tools::partition::umount(runner, mount_str).await?;

    Ok(())
}

/// Cleanup: detach loopback, convert raw→qcow2 (if `convert` is true), remove raw file.
///
/// Always runs, even if earlier phases failed — the loopback device must be detached.
pub async fn cleanup_ext4(
    prepared: PreparedExt4,
    convert: bool,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    info!("Cleanup: detaching loopback device");
    let detach_result = crate::tools::loopback::detach(runner, &prepared.device).await;

    if convert {
        let raw_str = prepared.raw_path.to_str().unwrap();
        let qcow2_str = prepared.qcow2_path.to_str().unwrap();

        info!("Cleanup: converting raw -> qcow2");
        crate::tools::qemu_img::convert_to_qcow2(runner, raw_str, qcow2_str).await?;
    }

    // Clean up raw file
    std::fs::remove_file(&prepared.raw_path).ok();

    detach_result?;

    if convert {
        info!(path = %prepared.qcow2_path.display(), "QCOW2 (ext4) image created");
    }

    Ok(())
}
