use crate::error::ForgeError;
use crate::tools::ToolRunner;
use tracing::info;

/// Run devfsadm in the staging root to populate /dev.
/// This is illumos-specific and is a no-op on other platforms.
#[cfg(target_os = "illumos")]
pub async fn run_devfsadm(runner: &dyn ToolRunner, root: &str) -> Result<(), ForgeError> {
    info!(root, "Running devfsadm");
    runner.run("devfsadm", &["-r", root]).await?;
    Ok(())
}

#[cfg(not(target_os = "illumos"))]
pub async fn run_devfsadm(_runner: &dyn ToolRunner, root: &str) -> Result<(), ForgeError> {
    info!(root, "Skipping devfsadm (not on illumos)");
    Ok(())
}
