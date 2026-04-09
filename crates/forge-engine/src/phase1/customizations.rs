use std::path::Path;

use spec_parser::schema::Customization;
use tracing::info;

use crate::error::ForgeError;

/// Apply customizations (user/group creation) by editing files in the staging root.
pub fn apply(customization: &Customization, staging_root: &Path) -> Result<(), ForgeError> {
    for user in &customization.users {
        create_user(&user.name, staging_root)?;
    }
    Ok(())
}

/// Create a user by appending entries to passwd, shadow, and group files in
/// the staging root. This does not use `useradd` since we're operating on a
/// staged filesystem, not the running system.
fn create_user(username: &str, staging_root: &Path) -> Result<(), ForgeError> {
    info!(username, "Creating user in staging root");

    let etc_dir = staging_root.join("etc");
    std::fs::create_dir_all(&etc_dir).map_err(|e| ForgeError::Customization {
        operation: format!("create /etc directory for user {username}"),
        detail: e.to_string(),
    })?;

    let passwd_path = etc_dir.join("passwd");
    let group_path = etc_dir.join("group");

    // Find the next available UID/GID (start at 1000, scan existing files)
    let next_uid = next_id_from_file(&passwd_path, 2);
    let next_gid = next_id_from_file(&group_path, 2);

    // Append to /etc/passwd
    let passwd_entry = format!("{username}:x:{next_uid}:{next_gid}::/home/{username}:/bin/sh\n");
    append_or_create(&passwd_path, &passwd_entry).map_err(|e| ForgeError::Customization {
        operation: format!("add user {username} to /etc/passwd"),
        detail: e.to_string(),
    })?;

    // Append to /etc/shadow
    let shadow_path = etc_dir.join("shadow");
    let shadow_entry = format!("{username}:*LK*:::::::\n");
    append_or_create(&shadow_path, &shadow_entry).map_err(|e| ForgeError::Customization {
        operation: format!("add user {username} to /etc/shadow"),
        detail: e.to_string(),
    })?;

    // Append to /etc/group
    let group_entry = format!("{username}::{next_gid}:\n");
    append_or_create(&group_path, &group_entry).map_err(|e| ForgeError::Customization {
        operation: format!("add group {username} to /etc/group"),
        detail: e.to_string(),
    })?;

    Ok(())
}

/// Scan a colon-delimited file (passwd or group) and return the next available
/// ID. The `id_field` parameter is the 0-based column index containing the
/// numeric ID (2 for passwd UID, 2 for group GID). Returns at least 1000.
fn next_id_from_file(path: &Path, id_field: usize) -> u32 {
    const MIN_ID: u32 = 1000;

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return MIN_ID,
    };

    let max_id = content
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(':').collect();
            fields.get(id_field)?.parse::<u32>().ok()
        })
        .filter(|&id| id >= MIN_ID)
        .max();

    match max_id {
        Some(id) => id + 1,
        None => MIN_ID,
    }
}

fn append_or_create(path: &Path, content: &str) -> Result<(), std::io::Error> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use spec_parser::schema::User;
    use tempfile::TempDir;

    #[test]
    fn test_create_single_user() {
        let staging = TempDir::new().unwrap();

        let customization = Customization {
            r#if: None,
            users: vec![User {
                name: "testuser".to_string(),
            }],
        };

        apply(&customization, staging.path()).unwrap();

        let passwd = std::fs::read_to_string(staging.path().join("etc/passwd")).unwrap();
        assert!(passwd.contains("testuser:x:1000:1000::/home/testuser:/bin/sh"));

        let shadow = std::fs::read_to_string(staging.path().join("etc/shadow")).unwrap();
        assert!(shadow.contains("testuser:*LK*:::::::"));

        let group = std::fs::read_to_string(staging.path().join("etc/group")).unwrap();
        assert!(group.contains("testuser::1000:"));
    }

    #[test]
    fn test_create_multiple_users() {
        let staging = TempDir::new().unwrap();

        let customization = Customization {
            r#if: None,
            users: vec![
                User {
                    name: "alice".to_string(),
                },
                User {
                    name: "bob".to_string(),
                },
            ],
        };

        apply(&customization, staging.path()).unwrap();

        let passwd = std::fs::read_to_string(staging.path().join("etc/passwd")).unwrap();
        assert!(passwd.contains("alice:x:1000:1000::/home/alice:/bin/sh"));
        assert!(passwd.contains("bob:x:1001:1001::/home/bob:/bin/sh"));

        let group = std::fs::read_to_string(staging.path().join("etc/group")).unwrap();
        assert!(passwd.contains("alice"));
        assert!(group.contains("alice::1000:"));
        assert!(group.contains("bob::1001:"));
    }

    #[test]
    fn test_create_user_appends_to_existing() {
        let staging = TempDir::new().unwrap();

        // Create pre-existing /etc/passwd
        std::fs::create_dir_all(staging.path().join("etc")).unwrap();
        std::fs::write(
            staging.path().join("etc/passwd"),
            "root:x:0:0:root:/root:/bin/sh\n",
        )
        .unwrap();

        let customization = Customization {
            r#if: None,
            users: vec![User {
                name: "admin".to_string(),
            }],
        };

        apply(&customization, staging.path()).unwrap();

        let passwd = std::fs::read_to_string(staging.path().join("etc/passwd")).unwrap();
        assert!(passwd.contains("root:x:0:0:root:/root:/bin/sh"));
        assert!(passwd.contains("admin:x:1000:1000::/home/admin:/bin/sh"));
    }

    #[test]
    fn test_no_users_is_noop() {
        let staging = TempDir::new().unwrap();

        let customization = Customization {
            r#if: None,
            users: vec![],
        };

        apply(&customization, staging.path()).unwrap();

        // etc directory should not have been created
        assert!(!staging.path().join("etc").exists());
    }
}
