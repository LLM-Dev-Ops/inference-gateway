//! Benchmark command for running performance benchmarks.

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use gateway_benchmarks::{
    all_targets, io, markdown, run_all_benchmarks, BenchmarkResult,
};
use std::path::PathBuf;
use tabled::{Table, Tabled};

/// Arguments for the benchmark command.
#[derive(Args, Debug)]
pub struct BenchmarkArgs {
    /// Benchmark subcommand
    #[command(subcommand)]
    pub command: BenchmarkCommand,
}

/// Benchmark subcommands.
#[derive(clap::Subcommand, Debug)]
pub enum BenchmarkCommand {
    /// Run benchmarks
    Run(RunArgs),
    /// List available benchmark targets
    List(ListArgs),
    /// Show results from previous benchmark run
    Results(ResultsArgs),
}

/// Arguments for running benchmarks.
#[derive(Args, Debug)]
pub struct RunArgs {
    /// Specific benchmark target to run (runs all if not specified)
    #[arg(short, long)]
    pub target: Option<String>,

    /// Output directory for results
    #[arg(short, long, default_value = "benchmarks/output")]
    pub output: PathBuf,

    /// Clean output directory before running
    #[arg(long)]
    pub clean: bool,

    /// Output results in JSON format
    #[arg(long)]
    pub json: bool,
}

/// Arguments for listing benchmarks.
#[derive(Args, Debug)]
pub struct ListArgs {
    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

/// Arguments for showing results.
#[derive(Args, Debug)]
pub struct ResultsArgs {
    /// Output directory containing results
    #[arg(short, long, default_value = "benchmarks/output")]
    pub output: PathBuf,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

/// Table row for benchmark results.
#[derive(Tabled)]
struct BenchmarkRow {
    #[tabled(rename = "Target")]
    target: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Latency (ms)")]
    latency: String,
    #[tabled(rename = "Throughput (rps)")]
    throughput: String,
    #[tabled(rename = "p99 (ms)")]
    p99: String,
}

/// Table row for listing benchmarks.
#[derive(Tabled)]
struct BenchmarkInfo {
    #[tabled(rename = "Target ID")]
    id: String,
    #[tabled(rename = "Description")]
    description: String,
    #[tabled(rename = "Iterations")]
    iterations: String,
}

/// Execute the benchmark command.
pub async fn execute(args: BenchmarkArgs, json: bool) -> Result<()> {
    match args.command {
        BenchmarkCommand::Run(run_args) => execute_run(run_args, json).await,
        BenchmarkCommand::List(list_args) => execute_list(list_args, json),
        BenchmarkCommand::Results(results_args) => execute_results(results_args, json),
    }
}

/// Execute the run subcommand.
async fn execute_run(args: RunArgs, global_json: bool) -> Result<()> {
    let json = args.json || global_json;

    if !json {
        println!("{}", "Running benchmarks...".cyan().bold());
        println!();
    }

    // Clean output directory if requested
    if args.clean {
        io::clean_output_dir(&args.output)?;
        if !json {
            println!("Cleaned output directory: {}", args.output.display());
        }
    }

    // Ensure output directories exist
    io::ensure_output_dirs(&args.output)?;

    // Run benchmarks
    let results: Vec<BenchmarkResult> = if let Some(target_id) = &args.target {
        // Run specific benchmark
        if let Some(target) = gateway_benchmarks::adapters::get_target(target_id) {
            if !json {
                println!("Running benchmark: {}", target_id.yellow());
            }
            match target.run().await {
                Ok(result) => vec![result],
                Err(e) => {
                    vec![BenchmarkResult::new(
                        target_id.clone(),
                        serde_json::json!({
                            "error": e.to_string(),
                            "status": "failed"
                        }),
                    )]
                }
            }
        } else {
            anyhow::bail!("Unknown benchmark target: {}", target_id);
        }
    } else {
        // Run all benchmarks
        let targets = all_targets();
        if !json {
            println!("Running {} benchmarks...", targets.len());
            println!();
        }

        run_all_benchmarks().await
    };

    // Write results
    io::write_raw_results(&results, &args.output)?;
    let summary = markdown::generate_summary(&results);
    io::write_summary(&summary, &args.output)?;

    // Output results
    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        print_results_table(&results);

        println!();
        println!(
            "{}",
            format!(
                "Results saved to: {}",
                args.output.join("summary.md").display()
            )
            .green()
        );
    }

    Ok(())
}

/// Execute the list subcommand.
fn execute_list(args: ListArgs, global_json: bool) -> Result<()> {
    let json = args.json || global_json;
    let targets = all_targets();

    if json {
        let info: Vec<serde_json::Value> = targets
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id(),
                    "description": t.description(),
                    "iterations": t.iterations()
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("{}", "Available Benchmark Targets".cyan().bold());
        println!();

        let rows: Vec<BenchmarkInfo> = targets
            .iter()
            .map(|t| BenchmarkInfo {
                id: t.id().to_string(),
                description: t.description().to_string(),
                iterations: t.iterations().to_string(),
            })
            .collect();

        let table = Table::new(rows).to_string();
        println!("{}", table);
    }

    Ok(())
}

/// Execute the results subcommand.
fn execute_results(args: ResultsArgs, global_json: bool) -> Result<()> {
    let json = args.json || global_json;
    let results = io::read_all_results(&args.output)?;

    if results.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("{}", "No benchmark results found.".yellow());
            println!(
                "Run benchmarks first with: {}",
                "llm-gateway benchmark run".cyan()
            );
        }
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        println!("{}", "Benchmark Results".cyan().bold());
        println!();
        print_results_table(&results);
    }

    Ok(())
}

/// Print results as a formatted table.
fn print_results_table(results: &[BenchmarkResult]) {
    let rows: Vec<BenchmarkRow> = results
        .iter()
        .map(|r| {
            let status = if r.is_error() {
                "❌ Failed".to_string()
            } else {
                "✅ Passed".to_string()
            };

            let latency = r
                .latency_ms()
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "-".to_string());

            let throughput = r
                .throughput_rps()
                .map(|v| format!("{:.0}", v))
                .unwrap_or_else(|| "-".to_string());

            let p99 = r
                .metrics
                .get("p99_ms")
                .and_then(|v| v.as_f64())
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "-".to_string());

            BenchmarkRow {
                target: r.target_id.clone(),
                status,
                latency,
                throughput,
                p99,
            }
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);

    // Print summary
    let total = results.len();
    let passed = results.iter().filter(|r| !r.is_error()).count();
    let failed = total - passed;

    println!();
    println!(
        "Total: {} | {} | {}",
        format!("{}", total).cyan(),
        format!("Passed: {}", passed).green(),
        if failed > 0 {
            format!("Failed: {}", failed).red().to_string()
        } else {
            format!("Failed: {}", failed).to_string()
        }
    );
}
