use std::path::Path;

use spec_parser::schema::Target;
use tracing::info;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// Build a QCOW2 VM image from the staged rootfs.
///
/// Pipeline:
/// 1. Create raw disk image of specified size
/// 2. Attach loopback device
/// 3. Create ZFS pool with spec properties
/// 4. Create boot environment structure (rpool/ROOT/be-1)
/// 5. Copy staging rootfs into mounted BE
/// 6. Install bootloader via chroot
/// 7. Set bootfs property
/// 8. Export pool, detach loopback
/// 9. Convert raw -> qcow2
pub async fn build_qcow2_zfs(
    target: &Target,
    staging_root: &Path,
    output_dir: &Path,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    let disk_size = target
        .disk_size
        .as_deref()
        .ok_or(ForgeError::MissingDiskSize)?;

    let bootloader_type = target.bootloader.as_deref().unwrap_or("uefi");

    let raw_path = output_dir.join(format!("{}.raw", target.name));
    let qcow2_path = output_dir.join(format!("{}.qcow2", target.name));
    let raw_str = raw_path.to_str().unwrap();
    let qcow2_str = qcow2_path.to_str().unwrap();

    // Collect pool properties
    let pool_props: Vec<(&str, &str)> = target
        .pool
        .as_ref()
        .map(|p| {
            p.properties
                .iter()
                .map(|prop| (prop.name.as_str(), prop.value.as_str()))
                .collect()
        })
        .unwrap_or_default();

    let pool_name = "rpool";
    let be_dataset = format!("{pool_name}/ROOT/be-1");

    info!(disk_size, "Step 1: Creating raw disk image");
    crate::tools::qemu_img::create_raw(runner, raw_str, disk_size).await?;

    info!("Step 2: Attaching loopback device");
    let device = crate::tools::loopback::attach(runner, raw_str).await?;

    // Wrap the rest in a closure-like structure so we can clean up on error
    let result = async {
        info!(device = %device, "Step 3: Creating ZFS pool");
        crate::tools::zpool::create(runner, pool_name, &device, &pool_props).await?;

        info!("Step 4: Creating boot environment structure");
        crate::tools::zfs::create(
            runner,
            &format!("{pool_name}/ROOT"),
            &[("canmount", "off"), ("mountpoint", "legacy")],
        )
        .await?;

        let staging_str = staging_root.to_str().unwrap_or(".");
        crate::tools::zfs::create(
            runner,
            &be_dataset,
            &[("canmount", "noauto"), ("mountpoint", staging_str)],
        )
        .await?;

        crate::tools::zfs::mount(runner, &be_dataset).await?;

        info!("Step 5: Copying staging rootfs into boot environment");
        copy_rootfs(staging_root, staging_root)?;

        info!("Step 6: Installing bootloader");
        crate::tools::bootloader::install(runner, staging_str, pool_name, bootloader_type).await?;

        info!("Step 7: Setting bootfs property");
        crate::tools::zpool::set(runner, pool_name, "bootfs", &be_dataset).await?;

        info!("Step 8: Exporting ZFS pool");
        crate::tools::zfs::unmount(runner, &be_dataset).await?;
        crate::tools::zpool::export(runner, pool_name).await?;

        Ok::<(), ForgeError>(())
    }
    .await;

    // Always try to detach loopback, even on error
    info!("Detaching loopback device");
    let detach_result = crate::tools::loopback::detach(runner, &device).await;

    // Return the original error if there was one
    result?;
    detach_result?;

    info!("Step 9: Converting raw -> qcow2");
    crate::tools::qemu_img::convert_to_qcow2(runner, raw_str, qcow2_str).await?;

    // Clean up raw file
    std::fs::remove_file(&raw_path).ok();

    info!(path = %qcow2_path.display(), "QCOW2 image created");
    Ok(())
}

/// Copy the staging rootfs into the mounted BE.
/// Since the BE is mounted at the staging root mountpoint, we use a recursive
/// copy approach for files that need relocation.
fn copy_rootfs(src: &Path, dest: &Path) -> Result<(), ForgeError> {
    // In the actual build, the ZFS dataset is mounted at the staging_root path,
    // so the files are already in place after package installation. This function
    // handles the case where we need to copy from a temp staging dir into the
    // mounted ZFS dataset.
    if src == dest {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(src).follow_links(false) {
        let entry = entry.map_err(|e| ForgeError::Qcow2Build {
            step: "copy_rootfs".to_string(),
            detail: e.to_string(),
        })?;

        let rel = entry.path().strip_prefix(src).unwrap_or(entry.path());
        let target = dest.join(rel);

        if entry.path().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if entry.path().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }

    Ok(())
}
