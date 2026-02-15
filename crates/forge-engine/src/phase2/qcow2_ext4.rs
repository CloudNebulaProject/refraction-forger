use std::path::Path;

use spec_parser::schema::Target;
use tracing::info;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// Build a QCOW2 VM image from the staged rootfs using ext4+GPT+GRUB.
///
/// Pipeline:
/// 1. Create raw disk image of specified size
/// 2. Attach loopback device + partprobe
/// 3. Create GPT partition table (EFI + root)
/// 4. Format partitions (FAT32 for EFI, ext4 for root)
/// 5. Mount root, copy staging rootfs
/// 6. Mount EFI at /boot/efi
/// 7. Bind-mount /dev, /proc, /sys
/// 8. chroot grub-install
/// 9. chroot grub-mkconfig
/// 10. Unmount all, detach loopback
/// 11. Convert raw -> qcow2
pub async fn build_qcow2_ext4(
    target: &Target,
    staging_root: &Path,
    output_dir: &Path,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    let disk_size = target
        .disk_size
        .as_deref()
        .ok_or(ForgeError::MissingDiskSize)?;

    let raw_path = output_dir.join(format!("{}.raw", target.name));
    let qcow2_path = output_dir.join(format!("{}.qcow2", target.name));
    let raw_str = raw_path.to_str().unwrap();
    let qcow2_str = qcow2_path.to_str().unwrap();

    info!(disk_size, "Step 1: Creating raw disk image");
    crate::tools::qemu_img::create_raw(runner, raw_str, disk_size).await?;

    info!("Step 2: Attaching loopback device");
    let device = crate::tools::loopback::attach(runner, raw_str).await?;

    // Re-read partition table after attaching loopback
    let _ = crate::tools::loopback::partprobe(runner, &device).await;

    let result = async {
        info!(device = %device, "Step 3: Creating GPT partition table");
        let (efi_part, root_part) =
            crate::tools::partition::create_gpt_efi_root(runner, &device).await?;

        // Re-read partition table after creating partitions
        crate::tools::loopback::partprobe(runner, &device).await?;

        info!("Step 4: Formatting partitions");
        crate::tools::partition::mkfs_fat32(runner, &efi_part).await?;
        crate::tools::partition::mkfs_ext4(runner, &root_part).await?;

        // Create a temporary mountpoint for the root partition
        let mount_dir = tempfile::tempdir().map_err(ForgeError::StagingSetup)?;
        let mount_str = mount_dir.path().to_str().unwrap();

        info!("Step 5: Mounting root partition and copying rootfs");
        crate::tools::partition::mount(runner, &root_part, mount_str).await?;

        // Copy staging rootfs into mounted root
        copy_rootfs(staging_root, mount_dir.path(), runner).await?;

        info!("Step 6: Mounting EFI partition");
        let efi_mount = mount_dir.path().join("boot/efi");
        std::fs::create_dir_all(&efi_mount)?;
        let efi_mount_str = efi_mount.to_str().unwrap();
        crate::tools::partition::mount(runner, &efi_part, efi_mount_str).await?;

        info!("Step 7: Bind-mounting /dev, /proc, /sys");
        let dev_mount = format!("{mount_str}/dev");
        let proc_mount = format!("{mount_str}/proc");
        let sys_mount = format!("{mount_str}/sys");
        std::fs::create_dir_all(&dev_mount)?;
        std::fs::create_dir_all(&proc_mount)?;
        std::fs::create_dir_all(&sys_mount)?;
        crate::tools::partition::bind_mount(runner, "/dev", &dev_mount).await?;
        crate::tools::partition::bind_mount(runner, "/proc", &proc_mount).await?;
        crate::tools::partition::bind_mount(runner, "/sys", &sys_mount).await?;

        info!("Step 8: Installing GRUB bootloader");
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

        info!("Step 9: Generating GRUB config");
        runner
            .run(
                "chroot",
                &[mount_str, "/usr/sbin/grub-mkconfig", "-o", "/boot/grub/grub.cfg"],
            )
            .await?;

        info!("Step 10: Unmounting");
        // Unmount in reverse order: bind mounts, EFI, root
        crate::tools::partition::umount(runner, &sys_mount).await?;
        crate::tools::partition::umount(runner, &proc_mount).await?;
        crate::tools::partition::umount(runner, &dev_mount).await?;
        crate::tools::partition::umount(runner, efi_mount_str).await?;
        crate::tools::partition::umount(runner, mount_str).await?;

        Ok::<(), ForgeError>(())
    }
    .await;

    // Always try to detach loopback, even on error
    info!("Detaching loopback device");
    let detach_result = crate::tools::loopback::detach(runner, &device).await;

    result?;
    detach_result?;

    info!("Step 11: Converting raw -> qcow2");
    crate::tools::qemu_img::convert_to_qcow2(runner, raw_str, qcow2_str).await?;

    // Clean up raw file
    std::fs::remove_file(&raw_path).ok();

    info!(path = %qcow2_path.display(), "QCOW2 (ext4) image created");
    Ok(())
}

/// Copy the staging rootfs into the mounted root partition.
///
/// Uses `cp -a` (archive mode) to properly preserve symlinks, permissions,
/// ownership, timestamps, and special files. This is critical for modern
/// distros with merged /usr where /lib, /bin, /sbin are symlinks.
async fn copy_rootfs(
    src: &Path,
    dest: &Path,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    let src_str = format!("{}/.", src.display());
    let dest_str = dest.to_str().unwrap();

    runner
        .run("cp", &["-a", &src_str, dest_str])
        .await
        .map_err(|_| ForgeError::Qcow2Build {
            step: "copy_rootfs".to_string(),
            detail: format!("cp -a {}/. -> {}", src.display(), dest.display()),
        })?;

    Ok(())
}
