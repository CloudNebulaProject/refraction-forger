use std::path::Path;

use crate::error::ForgeError;
use crate::tools::ToolRunner;
use tracing::info;

/// Bootstrap a minimal Debian/Ubuntu rootfs using debootstrap.
pub async fn debootstrap(
    runner: &dyn ToolRunner,
    suite: &str,
    root: &str,
    mirror: &str,
    components: Option<&str>,
) -> Result<(), ForgeError> {
    info!(suite, root, mirror, ?components, "Running debootstrap");
    let mut args = vec!["--arch", "amd64"];
    // Format: --components=main,universe (comma-separated)
    let comp_arg;
    if let Some(c) = components {
        comp_arg = format!("--components={}", c.replace(' ', ","));
        args.push(&comp_arg);
    }
    args.extend_from_slice(&[suite, root, mirror]);
    runner.run("debootstrap", &args).await?;
    Ok(())
}

/// Run `apt-get update` inside the chroot.
pub async fn update(runner: &dyn ToolRunner, root: &str) -> Result<(), ForgeError> {
    info!(root, "Running apt-get update in chroot");
    runner
        .run("chroot", &[root, "apt-get", "update", "-y"])
        .await?;
    Ok(())
}

/// Install packages inside the chroot using apt-get.
pub async fn install(
    runner: &dyn ToolRunner,
    root: &str,
    packages: &[String],
) -> Result<(), ForgeError> {
    if packages.is_empty() {
        return Ok(());
    }
    info!(root, count = packages.len(), "Installing packages via apt-get");
    let mut args = vec![root, "apt-get", "install", "-y", "--no-install-recommends"];
    let pkg_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
    args.extend(pkg_refs);
    runner.run("chroot", &args).await?;
    Ok(())
}

/// Write the primary sources.list in the chroot, replacing the debootstrap default.
pub async fn write_sources_list(
    root: &str,
    entries: &[String],
) -> Result<(), ForgeError> {
    let sources_path = Path::new(root).join("etc/apt/sources.list");
    info!(?sources_path, count = entries.len(), "Writing sources.list");
    let content = entries.join("\n") + "\n";
    std::fs::write(&sources_path, content).map_err(|e| ForgeError::Overlay {
        action: "write sources.list".to_string(),
        detail: sources_path.display().to_string(),
        source: e,
    })?;
    Ok(())
}

/// Add an APT source entry to the chroot's sources.list.d/.
pub async fn add_source(
    runner: &dyn ToolRunner,
    root: &str,
    entry: &str,
) -> Result<(), ForgeError> {
    info!(root, entry, "Adding APT source");
    let list_path = Path::new(root)
        .join("etc/apt/sources.list.d/extra.list");
    let list_str = list_path.to_str().unwrap_or("extra.list");

    // Append entry to the sources list file
    runner
        .run("sh", &["-c", &format!("echo '{entry}' >> {list_str}")])
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ToolOutput, ToolRunner};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;

    struct MockToolRunner {
        calls: Mutex<Vec<(String, Vec<String>)>>,
    }

    impl MockToolRunner {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<(String, Vec<String>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl ToolRunner for MockToolRunner {
        fn run<'a>(
            &'a self,
            program: &'a str,
            args: &'a [&'a str],
        ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ForgeError>> + Send + 'a>> {
            self.calls.lock().unwrap().push((
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
            ));
            Box::pin(async {
                Ok(ToolOutput {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 0,
                })
            })
        }
    }

    #[tokio::test]
    async fn test_debootstrap_args() {
        let runner = MockToolRunner::new();
        debootstrap(&runner, "jammy", "/tmp/root", "http://archive.ubuntu.com/ubuntu", None)
            .await
            .unwrap();

        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "debootstrap");
        assert_eq!(
            calls[0].1,
            vec!["--arch", "amd64", "jammy", "/tmp/root", "http://archive.ubuntu.com/ubuntu"]
        );
    }

    #[tokio::test]
    async fn test_debootstrap_with_components() {
        let runner = MockToolRunner::new();
        debootstrap(&runner, "jammy", "/tmp/root", "http://archive.ubuntu.com/ubuntu", Some("main universe"))
            .await
            .unwrap();

        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "debootstrap");
        assert_eq!(
            calls[0].1,
            vec!["--arch", "amd64", "--components=main,universe", "jammy", "/tmp/root", "http://archive.ubuntu.com/ubuntu"]
        );
    }

    #[tokio::test]
    async fn test_install_args() {
        let runner = MockToolRunner::new();
        let packages = vec!["curl".to_string(), "git".to_string()];
        install(&runner, "/tmp/root", &packages).await.unwrap();

        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "chroot");
        assert_eq!(
            calls[0].1,
            vec!["/tmp/root", "apt-get", "install", "-y", "--no-install-recommends", "curl", "git"]
        );
    }

    #[tokio::test]
    async fn test_install_empty() {
        let runner = MockToolRunner::new();
        install(&runner, "/tmp/root", &[]).await.unwrap();
        assert!(runner.calls().is_empty());
    }

    #[tokio::test]
    async fn test_update_args() {
        let runner = MockToolRunner::new();
        update(&runner, "/tmp/root").await.unwrap();

        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "chroot");
        assert_eq!(calls[0].1, vec!["/tmp/root", "apt-get", "update", "-y"]);
    }
}
