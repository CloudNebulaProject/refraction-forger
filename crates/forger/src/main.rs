mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use miette::Result;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "forger",
    version,
    about = "Build optimized OS images and publish to OCI registries"
)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build an image from a spec file
    Build {
        /// Path to the spec file
        #[arg(short, long)]
        spec: PathBuf,

        /// Target name to build (builds all if omitted)
        #[arg(short, long)]
        target: Option<String>,

        /// Active profiles for conditional blocks
        #[arg(short, long)]
        profile: Vec<String>,

        /// Output directory for build artifacts
        #[arg(short, long, default_value = "./output")]
        output_dir: PathBuf,
    },

    /// Validate a spec file (parse + resolve includes)
    Validate {
        /// Path to the spec file
        #[arg(short, long)]
        spec: PathBuf,
    },

    /// Inspect a resolved spec (parse + resolve + apply profiles)
    Inspect {
        /// Path to the spec file
        #[arg(short, long)]
        spec: PathBuf,

        /// Active profiles for conditional blocks
        #[arg(short, long)]
        profile: Vec<String>,
    },

    /// Push an OCI Image Layout to a registry
    Push {
        /// Path to the OCI Image Layout directory
        #[arg(short, long)]
        image: PathBuf,

        /// Registry reference (e.g., ghcr.io/org/image:tag)
        #[arg(short, long)]
        reference: String,

        /// Path to auth file (JSON with username/password or token)
        #[arg(short, long)]
        auth_file: Option<PathBuf>,
    },

    /// List available targets from a spec file
    Targets {
        /// Path to the spec file
        #[arg(short, long)]
        spec: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    match args.command {
        Commands::Build {
            spec,
            target,
            profile,
            output_dir,
        } => {
            commands::build::run(&spec, target.as_deref(), &profile, &output_dir).await?;
        }
        Commands::Validate { spec } => {
            commands::validate::run(&spec)?;
        }
        Commands::Inspect { spec, profile } => {
            commands::inspect::run(&spec, &profile)?;
        }
        Commands::Push {
            image,
            reference,
            auth_file,
        } => {
            commands::push::run(&image, &reference, auth_file.as_ref()).await?;
        }
        Commands::Targets { spec } => {
            commands::targets::run(&spec)?;
        }
    }

    Ok(())
}
