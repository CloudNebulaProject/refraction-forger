use std::path::Path;

use flate2::write::GzEncoder;
use flate2::Compression;
use miette::Diagnostic;
use sha2::{Digest, Sha256};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Error, Diagnostic)]
pub enum TarLayerError {
    #[error("Failed to walk staging directory: {path}")]
    #[diagnostic(help("Ensure the staging directory exists and is readable"))]
    WalkDir {
        path: String,
        #[source]
        source: walkdir::Error,
    },

    #[error("Failed to create tar archive")]
    #[diagnostic(help("Check disk space and permissions"))]
    TarCreate(#[source] std::io::Error),

    #[error("Failed to read file for tar: {path}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Result of creating a tar.gz layer from a staging directory.
#[derive(Clone)]
pub struct LayerBlob {
    /// Compressed tar.gz data
    pub data: Vec<u8>,
    /// SHA-256 digest of the compressed data (hex-encoded)
    pub digest: String,
    /// Uncompressed size in bytes
    pub uncompressed_size: u64,
}

/// Create a tar.gz layer from a staging directory. All paths in the tar are
/// relative to the staging root.
pub fn create_layer(staging_dir: &Path) -> Result<LayerBlob, TarLayerError> {
    let mut uncompressed_size: u64 = 0;
    let buf = Vec::new();
    let encoder = GzEncoder::new(buf, Compression::default());
    let mut tar = tar::Builder::new(encoder);

    for entry in WalkDir::new(staging_dir).follow_links(false) {
        let entry = entry.map_err(|e| TarLayerError::WalkDir {
            path: staging_dir.display().to_string(),
            source: e,
        })?;

        let full_path = entry.path();
        let rel_path = full_path
            .strip_prefix(staging_dir)
            .unwrap_or(full_path);

        // Skip the root directory itself
        if rel_path.as_os_str().is_empty() {
            continue;
        }

        let metadata = entry.metadata().map_err(|e| TarLayerError::WalkDir {
            path: full_path.display().to_string(),
            source: e,
        })?;

        if metadata.is_file() {
            uncompressed_size += metadata.len();
            let mut header = tar::Header::new_gnu();
            header.set_size(metadata.len());
            header.set_mode(0o644);
            header.set_cksum();

            let file_data =
                std::fs::read(full_path).map_err(|e| TarLayerError::ReadFile {
                    path: full_path.display().to_string(),
                    source: e,
                })?;

            tar.append_data(&mut header, rel_path, file_data.as_slice())
                .map_err(TarLayerError::TarCreate)?;
        } else if metadata.is_dir() {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
            header.set_mode(0o755);
            header.set_cksum();

            tar.append_data(&mut header, rel_path, std::io::empty())
                .map_err(TarLayerError::TarCreate)?;
        } else if metadata.is_symlink() {
            let link_target = std::fs::read_link(full_path).map_err(|e| {
                TarLayerError::ReadFile {
                    path: full_path.display().to_string(),
                    source: e,
                }
            })?;

            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_cksum();

            tar.append_link(&mut header, rel_path, &link_target)
                .map_err(TarLayerError::TarCreate)?;
        }
    }

    let encoder = tar.into_inner().map_err(TarLayerError::TarCreate)?;
    let compressed = encoder.finish().map_err(TarLayerError::TarCreate)?;

    let mut hasher = Sha256::new();
    hasher.update(&compressed);
    let digest = format!("sha256:{}", hex::encode(hasher.finalize()));

    Ok(LayerBlob {
        data: compressed,
        digest,
        uncompressed_size,
    })
}
