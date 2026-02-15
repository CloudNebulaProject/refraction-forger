use crate::error::ForgeError;
use crate::tools::ToolRunner;
use tracing::info;

/// Create a raw disk image of the given size.
pub async fn create_raw(
    runner: &dyn ToolRunner,
    path: &str,
    size: &str,
) -> Result<(), ForgeError> {
    info!(path, size, "Creating raw disk image");
    runner
        .run("qemu-img", &["create", "-f", "raw", path, size])
        .await?;
    Ok(())
}

/// Convert a raw image to qcow2 format.
pub async fn convert_to_qcow2(
    runner: &dyn ToolRunner,
    input: &str,
    output: &str,
) -> Result<(), ForgeError> {
    info!(input, output, "Converting raw image to qcow2");
    runner
        .run(
            "qemu-img",
            &["convert", "-f", "raw", "-O", "qcow2", input, output],
        )
        .await?;
    Ok(())
}

/// Get info about a disk image (JSON output).
pub async fn info(runner: &dyn ToolRunner, path: &str) -> Result<String, ForgeError> {
    let output = runner
        .run("qemu-img", &["info", "--output=json", path])
        .await?;
    Ok(output.stdout)
}
