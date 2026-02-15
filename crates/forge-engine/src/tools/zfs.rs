use crate::error::ForgeError;
use crate::tools::ToolRunner;
use tracing::info;

/// Create a ZFS dataset.
pub async fn create(
    runner: &dyn ToolRunner,
    dataset: &str,
    properties: &[(&str, &str)],
) -> Result<(), ForgeError> {
    info!(dataset, "Creating ZFS dataset");
    let mut args = vec!["create"];

    let prop_strings: Vec<String> = properties
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    for prop in &prop_strings {
        args.push("-o");
        args.push(prop);
    }

    // Create parent datasets if needed
    args.push("-p");
    args.push(dataset);

    runner.run("zfs", &args).await?;
    Ok(())
}

/// Mount a ZFS dataset at a given mountpoint.
pub async fn mount(runner: &dyn ToolRunner, dataset: &str) -> Result<(), ForgeError> {
    info!(dataset, "Mounting ZFS dataset");
    runner.run("zfs", &["mount", dataset]).await?;
    Ok(())
}

/// Set a property on a ZFS dataset.
pub async fn set(
    runner: &dyn ToolRunner,
    dataset: &str,
    property: &str,
    value: &str,
) -> Result<(), ForgeError> {
    let prop_val = format!("{property}={value}");
    runner.run("zfs", &["set", &prop_val, dataset]).await?;
    Ok(())
}

/// Unmount a ZFS dataset.
pub async fn unmount(runner: &dyn ToolRunner, dataset: &str) -> Result<(), ForgeError> {
    runner.run("zfs", &["unmount", dataset]).await?;
    Ok(())
}
