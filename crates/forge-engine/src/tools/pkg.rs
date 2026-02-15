use crate::error::ForgeError;
use crate::tools::ToolRunner;
use tracing::info;

/// Create a new IPS image at the given root path.
pub async fn image_create(runner: &dyn ToolRunner, root: &str) -> Result<(), ForgeError> {
    info!(root, "Creating IPS image");
    runner.run("pkg", &["image-create", "-F", root]).await?;
    Ok(())
}

/// Set a publisher in the IPS image at the given root.
pub async fn set_publisher(
    runner: &dyn ToolRunner,
    root: &str,
    name: &str,
    origin: &str,
) -> Result<(), ForgeError> {
    info!(root, name, origin, "Setting publisher");
    runner
        .run("pkg", &["-R", root, "set-publisher", "-O", origin, name])
        .await?;
    Ok(())
}

/// Install packages into the IPS image at the given root.
pub async fn install(
    runner: &dyn ToolRunner,
    root: &str,
    packages: &[String],
) -> Result<(), ForgeError> {
    if packages.is_empty() {
        return Ok(());
    }
    info!(root, count = packages.len(), "Installing packages");
    let mut args = vec!["-R", root, "install", "--accept"];
    let pkg_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
    args.extend(pkg_refs);
    runner.run("pkg", &args).await?;
    Ok(())
}

/// Change an IPS variant in the image at the given root.
///
/// Exit code 4 from `pkg` means "nothing to do" (already set) — treated as success.
pub async fn change_variant(
    runner: &dyn ToolRunner,
    root: &str,
    name: &str,
    value: &str,
) -> Result<(), ForgeError> {
    info!(root, name, value, "Changing variant");
    let variant_arg = format!("{name}={value}");
    match runner
        .run("pkg", &["-R", root, "change-variant", &variant_arg])
        .await
    {
        Ok(_) => Ok(()),
        Err(ForgeError::ToolNonZero { exit_code: 4, .. }) => {
            info!(name, value, "Variant already set — nothing to do");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Approve a CA certificate for a publisher in the IPS image.
pub async fn approve_ca_cert(
    runner: &dyn ToolRunner,
    root: &str,
    publisher: &str,
    certfile: &str,
) -> Result<(), ForgeError> {
    info!(root, publisher, certfile, "Approving CA certificate");
    runner
        .run(
            "pkg",
            &[
                "-R",
                root,
                "set-publisher",
                "--approve-ca-cert",
                certfile,
                publisher,
            ],
        )
        .await?;
    Ok(())
}

/// Set the incorporation package.
pub async fn set_incorporation(
    runner: &dyn ToolRunner,
    root: &str,
    incorporation: &str,
) -> Result<(), ForgeError> {
    info!(root, incorporation, "Setting incorporation");
    runner
        .run("pkg", &["-R", root, "install", "--accept", incorporation])
        .await?;
    Ok(())
}
