use std::path::{Path, PathBuf};

use spec_parser::schema::Target;
use tracing::info;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// State for a prepared ZFS QCOW2 disk image, ready for Phase 1 population.
#[derive(Debug)]
pub struct PreparedZfs {
    pub raw_path: PathBuf,
    pub qcow2_path: PathBuf,
    /// Loopback device for the raw disk (e.g., /dev/lofi/1 or /dev/loop0).
    pub device: String,
    /// EFI System Partition device (e.g., /dev/lofi/1p2 or /dev/loop0p2).
    pub efi_part: String,
    /// ZFS partition device (e.g., /dev/lofi/1p3 or /dev/loop0p3).
    pub zfs_part: String,
    /// Build-time pool name (unique to avoid collision with host's rpool).
    pub pool_name: String,
    /// Final pool name for the output image (typically "rpool").
    pub final_pool_name: String,
    pub be_dataset: String,
    pub bootloader_type: String,
    pub mount_dir: tempfile::TempDir,
}

impl PreparedZfs {
    /// The path where the ZFS boot-environment dataset is mounted;
    /// Phase 1 populates into this.
    pub fn root_mount(&self) -> &Path {
        self.mount_dir.path()
    }
}

/// Phase 2 prepare: create raw disk, attach loopback, create ZFS pool + BE, mount.
///
/// Returns a `PreparedZfs` whose `root_mount()` is the mounted BE dataset —
/// Phase 1 should populate the rootfs directly into that directory.
pub async fn prepare_zfs(
    target: &Target,
    output_dir: &Path,
    runner: &dyn ToolRunner,
) -> Result<PreparedZfs, ForgeError> {
    let disk_size = target
        .disk_size
        .as_deref()
        .ok_or(ForgeError::MissingDiskSize)?;

    let bootloader_type = target.bootloader.as_deref().unwrap_or("uefi").to_string();

    let raw_path = output_dir.join(format!("{}.raw", target.name));
    let qcow2_path = output_dir.join(format!("{}.qcow2", target.name));
    let raw_str = raw_path.to_str().unwrap();

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

    // Use a unique build-time pool name to avoid collisions when building
    // inside a VM that already has its own "rpool" (e.g. OmniOS builder VMs).
    // The pool is renamed to the final name after export.
    let final_pool_name = "rpool".to_string();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let build_id = format!("{:08x}", nanos ^ std::process::id());
    let pool_name = format!("forgebuild_{build_id}");
    let be_dataset = format!("{pool_name}/ROOT/be-1");

    info!(disk_size, "Step 1: Creating raw disk image");
    crate::tools::qemu_img::create_raw(runner, raw_str, disk_size).await?;

    info!("Step 2: Attaching loopback device");
    let device = crate::tools::loopback::attach(runner, raw_str).await?;

    // Create a whole-disk ZFS pool. bootadm install-bootloader will later
    // set up the UEFI ESP on this disk — it knows how to manage the EFI
    // System Partition on the pool's device via the -M flag.
    info!(device = %device, "Step 3: Creating ZFS pool");
    crate::tools::zpool::create(runner, &pool_name, &device, &pool_props).await?;

    // ESP and ZFS partition paths — on illumos, bootadm creates these when
    // it runs install-bootloader with -M. For now track them for the finalize step.
    let efi_part = format!("{device}s0");
    let zfs_part = device.clone();

    info!("Step 4: Creating boot environment structure");
    crate::tools::zfs::create(
        runner,
        &format!("{pool_name}/ROOT"),
        &[("canmount", "off"), ("mountpoint", "legacy")],
    )
    .await?;

    // Mount the BE dataset at a fresh tempdir — not the Phase 1 staging root.
    let mount_dir = tempfile::tempdir().map_err(ForgeError::StagingSetup)?;
    let mount_str = mount_dir.path().to_str().unwrap();

    crate::tools::zfs::create(
        runner,
        &be_dataset,
        &[("canmount", "noauto"), ("mountpoint", mount_str)],
    )
    .await?;

    crate::tools::zfs::mount(runner, &be_dataset).await?;

    info!("Step 5: ZFS BE mounted at {}", mount_str);

    Ok(PreparedZfs {
        raw_path,
        qcow2_path,
        device,
        efi_part,
        zfs_part,
        pool_name,
        final_pool_name,
        be_dataset,
        bootloader_type,
        mount_dir,
    })
}

/// Phase 2 finalize: install bootloader, set bootfs, unmount + export pool,
/// then rename the build-time pool to the final name (e.g. "rpool").
pub async fn finalize_zfs(
    prepared: &PreparedZfs,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    let mount_str = prepared.mount_dir.path().to_str().unwrap();

    // bootadm install-bootloader with -M flag manages the EFI System
    // Partition automatically — it creates, formats, mounts the ESP, installs
    // the illumos loader, and unmounts it. No manual ESP mounting needed.
    info!("Finalize step 1: Installing bootloader");
    crate::tools::bootloader::install(
        runner,
        mount_str,
        &prepared.pool_name,
        &prepared.bootloader_type,
    )
    .await?;

    info!("Finalize step 2: Setting bootfs property");
    crate::tools::zpool::set(runner, &prepared.pool_name, "bootfs", &prepared.be_dataset).await?;

    info!("Finalize step 3: Unmounting and exporting ZFS pool");
    crate::tools::zfs::unmount(runner, &prepared.be_dataset).await?;
    crate::tools::zpool::export(runner, &prepared.pool_name).await?;

    // Rename the pool from the build-time name to the final name.
    // The pool is exported and the loopback device is still attached,
    // so we can reimport with the new name.
    //
    // This will fail inside builder VMs that have their own "rpool" active
    // (e.g. OmniOS builders) — in that case the image keeps the build-time
    // pool name and can be renamed at deployment with:
    //   zpool import <build_name> rpool
    if prepared.pool_name != prepared.final_pool_name {
        info!(
            build_name = %prepared.pool_name,
            final_name = %prepared.final_pool_name,
            "Finalize step 4: Renaming pool to final name"
        );
        match crate::tools::zpool::rename_exported(
            runner,
            &prepared.zfs_part,
            &prepared.pool_name,
            &prepared.final_pool_name,
        )
        .await
        {
            Ok(()) => info!("Pool renamed to '{}'", prepared.final_pool_name),
            Err(e) => tracing::warn!(
                error = %e,
                build_name = %prepared.pool_name,
                "Pool rename failed (host likely has active '{pool}') — \
                 image pool is named '{build}'; rename at deployment with: \
                 zpool import {build} {pool}",
                pool = prepared.final_pool_name,
                build = prepared.pool_name,
            ),
        }
    }

    Ok(())
}

/// Cleanup: detach loopback, convert raw→qcow2 (if `convert` is true), remove raw file.
///
/// Always runs, even if earlier phases failed — the loopback device must be detached.
pub async fn cleanup_zfs(
    prepared: PreparedZfs,
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
        info!(path = %prepared.qcow2_path.display(), "QCOW2 (ZFS) image created");
    }

    Ok(())
}
