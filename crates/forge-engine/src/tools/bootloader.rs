use crate::error::ForgeError;
use crate::tools::ToolRunner;
use tracing::info;

/// Install the bootloader into the staging root.
///
/// For illumos: uses `bootadm` to install the boot archive and UEFI loader.
/// For Linux: uses `grub-install` with chroot.
#[cfg(target_os = "illumos")]
pub async fn install(
    runner: &dyn ToolRunner,
    staging_root: &str,
    pool_name: &str,
    bootloader_type: &str,
) -> Result<(), ForgeError> {
    info!(staging_root, pool_name, bootloader_type, "Installing bootloader (illumos)");

    // Install the boot archive
    runner
        .run("bootadm", &["update-archive", "-R", staging_root])
        .await?;

    if bootloader_type == "uefi" {
        // Install the UEFI bootloader
        runner
            .run(
                "bootadm",
                &["install-bootloader", "-M", "-f", "-P", pool_name, "-R", staging_root],
            )
            .await?;
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub async fn install(
    runner: &dyn ToolRunner,
    staging_root: &str,
    _pool_name: &str,
    bootloader_type: &str,
) -> Result<(), ForgeError> {
    info!(staging_root, bootloader_type, "Installing bootloader (Linux)");

    match bootloader_type {
        "grub" | "uefi" => {
            runner
                .run(
                    "chroot",
                    &[staging_root, "grub-install", "--target=x86_64-efi"],
                )
                .await?;
        }
        other => {
            return Err(ForgeError::Qcow2Build {
                step: "bootloader_install".to_string(),
                detail: format!("Unsupported bootloader type: {other}"),
            });
        }
    }

    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "illumos")))]
pub async fn install(
    _runner: &dyn ToolRunner,
    _staging_root: &str,
    _pool_name: &str,
    bootloader_type: &str,
) -> Result<(), ForgeError> {
    Err(ForgeError::Qcow2Build {
        step: "bootloader_install".to_string(),
        detail: format!(
            "Bootloader installation is not supported on this platform (type: {bootloader_type})"
        ),
    })
}
