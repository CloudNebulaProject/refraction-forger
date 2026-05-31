// See note in vm-manager/src/error.rs about thiserror 2 + edition 2024
#![allow(unused_assignments)]

use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum BuilderError {
    #[error("failed to resolve builder image: {detail}")]
    #[diagnostic(
        code(forge_builder::image_resolve_failed),
        help("ensure the builder image reference is valid and the registry is reachable — for OCI images, check that GITHUB_TOKEN is set")
    )]
    ImageResolveFailed { detail: String },

    #[error("builder VM lifecycle error at {phase}: {detail}")]
    #[diagnostic(
        code(forge_builder::vm_lifecycle),
        help("ensure QEMU and KVM are available on this host — install qemu-system-x86_64 and check /dev/kvm permissions")
    )]
    VmLifecycle { phase: String, detail: String },

    #[error("forger binary for target {target_triple} not found at {path}")]
    #[diagnostic(
        code(forge_builder::binary_not_found),
        help("build the cross-compiled binary with:\n  cargo build --target {target_triple} --release -p forger")
    )]
    BinaryNotFound {
        target_triple: String,
        path: String,
    },

    #[error("failed to download forger binary from {url}: {detail}")]
    #[diagnostic(
        code(forge_builder::binary_download_failed),
        help("check network connectivity and that the release exists at the given URL")
    )]
    BinaryDownloadFailed { url: String, detail: String },

    #[error("invalid builder binary source {input:?}: {detail}")]
    #[diagnostic(
        code(forge_builder::binary_source_invalid),
        help("use a /path, ./relative/path, http(s):// URL, or oci:// reference")
    )]
    // NB: the field is named `input` rather than `source` because thiserror
    // treats a field literally named `source` as the `Error::source()` cause
    // (which must itself be an error type).
    BinarySourceInvalid { input: String, detail: String },

    #[error("sha256 mismatch for builder binary from {url}: expected {expected}, got {got}")]
    #[diagnostic(
        code(forge_builder::binary_sha256_mismatch),
        help("the fetched bytes do not match the configured sha256 — verify the source or update the pin")
    )]
    BinarySha256Mismatch {
        url: String,
        expected: String,
        got: String,
    },

    #[error("OCI pull failed for {reference}: {detail}")]
    #[diagnostic(
        code(forge_builder::oci_pull_failed),
        help("check the registry is reachable and credentials are available (docker login, or FORGER_OCI_BEARER)")
    )]
    OciPullFailed { reference: String, detail: String },

    #[error("OCI image {reference} must have exactly one layer, found {layer_count}")]
    #[diagnostic(
        code(forge_builder::oci_malformed),
        help("publish the forger binary as a single-layer OCI artifact; for multi-platform, template {{triple}} into a per-platform tag")
    )]
    OciMalformed {
        reference: String,
        layer_count: usize,
    },

    #[error("OCI artifact {reference} has unsupported layer media type {media_type}")]
    #[diagnostic(
        code(forge_builder::oci_unsupported_layer),
        help("publish with: oras push <ref> ./forger:application/vnd.refraction.forger.binary.v1")
    )]
    OciUnsupportedLayer {
        reference: String,
        media_type: String,
    },

    #[error("file transfer to builder VM failed: {detail}")]
    #[diagnostic(
        code(forge_builder::transfer_failed),
        help("check that the builder VM is reachable via SSH and has enough disk space")
    )]
    TransferFailed { detail: String },

    #[error("remote build inside builder VM failed with exit code {exit_code}")]
    #[diagnostic(
        code(forge_builder::remote_build_failed),
        help("check the build output above for errors — the forger build ran inside the builder VM\n{detail}")
    )]
    RemoteBuildFailed { exit_code: i32, detail: String },

    #[error("failed to download build artifacts from builder VM: {detail}")]
    #[diagnostic(
        code(forge_builder::download_failed),
        help("the build may have succeeded but artifact retrieval failed — check VM connectivity")
    )]
    DownloadFailed { detail: String },

    #[error("SSH keypair generation failed: {detail}")]
    #[diagnostic(
        code(forge_builder::keygen_failed),
        help("this is an internal error in Ed25519 key generation — please report it")
    )]
    KeygenFailed { detail: String },

    #[error("failed to push artifact to OCI registry: {detail}")]
    #[diagnostic(
        code(forge_builder::artifact_push_failed),
        help("check that GITHUB_TOKEN is set and the registry reference is valid")
    )]
    ArtifactPushFailed { detail: String },

    #[error(transparent)]
    #[diagnostic(code(forge_builder::vm_error))]
    VmError(#[from] vm_manager::VmError),
}
