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

    // Append to /etc/passwd
    let passwd_path = etc_dir.join("passwd");
    let passwd_entry = format!("{username}:x:1000:1000::/home/{username}:/bin/sh\n");
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
    let group_path = etc_dir.join("group");
    let group_entry = format!("{username}::1000:\n");
    append_or_create(&group_path, &group_entry).map_err(|e| ForgeError::Customization {
        operation: format!("add group {username} to /etc/group"),
        detail: e.to_string(),
    })?;

    Ok(())
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
