pub mod apt;
pub mod bootloader;
pub mod devfsadm;
pub mod loopback;
pub mod partition;
pub mod pkg;
pub mod qemu_img;
pub mod zfs;
pub mod zpool;

use std::future::Future;
use std::pin::Pin;

use crate::error::ForgeError;

/// Output from a tool execution.
#[derive(Debug)]
pub struct ToolOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Trait for executing external tools. Allows mocking in tests.
pub trait ToolRunner: Send + Sync {
    fn run<'a>(
        &'a self,
        program: &'a str,
        args: &'a [&'a str],
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ForgeError>> + Send + 'a>>;
}

/// Real tool runner that uses `tokio::process::Command`.
pub struct SystemToolRunner;

impl ToolRunner for SystemToolRunner {
    fn run<'a>(
        &'a self,
        program: &'a str,
        args: &'a [&'a str],
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ForgeError>> + Send + 'a>> {
        Box::pin(async move {
            let output = tokio::process::Command::new(program)
                .args(args)
                .output()
                .await
                .map_err(|e| ForgeError::ToolExecution {
                    tool: program.to_string(),
                    args: args.join(" "),
                    stderr: String::new(),
                    source: e,
                })?;

            let result = ToolOutput {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
            };

            if !output.status.success() {
                return Err(ForgeError::ToolNonZero {
                    tool: program.to_string(),
                    args: args.join(" "),
                    exit_code: result.exit_code,
                    stderr: result.stderr,
                });
            }

            Ok(result)
        })
    }
}
