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

/// Attach a file to a labeled loopback device (illumos).
///
/// Labeled lofi devices appear as real disks with partition tables,
/// allowing `zpool create -B` to create the ESP and BIOS boot partitions.
/// The returned device path is the base disk device (e.g., the `p0` device
/// with `p0` stripped, as expected by zpool).
#[cfg(target_os = "illumos")]
pub async fn attach_labeled(
    runner: &dyn ToolRunner,
    file_path: &str,
) -> Result<String, ForgeError> {
    info!(file_path, "Attaching labeled loopback device (illumos)");
    let output = runner.run("lofiadm", &["-l", "-a", file_path]).await?;
    let dev = output.stdout.trim().to_string();
    // lofiadm -l returns a device like /dev/lofi/1 but the actual disk device
    // that zpool needs is the one without a partition suffix. Strip p0 if present.
    Ok(dev.trim_end_matches("p0").to_string())
}

/// Attach a file as a labeled loopback device (Linux stub — not applicable).
#[cfg(not(target_os = "illumos"))]
pub async fn attach_labeled(
    runner: &dyn ToolRunner,
    file_path: &str,
) -> Result<String, ForgeError> {
    // On Linux, losetup doesn't have a labeled mode — just use regular attach
    attach(runner, file_path).await
}

/// Detach a loopback device.
#[cfg(target_os = "illumos")]
pub async fn detach(runner: &dyn ToolRunner, device: &str) -> Result<(), ForgeError> {
    info!(device, "Detaching loopback device (illumos)");
    runner.run("lofiadm", &["-d", device]).await?;
    Ok(())
}

/// Re-read the partition table of a device.
#[cfg(target_os = "linux")]
pub async fn partprobe(runner: &dyn ToolRunner, device: &str) -> Result<(), ForgeError> {
    info!(device, "Re-reading partition table (partprobe)");
    runner.run("partprobe", &[device]).await?;
    Ok(())
}

#[cfg(target_os = "illumos")]
pub async fn partprobe(_runner: &dyn ToolRunner, _device: &str) -> Result<(), ForgeError> {
    // illumos doesn't need partprobe for lofi devices
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "illumos")))]
pub async fn partprobe(_runner: &dyn ToolRunner, _device: &str) -> Result<(), ForgeError> {
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
