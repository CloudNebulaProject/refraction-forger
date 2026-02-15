use spec_parser::schema::PackageList;
use tracing::info;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// Install all package lists into the staging root via `apt-get install` in chroot.
pub async fn install_all(
    runner: &dyn ToolRunner,
    root: &str,
    package_lists: &[PackageList],
) -> Result<(), ForgeError> {
    let all_packages: Vec<String> = package_lists
        .iter()
        .flat_map(|pl| pl.packages.iter().map(|p| p.name.clone()))
        .collect();

    if all_packages.is_empty() {
        info!("No packages to install");
        return Ok(());
    }

    info!(count = all_packages.len(), "Installing packages via apt");
    crate::tools::apt::install(runner, root, &all_packages).await
}
