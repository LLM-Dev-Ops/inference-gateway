//! File I/O operations for benchmark results.

use crate::BenchmarkResult;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Write raw benchmark results to JSON files in the output directory.
///
/// Each result is written to a separate file named `{target_id}.json` in the
/// `raw/` subdirectory of the output directory.
///
/// # Arguments
///
/// * `results` - Slice of benchmark results to write
/// * `output_dir` - Base output directory (e.g., `benchmarks/output/`)
pub fn write_raw_results(results: &[BenchmarkResult], output_dir: &Path) -> Result<()> {
    let raw_dir = output_dir.join("raw");
    fs::create_dir_all(&raw_dir).context("Failed to create raw output directory")?;

    for result in results {
        let filename = format!("{}.json", sanitize_filename(&result.target_id));
        let filepath = raw_dir.join(&filename);

        let json = serde_json::to_string_pretty(result)
            .context(format!("Failed to serialize result for {}", result.target_id))?;

        fs::write(&filepath, json).context(format!("Failed to write {}", filepath.display()))?;
    }

    // Also write a combined results file
    let all_results_path = output_dir.join("all_results.json");
    let all_json =
        serde_json::to_string_pretty(results).context("Failed to serialize all results")?;
    fs::write(&all_results_path, all_json)
        .context(format!("Failed to write {}", all_results_path.display()))?;

    Ok(())
}

/// Write the summary markdown report to the output directory.
///
/// # Arguments
///
/// * `summary` - The markdown summary content
/// * `output_dir` - Base output directory (e.g., `benchmarks/output/`)
pub fn write_summary(summary: &str, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    let summary_path = output_dir.join("summary.md");
    fs::write(&summary_path, summary)
        .context(format!("Failed to write {}", summary_path.display()))?;

    Ok(())
}

/// Read all raw benchmark results from the output directory.
///
/// # Arguments
///
/// * `output_dir` - Base output directory (e.g., `benchmarks/output/`)
///
/// # Returns
///
/// A vector of all benchmark results found in the raw directory.
pub fn read_raw_results(output_dir: &Path) -> Result<Vec<BenchmarkResult>> {
    let raw_dir = output_dir.join("raw");

    if !raw_dir.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    for entry in fs::read_dir(&raw_dir).context("Failed to read raw directory")? {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "json") {
            let content = fs::read_to_string(&path)
                .context(format!("Failed to read {}", path.display()))?;

            let result: BenchmarkResult = serde_json::from_str(&content)
                .context(format!("Failed to parse {}", path.display()))?;

            results.push(result);
        }
    }

    // Sort by target_id for consistent ordering
    results.sort_by(|a, b| a.target_id.cmp(&b.target_id));

    Ok(results)
}

/// Read the combined results file if it exists.
///
/// # Arguments
///
/// * `output_dir` - Base output directory (e.g., `benchmarks/output/`)
pub fn read_all_results(output_dir: &Path) -> Result<Vec<BenchmarkResult>> {
    let all_results_path = output_dir.join("all_results.json");

    if !all_results_path.exists() {
        return read_raw_results(output_dir);
    }

    let content = fs::read_to_string(&all_results_path)
        .context(format!("Failed to read {}", all_results_path.display()))?;

    let results: Vec<BenchmarkResult> = serde_json::from_str(&content)
        .context(format!("Failed to parse {}", all_results_path.display()))?;

    Ok(results)
}

/// Sanitize a string for use as a filename.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Ensure the output directory structure exists.
///
/// Creates:
/// - `{output_dir}/`
/// - `{output_dir}/raw/`
pub fn ensure_output_dirs(output_dir: &Path) -> Result<()> {
    let raw_dir = output_dir.join("raw");
    fs::create_dir_all(&raw_dir).context("Failed to create output directories")?;
    Ok(())
}

/// Clean the output directory by removing all files.
///
/// # Safety
///
/// This will delete all files in the output directory. Use with caution.
pub fn clean_output_dir(output_dir: &Path) -> Result<()> {
    if output_dir.exists() {
        fs::remove_dir_all(output_dir).context("Failed to remove output directory")?;
    }
    ensure_output_dirs(output_dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_and_read_results() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let output_dir = temp_dir.path();

        let results = vec![
            BenchmarkResult::new("test_a", serde_json::json!({"value": 1})),
            BenchmarkResult::new("test_b", serde_json::json!({"value": 2})),
        ];

        write_raw_results(&results, output_dir).expect("Failed to write results");

        let read_results = read_raw_results(output_dir).expect("Failed to read results");
        assert_eq!(read_results.len(), 2);
        assert_eq!(read_results[0].target_id, "test_a");
        assert_eq!(read_results[1].target_id, "test_b");
    }

    #[test]
    fn test_write_summary() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let output_dir = temp_dir.path();

        write_summary("# Test Summary", output_dir).expect("Failed to write summary");

        let summary_path = output_dir.join("summary.md");
        assert!(summary_path.exists());

        let content = fs::read_to_string(summary_path).expect("Failed to read summary");
        assert_eq!(content, "# Test Summary");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("test_name"), "test_name");
        assert_eq!(sanitize_filename("test/name"), "test_name");
        assert_eq!(sanitize_filename("test:name"), "test_name");
        assert_eq!(sanitize_filename("Test Name"), "Test_Name");
    }

    #[test]
    fn test_ensure_output_dirs() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let output_dir = temp_dir.path().join("benchmarks/output");

        ensure_output_dirs(&output_dir).expect("Failed to ensure dirs");

        assert!(output_dir.exists());
        assert!(output_dir.join("raw").exists());
    }
}
