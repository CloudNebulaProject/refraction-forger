use crate::error::ForgeError;
use crate::tools::ToolRunner;
use tracing::info;

/// Attach a file to a loopback device and return the device path.
#[cfg(target_os = "linux")]
pub async fn attach(runner: &dyn ToolRunner, file_path: &str) -> Result<String, ForgeError> {
    info!(file_path, "Attaching loopback device (Linux)");
    let output = runner
        .run("losetup", &["--find", "--show", file_path])
        .await?;
    Ok(output.stdout.trim().to_string())
}

/// Detach a loopback device.
#[cfg(target_os = "linux")]
pub async fn detach(runner: &dyn ToolRunner, device: &str) -> Result<(), ForgeError> {
    info!(device, "Detaching loopback device (Linux)");
    runner.run("losetup", &["--detach", device]).await?;
    Ok(())
}

/// Attach a file to a loopback device and return the device path.
#[cfg(target_os = "illumos")]
pub async fn attach(runner: &dyn ToolRunner, file_path: &str) -> Result<String, ForgeError> {
    info!(file_path, "Attaching loopback device (illumos)");
    let output = runner.run("lofiadm", &["-a", file_path]).await?;
    Ok(output.stdout.trim().to_string())
}

/// Detach a loopback device.
#[cfg(target_os = "illumos")]
pub async fn detach(runner: &dyn ToolRunner, device: &str) -> Result<(), ForgeError> {
    info!(device, "Detaching loopback device (illumos)");
    runner.run("lofiadm", &["-d", device]).await?;
    Ok(())
}

// Stub for unsupported platforms (compile-time guard)
#[cfg(not(any(target_os = "linux", target_os = "illumos")))]
pub async fn attach(_runner: &dyn ToolRunner, file_path: &str) -> Result<String, ForgeError> {
    Err(ForgeError::Qcow2Build {
        step: "loopback_attach".to_string(),
        detail: format!("Loopback devices are not supported on this platform (file: {file_path})"),
    })
}

#[cfg(not(any(target_os = "linux", target_os = "illumos")))]
pub async fn detach(_runner: &dyn ToolRunner, device: &str) -> Result<(), ForgeError> {
    Err(ForgeError::Qcow2Build {
        step: "loopback_detach".to_string(),
        detail: format!(
            "Loopback devices are not supported on this platform (device: {device})"
        ),
    })
}
