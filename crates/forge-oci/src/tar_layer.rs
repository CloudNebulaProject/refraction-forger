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
    /// SHA-256 digest of the compressed data (format: "sha256:<hex>")
    pub digest: String,
    /// SHA-256 digest of the uncompressed tar data (format: "sha256:<hex>")
    /// Used as the OCI diff_id per the image spec.
    pub uncompressed_digest: String,
    /// Uncompressed size in bytes
    pub uncompressed_size: u64,
}

/// Create a tar.gz layer from a staging directory. All paths in the tar are
/// relative to the staging root.
pub fn create_layer(staging_dir: &Path) -> Result<LayerBlob, TarLayerError> {
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

    // To get the uncompressed tar, decompress it back (simplest correct approach)
    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut uncompressed_tar = Vec::new();
    decoder
        .read_to_end(&mut uncompressed_tar)
        .map_err(TarLayerError::TarCreate)?;

    let mut uncompressed_hasher = Sha256::new();
    uncompressed_hasher.update(&uncompressed_tar);
    let uncompressed_digest = format!("sha256:{}", hex::encode(uncompressed_hasher.finalize()));

    let mut hasher = Sha256::new();
    hasher.update(&compressed);
    let digest = format!("sha256:{}", hex::encode(hasher.finalize()));

    Ok(LayerBlob {
        data: compressed,
        digest,
        uncompressed_digest,
        uncompressed_size: uncompressed_tar.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_create_layer_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let layer = create_layer(tmp.path()).unwrap();

        assert!(layer.data.len() > 0);
        assert!(layer.digest.starts_with("sha256:"));
        assert!(layer.uncompressed_digest.starts_with("sha256:"));
        // Empty tar still has end-of-archive markers
        assert!(layer.uncompressed_size > 0);
        // Compressed and uncompressed digests must differ
        assert_ne!(layer.digest, layer.uncompressed_digest);
    }

    #[test]
    fn test_create_layer_with_file() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("hello.txt"), "Hello, world!").unwrap();

        let layer = create_layer(tmp.path()).unwrap();

        assert!(layer.digest.starts_with("sha256:"));
        assert!(layer.uncompressed_digest.starts_with("sha256:"));
        // Uncompressed tar is larger than just the file content (includes headers)
        assert!(layer.uncompressed_size > 13);
        assert!(layer.data.len() > 0);

        // Verify we can decompress the gzip layer
        let decoder = flate2::read::GzDecoder::new(layer.data.as_slice());
        let mut archive = tar::Archive::new(decoder);
        let entries: Vec<_> = archive.entries().unwrap().collect();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_create_layer_with_nested_dirs() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("etc/ssh")).unwrap();
        fs::write(tmp.path().join("etc/ssh/sshd_config"), "Port 22\n").unwrap();
        fs::write(tmp.path().join("etc/hostname"), "test\n").unwrap();

        let layer = create_layer(tmp.path()).unwrap();

        // Decompress and verify structure
        let decoder = flate2::read::GzDecoder::new(layer.data.as_slice());
        let mut archive = tar::Archive::new(decoder);
        let paths: Vec<String> = archive
            .entries()
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path().unwrap().display().to_string())
            .collect();

        assert!(paths.contains(&"etc".to_string()));
        assert!(paths.contains(&"etc/ssh".to_string()));
        assert!(paths.contains(&"etc/ssh/sshd_config".to_string()));
        assert!(paths.contains(&"etc/hostname".to_string()));
    }

    #[test]
    fn test_create_layer_with_symlink() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("target.txt"), "data").unwrap();
        std::os::unix::fs::symlink("target.txt", tmp.path().join("link.txt")).unwrap();

        let layer = create_layer(tmp.path()).unwrap();

        let decoder = flate2::read::GzDecoder::new(layer.data.as_slice());
        let mut archive = tar::Archive::new(decoder);
        let mut found_symlink = false;
        for entry in archive.entries().unwrap() {
            let entry = entry.unwrap();
            if entry.path().unwrap().display().to_string() == "link.txt" {
                assert_eq!(entry.header().entry_type(), tar::EntryType::Symlink);
                found_symlink = true;
            }
        }
        assert!(found_symlink, "symlink entry not found in tar");
    }

    #[test]
    fn test_digest_is_deterministic() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("file.txt"), "deterministic content").unwrap();

        let layer1 = create_layer(tmp.path()).unwrap();
        let layer2 = create_layer(tmp.path()).unwrap();

        assert_eq!(layer1.digest, layer2.digest);
    }
}
