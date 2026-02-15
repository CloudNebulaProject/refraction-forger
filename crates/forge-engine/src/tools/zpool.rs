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
