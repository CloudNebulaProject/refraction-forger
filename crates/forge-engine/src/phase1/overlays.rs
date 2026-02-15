use std::path::Path;

use spec_parser::schema::OverlayAction;
use tracing::info;

use crate::error::ForgeError;
use crate::tools::ToolRunner;

/// Apply a list of overlay actions to the staging root.
pub async fn apply_overlays(
    actions: &[OverlayAction],
    staging_root: &Path,
    files_dir: &Path,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    for action in actions {
        apply_action(action, staging_root, files_dir, runner).await?;
    }
    Ok(())
}

async fn apply_action(
    action: &OverlayAction,
    staging_root: &Path,
    files_dir: &Path,
    runner: &dyn ToolRunner,
) -> Result<(), ForgeError> {
    match action {
        OverlayAction::File(file_overlay) => {
            let dest = staging_root.join(
                file_overlay
                    .destination
                    .strip_prefix('/')
                    .unwrap_or(&file_overlay.destination),
            );

            // Ensure parent directory exists
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ForgeError::Overlay {
                    action: format!("create parent dir for {}", file_overlay.destination),
                    detail: parent.display().to_string(),
                    source: e,
                })?;
            }

            if let Some(ref source) = file_overlay.source {
                // Copy source file to destination
                let src_path = files_dir.join(source);
                if !src_path.exists() {
                    return Err(ForgeError::OverlaySourceMissing {
                        path: src_path.display().to_string(),
                    });
                }
                info!(
                    source = %src_path.display(),
                    destination = %dest.display(),
                    "Copying overlay file"
                );
                std::fs::copy(&src_path, &dest).map_err(|e| ForgeError::Overlay {
                    action: format!("copy {} -> {}", source, file_overlay.destination),
                    detail: String::new(),
                    source: e,
                })?;
            } else {
                // Create empty file
                info!(destination = %dest.display(), "Creating empty overlay file");
                std::fs::write(&dest, b"").map_err(|e| ForgeError::Overlay {
                    action: format!("create empty file {}", file_overlay.destination),
                    detail: String::new(),
                    source: e,
                })?;
            }

            // Set permissions if specified
            #[cfg(unix)]
            if let Some(ref mode) = file_overlay.mode {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mode_val) = u32::from_str_radix(mode, 8) {
                    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(mode_val))
                        .map_err(|e| ForgeError::Overlay {
                            action: format!("set mode {} on {}", mode, file_overlay.destination),
                            detail: String::new(),
                            source: e,
                        })?;
                }
            }
        }

        OverlayAction::Devfsadm(_) => {
            let root_str = staging_root.to_str().unwrap_or(".");
            crate::tools::devfsadm::run_devfsadm(runner, root_str).await?;
        }

        OverlayAction::EnsureDir(ensure_dir) => {
            let dir_path = staging_root.join(
                ensure_dir
                    .path
                    .strip_prefix('/')
                    .unwrap_or(&ensure_dir.path),
            );
            info!(path = %dir_path.display(), "Ensuring directory exists");
            std::fs::create_dir_all(&dir_path).map_err(|e| ForgeError::Overlay {
                action: format!("ensure directory {}", ensure_dir.path),
                detail: String::new(),
                source: e,
            })?;

            #[cfg(unix)]
            if let Some(ref mode) = ensure_dir.mode {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mode_val) = u32::from_str_radix(mode, 8) {
                    std::fs::set_permissions(
                        &dir_path,
                        std::fs::Permissions::from_mode(mode_val),
                    )
                    .map_err(|e| ForgeError::Overlay {
                        action: format!("set mode {} on {}", mode, ensure_dir.path),
                        detail: String::new(),
                        source: e,
                    })?;
                }
            }
        }

        OverlayAction::RemoveFiles(remove) => {
            if let Some(ref file) = remove.file {
                let path = staging_root.join(file.strip_prefix('/').unwrap_or(file));
                if path.exists() {
                    info!(path = %path.display(), "Removing file");
                    std::fs::remove_file(&path).map_err(|e| ForgeError::Overlay {
                        action: format!("remove file {file}"),
                        detail: String::new(),
                        source: e,
                    })?;
                }
            }
            if let Some(ref dir) = remove.dir {
                let path = staging_root.join(dir.strip_prefix('/').unwrap_or(dir));
                if path.exists() {
                    info!(path = %path.display(), "Removing directory contents");
                    // Remove directory contents but not the directory itself
                    for entry in std::fs::read_dir(&path).map_err(|e| ForgeError::Overlay {
                        action: format!("read directory {dir}"),
                        detail: String::new(),
                        source: e,
                    })? {
                        let entry = entry.map_err(|e| ForgeError::Overlay {
                            action: format!("read entry in {dir}"),
                            detail: String::new(),
                            source: e,
                        })?;
                        let entry_path = entry.path();
                        if entry_path.is_dir() {
                            std::fs::remove_dir_all(&entry_path).map_err(|e| {
                                ForgeError::Overlay {
                                    action: format!(
                                        "remove dir {}",
                                        entry_path.display()
                                    ),
                                    detail: String::new(),
                                    source: e,
                                }
                            })?;
                        } else {
                            std::fs::remove_file(&entry_path).map_err(|e| {
                                ForgeError::Overlay {
                                    action: format!(
                                        "remove file {}",
                                        entry_path.display()
                                    ),
                                    detail: String::new(),
                                    source: e,
                                }
                            })?;
                        }
                    }
                }
            }
            if let Some(ref pattern) = remove.pattern {
                info!(pattern, "Removing files matching pattern");
                // Simple glob-like pattern matching: only supports trailing *
                let base = staging_root.join(
                    pattern
                        .trim_end_matches('*')
                        .strip_prefix('/')
                        .unwrap_or(pattern.trim_end_matches('*')),
                );
                if let Some(parent) = base.parent() {
                    if parent.exists() {
                        for entry in std::fs::read_dir(parent).map_err(|e| {
                            ForgeError::Overlay {
                                action: format!("read directory for pattern {pattern}"),
                                detail: String::new(),
                                source: e,
                            }
                        })? {
                            let entry = entry.map_err(|e| ForgeError::Overlay {
                                action: format!("read entry for pattern {pattern}"),
                                detail: String::new(),
                                source: e,
                            })?;
                            let name = entry.file_name().to_string_lossy().to_string();
                            let base_name = base
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if name.starts_with(&base_name) {
                                let entry_path = entry.path();
                                if entry_path.is_file() {
                                    std::fs::remove_file(&entry_path).ok();
                                }
                            }
                        }
                    }
                }
            }
        }

        OverlayAction::EnsureSymlink(symlink) => {
            let link_path = staging_root.join(
                symlink
                    .path
                    .strip_prefix('/')
                    .unwrap_or(&symlink.path),
            );

            // Ensure parent directory exists
            if let Some(parent) = link_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ForgeError::Overlay {
                    action: format!("create parent dir for symlink {}", symlink.path),
                    detail: String::new(),
                    source: e,
                })?;
            }

            // Remove existing file/symlink if present
            if link_path.exists() || link_path.symlink_metadata().is_ok() {
                std::fs::remove_file(&link_path).ok();
            }

            info!(
                link = %link_path.display(),
                target = %symlink.target,
                "Creating symlink"
            );

            #[cfg(unix)]
            std::os::unix::fs::symlink(&symlink.target, &link_path).map_err(|e| {
                ForgeError::Overlay {
                    action: format!("create symlink {} -> {}", symlink.path, symlink.target),
                    detail: String::new(),
                    source: e,
                }
            })?;

            #[cfg(not(unix))]
            return Err(ForgeError::Overlay {
                action: format!("create symlink {} -> {}", symlink.path, symlink.target),
                detail: "Symlinks are only supported on Unix platforms".to_string(),
                source: std::io::Error::new(std::io::ErrorKind::Unsupported, "not unix"),
            });
        }

        OverlayAction::Shadow(shadow) => {
            let shadow_path = staging_root.join("etc/shadow");
            if shadow_path.exists() {
                info!(username = %shadow.username, "Updating shadow password");
                let content = std::fs::read_to_string(&shadow_path).map_err(|e| {
                    ForgeError::Overlay {
                        action: format!("read /etc/shadow for user {}", shadow.username),
                        detail: String::new(),
                        source: e,
                    }
                })?;

                let mut found = false;
                let new_content: String = content
                    .lines()
                    .map(|line| {
                        let parts: Vec<&str> = line.splitn(9, ':').collect();
                        if parts.len() >= 2 && parts[0] == shadow.username {
                            found = true;
                            let mut new_parts = parts.clone();
                            new_parts[1] = &shadow.password;
                            new_parts.join(":")
                        } else {
                            line.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                let final_content = if found {
                    new_content
                } else {
                    format!("{new_content}\n{}:{}:::::::\n", shadow.username, shadow.password)
                };

                std::fs::write(&shadow_path, final_content).map_err(|e| ForgeError::Overlay {
                    action: format!("write /etc/shadow for user {}", shadow.username),
                    detail: String::new(),
                    source: e,
                })?;
            } else {
                // Create shadow file with this entry
                let content = format!("{}:{}:::::::\n", shadow.username, shadow.password);
                let etc_dir = staging_root.join("etc");
                std::fs::create_dir_all(&etc_dir).map_err(|e| ForgeError::Overlay {
                    action: "create /etc for shadow".to_string(),
                    detail: String::new(),
                    source: e,
                })?;
                std::fs::write(&shadow_path, content).map_err(|e| ForgeError::Overlay {
                    action: format!("create /etc/shadow for user {}", shadow.username),
                    detail: String::new(),
                    source: e,
                })?;
            }
        }
    }

    Ok(())
}
