//! Output formatting utilities for the CLI.

use colored::Colorize;
use serde::Serialize;
use std::io::{self, Write};

/// Output format for CLI results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text output.
    Text,
    /// JSON output.
    Json,
}

impl OutputFormat {
    /// Create an output format from a JSON flag.
    pub fn from_json_flag(json: bool) -> Self {
        if json {
            Self::Json
        } else {
            Self::Text
        }
    }
}

/// Print a success message.
pub fn success(message: &str) {
    println!("{} {}", "✓".green().bold(), message);
}

/// Print an error message.
pub fn error(message: &str) {
    eprintln!("{} {}", "✗".red().bold(), message);
}

/// Print a warning message.
pub fn warning(message: &str) {
    eprintln!("{} {}", "⚠".yellow().bold(), message);
}

/// Print an info message.
pub fn info(message: &str) {
    println!("{} {}", "ℹ".blue().bold(), message);
}

/// Print a key-value pair.
pub fn key_value(key: &str, value: &str) {
    println!("  {}: {}", key.bold(), value);
}

/// Print a section header.
pub fn section(title: &str) {
    println!("\n{}", title.bold().underline());
}

/// Print a status indicator.
pub fn status(label: &str, ok: bool) {
    let indicator = if ok {
        "●".green()
    } else {
        "●".red()
    };
    println!("  {} {}", indicator, label);
}

/// Print JSON output.
pub fn json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    let output = serde_json::to_string_pretty(value)?;
    println!("{}", output);
    Ok(())
}

/// Print compact JSON output.
pub fn json_compact<T: Serialize>(value: &T) -> anyhow::Result<()> {
    let output = serde_json::to_string(value)?;
    println!("{}", output);
    Ok(())
}

/// Create a spinner for long-running operations.
pub fn spinner(message: &str) -> indicatif::ProgressBar {
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_style(
        indicatif::ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.blue} {msg}")
            .expect("valid template"),
    );
    spinner.set_message(message.to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    spinner
}

/// Print a table of data.
pub fn table<T: tabled::Tabled>(data: &[T]) {
    use tabled::{settings::Style, Table};

    if data.is_empty() {
        println!("  (no data)");
        return;
    }

    let table = Table::new(data).with(Style::rounded()).to_string();
    println!("{}", table);
}

/// Print streaming text output.
pub fn stream_text(text: &str) {
    print!("{}", text);
    io::stdout().flush().ok();
}

/// Print a newline for streaming output.
pub fn stream_newline() {
    println!();
}

/// Format bytes as a human-readable size.
pub fn format_bytes(bytes: u64) -> String {
    bytesize::ByteSize(bytes).to_string()
}

/// Format a duration as a human-readable string.
pub fn format_duration(duration: std::time::Duration) -> String {
    humantime::format_duration(duration).to_string()
}

/// Format a timestamp as a human-readable string.
pub fn format_timestamp(timestamp: i64) -> String {
    use chrono::{TimeZone, Utc};
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Result output that can be formatted as text or JSON.
#[derive(Debug, Serialize)]
pub struct CommandResult<T: Serialize> {
    /// Whether the command succeeded.
    pub success: bool,
    /// Result data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Informational message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T: Serialize> CommandResult<T> {
    /// Create a successful result with data.
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            message: None,
        }
    }

    /// Create a successful result with a message.
    pub fn success_message(message: impl Into<String>) -> Self {
        Self {
            success: true,
            data: None,
            error: None,
            message: Some(message.into()),
        }
    }

    /// Create a failed result.
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
            message: None,
        }
    }

    /// Print the result in the specified format.
    pub fn print(&self, format: OutputFormat) -> anyhow::Result<()> {
        match format {
            OutputFormat::Json => json(self),
            OutputFormat::Text => {
                if let Some(ref err) = self.error {
                    error(err);
                }
                if let Some(ref msg) = self.message {
                    if self.success {
                        success(msg);
                    } else {
                        error(msg);
                    }
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        // Test that bytes are formatted to human-readable strings
        let kb = format_bytes(1024);
        assert!(kb.contains("KB") || kb.contains("KiB"), "Expected KB format: {}", kb);

        let mb = format_bytes(1_048_576);
        assert!(mb.contains("KB") || mb.contains("MB") || mb.contains("MiB"), "Expected MB format: {}", mb);

        // Test zero bytes
        let zero = format_bytes(0);
        assert!(zero.contains("0") || zero.contains("B"), "Expected zero format: {}", zero);
    }

    #[test]
    fn test_format_timestamp() {
        let ts = format_timestamp(1704067200); // 2024-01-01 00:00:00 UTC
        assert!(ts.contains("2024"));
    }

    #[test]
    fn test_command_result_success() {
        let result: CommandResult<String> = CommandResult::success("test".to_string());
        assert!(result.success);
        assert_eq!(result.data, Some("test".to_string()));
    }

    #[test]
    fn test_command_result_failure() {
        let result: CommandResult<()> = CommandResult::failure("error");
        assert!(!result.success);
        assert_eq!(result.error, Some("error".to_string()));
    }
}
