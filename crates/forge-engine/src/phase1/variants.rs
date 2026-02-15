use spec_parser::schema::Variants;
use tracing::info;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// Apply IPS variants via `pkg -R change-variant`.
pub async fn apply_variants(
    runner: &dyn ToolRunner,
    root: &str,
    variants: &Variants,
) -> Result<(), ForgeError> {
    for var in &variants.vars {
        info!(name = %var.name, value = %var.value, "Applying IPS variant");
        crate::tools::pkg::change_variant(runner, root, &var.name, &var.value).await?;
    }
    Ok(())
}
