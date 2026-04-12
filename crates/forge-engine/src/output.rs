//! Output formatting for build progress and tool execution.
//!
//! Supports three modes:
//! - **Pretty**: Human-readable with spinners and styled tool output (default)
//! - **Json**: Machine-readable JSON events, one per line (NDJSON)
//! - **Quiet**: Errors only, no progress output

use std::io::Write;
use std::sync::Arc;

use serde::Serialize;

/// Controls how build progress and tool output are displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// Human-readable output with spinners and styled formatting.
    #[default]
    Pretty,
    /// Machine-readable NDJSON (one JSON object per line).
    Json,
    /// Suppress all output except errors.
    Quiet,
}

impl std::str::FromStr for OutputMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pretty" => Ok(OutputMode::Pretty),
            "json" => Ok(OutputMode::Json),
            "quiet" => Ok(OutputMode::Quiet),
            other => Err(format!(
                "unknown output mode '{other}', expected: pretty, json, quiet"
            )),
        }
    }
}

/// A structured event emitted during the build.
#[derive(Debug, Serialize)]
pub struct BuildEvent {
    /// Event type (e.g., "phase_start", "tool_run", "tool_output", "tool_done")
    #[serde(rename = "type")]
    pub event_type: &'static str,
    /// Milliseconds since build started
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
    /// Tool or phase name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Output stream ("stdout" or "stderr")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<&'static str>,
    /// Line of output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    /// Exit code for tool_done events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Additional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Handle for writing formatted build output.
#[derive(Clone)]
pub struct OutputHandler {
    mode: OutputMode,
    start: std::time::Instant,
    writer: Arc<std::sync::Mutex<Box<dyn Write + Send>>>,
}

impl OutputHandler {
    pub fn new(mode: OutputMode) -> Self {
        Self {
            mode,
            start: std::time::Instant::now(),
            writer: Arc::new(std::sync::Mutex::new(Box::new(std::io::stderr()))),
        }
    }

    pub fn mode(&self) -> OutputMode {
        self.mode
    }

    fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Emit a phase start event (e.g., "Phase 1: rootfs assembly").
    pub fn phase_start(&self, name: &str) {
        match self.mode {
            OutputMode::Pretty => {
                let mut w = self.writer.lock().unwrap();
                let _ = writeln!(w, "\x1b[1;36m▸ {name}\x1b[0m");
            }
            OutputMode::Json => {
                self.emit_json(&BuildEvent {
                    event_type: "phase_start",
                    elapsed_ms: Some(self.elapsed_ms()),
                    name: Some(name.to_string()),
                    stream: None,
                    line: None,
                    exit_code: None,
                    message: None,
                });
            }
            OutputMode::Quiet => {}
        }
    }

    /// Emit a phase completion event.
    pub fn phase_done(&self, name: &str) {
        match self.mode {
            OutputMode::Pretty => {
                let mut w = self.writer.lock().unwrap();
                let elapsed = self.elapsed_ms();
                let _ = writeln!(w, "\x1b[1;32m✓ {name}\x1b[0m \x1b[2m({elapsed}ms)\x1b[0m");
            }
            OutputMode::Json => {
                self.emit_json(&BuildEvent {
                    event_type: "phase_done",
                    elapsed_ms: Some(self.elapsed_ms()),
                    name: Some(name.to_string()),
                    stream: None,
                    line: None,
                    exit_code: None,
                    message: None,
                });
            }
            OutputMode::Quiet => {}
        }
    }

    /// Emit tool execution start.
    pub fn tool_start(&self, tool: &str, args: &[&str]) {
        match self.mode {
            OutputMode::Pretty => {
                let mut w = self.writer.lock().unwrap();
                let cmd = format_command(tool, args);
                let _ = writeln!(w, "\x1b[2m  $ {cmd}\x1b[0m");
            }
            OutputMode::Json => {
                self.emit_json(&BuildEvent {
                    event_type: "tool_run",
                    elapsed_ms: Some(self.elapsed_ms()),
                    name: Some(format!("{tool} {}", args.join(" "))),
                    stream: None,
                    line: None,
                    exit_code: None,
                    message: None,
                });
            }
            OutputMode::Quiet => {}
        }
    }

    /// Emit a line of tool output.
    pub fn tool_output(&self, tool: &str, stream: &'static str, line: &str) {
        match self.mode {
            OutputMode::Pretty => {
                let mut w = self.writer.lock().unwrap();
                let prefix = if stream == "stderr" {
                    "\x1b[33m"
                } else {
                    "\x1b[2m"
                };
                let _ = writeln!(w, "  {prefix}│\x1b[0m {line}");
            }
            OutputMode::Json => {
                self.emit_json(&BuildEvent {
                    event_type: "tool_output",
                    elapsed_ms: Some(self.elapsed_ms()),
                    name: Some(tool.to_string()),
                    stream: Some(stream),
                    line: Some(line.to_string()),
                    exit_code: None,
                    message: None,
                });
            }
            OutputMode::Quiet => {}
        }
    }

    /// Emit tool completion.
    pub fn tool_done(&self, tool: &str, exit_code: i32) {
        match self.mode {
            OutputMode::Pretty => {
                if exit_code != 0 {
                    let mut w = self.writer.lock().unwrap();
                    let _ = writeln!(
                        w,
                        "\x1b[1;31m  ✗ {tool} exited with code {exit_code}\x1b[0m"
                    );
                }
            }
            OutputMode::Json => {
                self.emit_json(&BuildEvent {
                    event_type: "tool_done",
                    elapsed_ms: Some(self.elapsed_ms()),
                    name: Some(tool.to_string()),
                    stream: None,
                    line: None,
                    exit_code: Some(exit_code),
                    message: None,
                });
            }
            OutputMode::Quiet => {}
        }
    }

    /// Emit an informational message.
    pub fn info(&self, message: &str) {
        match self.mode {
            OutputMode::Pretty => {
                let mut w = self.writer.lock().unwrap();
                let _ = writeln!(w, "  \x1b[2m{message}\x1b[0m");
            }
            OutputMode::Json => {
                self.emit_json(&BuildEvent {
                    event_type: "info",
                    elapsed_ms: Some(self.elapsed_ms()),
                    name: None,
                    stream: None,
                    line: None,
                    exit_code: None,
                    message: Some(message.to_string()),
                });
            }
            OutputMode::Quiet => {}
        }
    }

    fn emit_json(&self, event: &BuildEvent) {
        if let Ok(json) = serde_json::to_string(event) {
            let mut w = self.writer.lock().unwrap();
            let _ = writeln!(w, "{json}");
        }
    }
}

/// Format a command and args for display, truncating very long arg lists.
fn format_command(tool: &str, args: &[&str]) -> String {
    let full = format!("{tool} {}", args.join(" "));
    if full.len() > 200 {
        format!("{}… ({} args)", &full[..197], args.len())
    } else {
        full
    }
}
