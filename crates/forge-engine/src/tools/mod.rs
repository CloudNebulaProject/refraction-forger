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
use crate::output::OutputHandler;

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

/// Real tool runner that streams stdout/stderr line-by-line through an
/// [`OutputHandler`] instead of buffering all output until completion.
pub struct SystemToolRunner {
    output: OutputHandler,
}

impl SystemToolRunner {
    pub fn new(output: OutputHandler) -> Self {
        Self { output }
    }
}

impl ToolRunner for SystemToolRunner {
    fn run<'a>(
        &'a self,
        program: &'a str,
        args: &'a [&'a str],
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ForgeError>> + Send + 'a>> {
        Box::pin(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            use tokio::process::Command;

            self.output.tool_start(program, args);

            let mut child = Command::new(program)
                .args(args)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| ForgeError::ToolExecution {
                    tool: program.to_string(),
                    args: args.join(" "),
                    stderr: String::new(),
                    source: e,
                })?;

            let stdout_pipe = child.stdout.take();
            let stderr_pipe = child.stderr.take();

            let mut stdout_lines = Vec::new();
            let mut stderr_lines = Vec::new();

            // Read stdout and stderr concurrently, streaming each line
            let tool_name = program.to_string();
            let output_handle = self.output.clone();

            let stdout_task = {
                let tool_name = tool_name.clone();
                let output_handle = output_handle.clone();
                tokio::spawn(async move {
                    let mut lines = Vec::new();
                    if let Some(pipe) = stdout_pipe {
                        let reader = BufReader::new(pipe);
                        let mut line_reader = reader.lines();
                        while let Ok(Some(line)) = line_reader.next_line().await {
                            output_handle.tool_output(&tool_name, "stdout", &line);
                            lines.push(line);
                        }
                    }
                    lines
                })
            };

            let stderr_task = {
                let tool_name = tool_name.clone();
                let output_handle = output_handle.clone();
                tokio::spawn(async move {
                    let mut lines = Vec::new();
                    if let Some(pipe) = stderr_pipe {
                        let reader = BufReader::new(pipe);
                        let mut line_reader = reader.lines();
                        while let Ok(Some(line)) = line_reader.next_line().await {
                            output_handle.tool_output(&tool_name, "stderr", &line);
                            lines.push(line);
                        }
                    }
                    lines
                })
            };

            // Wait for streams to drain and process to exit
            if let Ok(lines) = stdout_task.await {
                stdout_lines = lines;
            }
            if let Ok(lines) = stderr_task.await {
                stderr_lines = lines;
            }

            let status = child.wait().await.map_err(|e| ForgeError::ToolExecution {
                tool: program.to_string(),
                args: args.join(" "),
                stderr: String::new(),
                source: e,
            })?;

            let exit_code = status.code().unwrap_or(-1);
            self.output.tool_done(program, exit_code);

            let result = ToolOutput {
                stdout: stdout_lines.join("\n"),
                stderr: stderr_lines.join("\n"),
                exit_code,
            };

            if !status.success() {
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
