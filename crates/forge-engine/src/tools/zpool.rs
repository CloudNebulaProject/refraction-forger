use crate::error::ForgeError;
use crate::tools::ToolRunner;
use tracing::info;

/// Create a ZFS pool on the given device.
pub async fn create(
    runner: &dyn ToolRunner,
    pool_name: &str,
    device: &str,
    properties: &[(&str, &str)],
) -> Result<(), ForgeError> {
    info!(pool_name, device, "Creating ZFS pool");
    let mut args = vec!["create"];

    // Suppress default mountpoint — child datasets set explicit mountpoints
    args.extend_from_slice(&["-m", "none"]);

    // Add -o property=value for each pool property
    let prop_strings: Vec<String> = properties
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    for prop in &prop_strings {
        args.push("-o");
        args.push(prop);
    }

    args.push(pool_name);
    args.push(device);

    runner.run("zpool", &args).await?;
    Ok(())
}

/// Create a bootable ZFS pool with GPT + EFI System Partition.
///
/// Uses `zpool create -B` which tells illumos to set up the disk with
/// a BIOS boot partition, EFI System Partition, and ZFS partition.
/// This is the idiomatic way to create bootable ZFS pools on illumos.
pub async fn create_bootable(
    runner: &dyn ToolRunner,
    pool_name: &str,
    device: &str,
    properties: &[(&str, &str)],
) -> Result<(), ForgeError> {
    info!(pool_name, device, "Creating bootable ZFS pool (-B)");
    let mut args = vec!["create", "-B"];

    // Suppress default mountpoint
    args.extend_from_slice(&["-m", "none"]);

    // Add -o property=value for each pool property
    let prop_strings: Vec<String> = properties
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    for prop in &prop_strings {
        args.push("-o");
        args.push(prop);
    }

    args.push(pool_name);
    args.push(device);

    runner.run("zpool", &args).await?;
    Ok(())
}

/// Export a ZFS pool.
pub async fn export(runner: &dyn ToolRunner, pool_name: &str) -> Result<(), ForgeError> {
    info!(pool_name, "Exporting ZFS pool");
    runner.run("zpool", &["export", pool_name]).await?;
    Ok(())
}

/// Rename an exported pool by importing it with a new name and re-exporting.
///
/// The pool must already be exported. The `device` is the loopback device
/// (e.g. `/dev/lofi/1`) where the pool resides — used with `-d` to avoid
/// scanning all devices.
pub async fn rename_exported(
    runner: &dyn ToolRunner,
    device: &str,
    old_name: &str,
    new_name: &str,
) -> Result<(), ForgeError> {
    info!(old_name, new_name, device, "Renaming ZFS pool via import/export");

    // Import from the specific device, rename, don't mount anything
    runner
        .run(
            "zpool",
            &["import", "-f", "-N", "-d", device, old_name, new_name],
        )
        .await?;

    // Immediately export the renamed pool
    runner.run("zpool", &["export", new_name]).await?;

    info!(new_name, "Pool renamed successfully");
    Ok(())
}

/// Destroy a ZFS pool (force).
pub async fn destroy(runner: &dyn ToolRunner, pool_name: &str) -> Result<(), ForgeError> {
    info!(pool_name, "Destroying ZFS pool");
    runner.run("zpool", &["destroy", "-f", pool_name]).await?;
    Ok(())
}

/// Set a property on a ZFS pool.
pub async fn set(
    runner: &dyn ToolRunner,
    pool_name: &str,
    property: &str,
    value: &str,
) -> Result<(), ForgeError> {
    let prop_val = format!("{property}={value}");
    runner.run("zpool", &["set", &prop_val, pool_name]).await?;
    Ok(())
}
