mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use forge_engine::output::OutputMode;
use miette::Result;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "forger",
    version,
    about = "Build optimized OS images and publish to OCI registries"
)]
struct Args {
    /// Output format: pretty (default), json, or quiet
    #[arg(long, global = true, default_value = "pretty")]
    output: String,

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

        /// Force local build (skip builder VM detection)
        #[arg(long)]
        local: bool,

        /// Force build inside a builder VM
        #[arg(long, conflicts_with = "local")]
        use_builder: bool,

        /// Skip OCI registry push (host-side push used with builder VMs)
        #[arg(long)]
        skip_push: bool,

        /// Override builder VM image (path, URL, or oci:// reference)
        #[arg(long)]
        builder_image: Option<String>,

        /// Override the forger binary run inside the builder VM
        /// (a /path, http(s):// URL, or oci:// reference; supports the
        /// `{triple}` template token)
        #[arg(long)]
        builder_binary: Option<String>,

        /// sha256 hex pin for `--builder-binary` (URL/OCI sources)
        #[arg(long, requires = "builder_binary")]
        builder_binary_sha256: Option<String>,

        /// Override the output artifact base name (default: the target name).
        /// Requires `--target` to disambiguate which artifact to rename.
        #[arg(long, requires = "target")]
        output_name: Option<String>,
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

    /// Push an OCI Image Layout or QCOW2 artifact to a registry
    Push {
        /// Path to the OCI Image Layout directory (or QCOW2 file with --artifact)
        #[arg(short, long)]
        image: PathBuf,

        /// Registry reference (e.g., ghcr.io/org/image:tag)
        #[arg(short, long)]
        reference: String,

        /// Path to auth file (JSON with username/password or token)
        #[arg(short, long)]
        auth_file: Option<PathBuf>,

        /// Push as a QCOW2 OCI artifact instead of an OCI Image Layout
        #[arg(long)]
        artifact: bool,
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
    let args = Args::parse();

    let output_mode: OutputMode = args
        .output
        .parse()
        .map_err(|e: String| miette::miette!(e))?;

    // Configure tracing based on output mode:
    // - Json/Quiet: suppress tracing (output handler provides structured events)
    // - Pretty: normal tracing with info level
    let log_level = match output_mode {
        OutputMode::Pretty => "info",
        OutputMode::Json | OutputMode::Quiet => "warn",
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)),
        )
        .init();

    match args.command {
        Commands::Build {
            spec,
            target,
            profile,
            output_dir,
            local,
            use_builder,
            skip_push,
            builder_image,
            builder_binary,
            builder_binary_sha256,
            output_name,
        } => {
            commands::build::run(
                &spec,
                target.as_deref(),
                &profile,
                &output_dir,
                local,
                use_builder,
                skip_push,
                builder_image.as_deref(),
                builder_binary.as_deref(),
                builder_binary_sha256.as_deref(),
                output_name.as_deref(),
                output_mode,
            )
            .await?;
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
            artifact,
        } => {
            commands::push::run(&image, &reference, auth_file.as_ref(), artifact, output_mode).await?;
        }
        Commands::Targets { spec } => {
            commands::targets::run(&spec)?;
        }
    }

    Ok(())
}
