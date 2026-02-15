use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum ForgeError {
    #[error("Staging directory setup failed")]
    #[diagnostic(
        help("Ensure sufficient disk space and write permissions for temporary directories"),
        code(forge::staging_failed)
    )]
    StagingSetup(#[source] std::io::Error),

    #[error("Base tarball extraction failed: {path}")]
    #[diagnostic(
        help("Verify the base tarball exists and is a valid tar.gz or tar archive"),
        code(forge::base_extract_failed)
    )]
    BaseExtract {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Package manager operation failed: {operation}")]
    #[diagnostic(
        help("Check that the package manager is available on the build host and the repositories are reachable.\nCommand: {command}"),
        code(forge::pkg_failed)
    )]
    PackageManager {
        operation: String,
        command: String,
        detail: String,
    },

    #[error("Tool execution failed: `{tool} {args}`")]
    #[diagnostic(
        help("Ensure '{tool}' is installed and available in PATH.\nStderr: {stderr}"),
        code(forge::tool_failed)
    )]
    ToolExecution {
        tool: String,
        args: String,
        stderr: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Tool returned non-zero exit code: `{tool} {args}` (exit code {exit_code})")]
    #[diagnostic(
        help("The command failed. Check stderr output below for details.\nStderr: {stderr}"),
        code(forge::tool_exit_code)
    )]
    ToolNonZero {
        tool: String,
        args: String,
        exit_code: i32,
        stderr: String,
    },

    #[error("Overlay application failed: {action}")]
    #[diagnostic(
        help("Check that the source file exists and the destination path is valid.\n{detail}"),
        code(forge::overlay_failed)
    )]
    Overlay {
        action: String,
        detail: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Overlay source file not found: {path}")]
    #[diagnostic(
        help("Ensure the file exists relative to the images/files/ directory"),
        code(forge::overlay_source_missing)
    )]
    OverlaySourceMissing { path: String },

    #[error("Customization failed: {operation}")]
    #[diagnostic(
        help("{detail}"),
        code(forge::customization_failed)
    )]
    Customization {
        operation: String,
        detail: String,
    },

    #[error("QCOW2 image creation failed: {step}")]
    #[diagnostic(
        help("{detail}"),
        code(forge::qcow2_failed)
    )]
    Qcow2Build { step: String, detail: String },

    #[error("OCI image creation failed")]
    #[diagnostic(code(forge::oci_failed))]
    OciBuild(String),

    #[error("Disk size not specified for qcow2 target")]
    #[diagnostic(
        help("Add a `disk_size \"2000M\"` child node to your qcow2 target block"),
        code(forge::missing_disk_size)
    )]
    MissingDiskSize,

    #[error("Invalid disk size: {value}")]
    #[diagnostic(
        help("Use a value like \"2000M\" or \"20G\""),
        code(forge::invalid_disk_size)
    )]
    InvalidDiskSize { value: String },

    #[error("No target named '{name}' found in spec")]
    #[diagnostic(
        help("Available targets: {available}. Use `forger targets` to list them."),
        code(forge::target_not_found)
    )]
    TargetNotFound { name: String, available: String },

    #[error("Unsupported filesystem '{fs_type}' for target '{target}'")]
    #[diagnostic(
        help("Supported filesystems are 'zfs' (default for OmniOS) and 'ext4' (for Ubuntu/Linux). Set `filesystem \"zfs\"` or `filesystem \"ext4\"` in the target block."),
        code(forge::unsupported_filesystem)
    )]
    UnsupportedFilesystem { fs_type: String, target: String },

    #[error("OCI artifact push failed for {reference}")]
    #[diagnostic(
        help("Check that the registry is reachable, credentials are valid (GITHUB_TOKEN for ghcr.io), and the reference format is correct.\n{detail}"),
        code(forge::artifact_push_failed)
    )]
    ArtifactPushFailed { reference: String, detail: String },

    #[error("IO error")]
    Io(#[from] std::io::Error),
}
