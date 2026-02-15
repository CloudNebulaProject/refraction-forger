use std::path::{Path, PathBuf};

use tracing::info;

use vm_manager::ssh;

use crate::error::BuilderError;
use crate::lifecycle::BuilderSession;

const REMOTE_BUILD_DIR: &str = "/var/tmp/forger-build";

/// Upload all build inputs to the builder VM.
pub fn upload_build_inputs(
    session: &BuilderSession,
    forger_binary: &Path,
    spec_path: &Path,
    files_dir: &Path,
) -> Result<(), BuilderError> {
    let sess = &session.ssh_session;

    // Create remote build directory
    ssh::exec(sess, &format!("mkdir -p {REMOTE_BUILD_DIR}/output"))
        .map_err(|e| BuilderError::TransferFailed {
            detail: format!("mkdir: {e}"),
        })?;

    // Upload forger binary
    info!("Uploading forger binary to builder VM");
    let remote_forger = PathBuf::from(format!("{REMOTE_BUILD_DIR}/forger"));
    ssh::upload(sess, forger_binary, &remote_forger).map_err(|e| {
        BuilderError::TransferFailed {
            detail: format!("upload forger binary: {e}"),
        }
    })?;

    // Make executable
    ssh::exec(sess, &format!("chmod +x {REMOTE_BUILD_DIR}/forger")).map_err(|e| {
        BuilderError::TransferFailed {
            detail: format!("chmod forger: {e}"),
        }
    })?;

    // Upload spec file
    info!("Uploading spec file");
    let remote_spec = PathBuf::from(format!("{REMOTE_BUILD_DIR}/spec.kdl"));
    ssh::upload(sess, spec_path, &remote_spec).map_err(|e| BuilderError::TransferFailed {
        detail: format!("upload spec: {e}"),
    })?;

    // Upload sibling .kdl files (base/include references resolved relative to spec dir)
    if let Some(spec_dir) = spec_path.parent() {
        if let Ok(entries) = std::fs::read_dir(spec_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "kdl") && path != spec_path {
                    let filename = path.file_name().unwrap();
                    let remote_path =
                        PathBuf::from(format!("{REMOTE_BUILD_DIR}/{}", filename.to_string_lossy()));
                    info!(file = %filename.to_string_lossy(), "Uploading include file");
                    ssh::upload(sess, &path, &remote_path).map_err(|e| {
                        BuilderError::TransferFailed {
                            detail: format!("upload include {}: {e}", filename.to_string_lossy()),
                        }
                    })?;
                }
            }
        }
    }

    // Upload files/ directory if it exists (tar locally → upload → extract remotely)
    if files_dir.exists() && files_dir.is_dir() {
        upload_directory(sess, files_dir, &format!("{REMOTE_BUILD_DIR}/files"))?;
    }

    Ok(())
}

/// Upload a local directory to the VM by creating a tar, uploading, and extracting.
fn upload_directory(
    sess: &ssh2::Session,
    local_dir: &Path,
    remote_dir: &str,
) -> Result<(), BuilderError> {
    info!(local = %local_dir.display(), remote = %remote_dir, "Uploading directory to builder VM");

    // Create a tar archive in memory
    let mut tar_buf = Vec::new();
    {
        let mut ar = tar::Builder::new(&mut tar_buf);
        ar.append_dir_all(".", local_dir)
            .map_err(|e| BuilderError::TransferFailed {
                detail: format!("tar {}: {e}", local_dir.display()),
            })?;
        ar.finish().map_err(|e| BuilderError::TransferFailed {
            detail: format!("tar finish: {e}"),
        })?;
    }

    // Write tar to a temp file so we can upload it
    let tmp = tempfile::NamedTempFile::new().map_err(|e| BuilderError::TransferFailed {
        detail: format!("tempfile: {e}"),
    })?;
    std::fs::write(tmp.path(), &tar_buf).map_err(|e| BuilderError::TransferFailed {
        detail: format!("write tar: {e}"),
    })?;

    let remote_tar = PathBuf::from(format!("{remote_dir}.tar"));
    ssh::upload(sess, tmp.path(), &remote_tar).map_err(|e| BuilderError::TransferFailed {
        detail: format!("upload tar: {e}"),
    })?;

    // Extract on remote
    ssh::exec(
        sess,
        &format!("mkdir -p {remote_dir} && tar xf {remote_dir}.tar -C {remote_dir} && rm {remote_dir}.tar"),
    )
    .map_err(|e| BuilderError::TransferFailed {
        detail: format!("extract tar: {e}"),
    })?;

    Ok(())
}

/// Download build artifacts from the builder VM.
pub fn download_artifacts(
    session: &BuilderSession,
    output_dir: &Path,
) -> Result<(), BuilderError> {
    let sess = &session.ssh_session;
    let remote_output = format!("{REMOTE_BUILD_DIR}/output");

    // List files in remote output directory (use ls -1 for portability; GNU
    // find -printf is not available on illumos)
    let (stdout, _, exit_code) = ssh::exec(
        sess,
        &format!("ls -1 {remote_output}/ 2>/dev/null"),
    )
    .map_err(|e| BuilderError::DownloadFailed {
        detail: format!("list remote files: {e}"),
    })?;

    if exit_code != 0 {
        return Err(BuilderError::DownloadFailed {
            detail: "failed to list remote output directory".to_string(),
        });
    }

    std::fs::create_dir_all(output_dir).map_err(|e| BuilderError::DownloadFailed {
        detail: format!("create output dir: {e}"),
    })?;

    for filename in stdout.lines() {
        let filename = filename.trim();
        if filename.is_empty() {
            continue;
        }

        let remote_path = PathBuf::from(format!("{remote_output}/{filename}"));
        let local_path = output_dir.join(filename);

        info!(file = %filename, "Downloading artifact from builder VM");
        ssh::download(sess, &remote_path, &local_path).map_err(|e| {
            BuilderError::DownloadFailed {
                detail: format!("download {filename}: {e}"),
            }
        })?;
    }

    Ok(())
}
