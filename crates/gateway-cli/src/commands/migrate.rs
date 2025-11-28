//! Migrate command - database migration management.

use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Serialize;

use crate::output::{self, CommandResult, OutputFormat};

/// Arguments for the migrate command.
#[derive(Args, Debug)]
pub struct MigrateArgs {
    #[command(subcommand)]
    pub command: MigrateCommand,

    /// Database URL
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: Option<String>,
}

/// Migrate subcommands.
#[derive(Subcommand, Debug)]
pub enum MigrateCommand {
    /// Run all pending migrations
    Run(RunArgs),

    /// Roll back migrations
    Rollback(RollbackArgs),

    /// Show migration status
    Status(StatusArgs),

    /// Validate migrations
    Validate,

    /// Create a new migration
    New(NewArgs),

    /// Show migration info
    Info,
}

/// Arguments for migrate run.
#[derive(Args, Debug)]
pub struct RunArgs {
    /// Only run specific migration version
    #[arg(long)]
    pub version: Option<i64>,

    /// Dry run - show what would be done
    #[arg(long)]
    pub dry_run: bool,
}

/// Arguments for migrate rollback.
#[derive(Args, Debug)]
pub struct RollbackArgs {
    /// Number of migrations to roll back
    #[arg(long, default_value = "1")]
    pub count: usize,

    /// Roll back to specific version
    #[arg(long)]
    pub version: Option<i64>,

    /// Dry run - show what would be done
    #[arg(long)]
    pub dry_run: bool,
}

/// Arguments for migrate status.
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Show all migrations (including applied)
    #[arg(long)]
    pub all: bool,
}

/// Arguments for new migration.
#[derive(Args, Debug, Clone)]
pub struct NewArgs {
    /// Migration name
    pub name: String,
}

/// Migration status output.
#[derive(Debug, Serialize)]
pub struct MigrationStatusOutput {
    pub total: usize,
    pub applied: usize,
    pub pending: usize,
    pub migrations: Vec<MigrationInfo>,
}

/// Individual migration info.
#[derive(Debug, Serialize)]
pub struct MigrationInfo {
    pub version: i64,
    pub name: String,
    pub status: String,
    pub applied_at: Option<String>,
    pub execution_time_ms: Option<i64>,
}

/// Execute the migrate command.
pub async fn execute(args: MigrateArgs, json: bool) -> Result<()> {
    let format = OutputFormat::from_json_flag(json);

    // Check for database URL (not needed for info and new commands)
    let database_url = args.database_url.or_else(|| std::env::var("DATABASE_URL").ok());

    // Commands that don't need database URL
    match &args.command {
        MigrateCommand::Info => return execute_info(format).await,
        MigrateCommand::New(new_args) => return execute_new(new_args.clone(), format).await,
        _ => {}
    }

    // All other commands need database URL
    let Some(database_url) = database_url else {
        let result: CommandResult<()> =
            CommandResult::failure("DATABASE_URL environment variable or --database-url required");
        result.print(format)?;
        return Ok(());
    };

    match args.command {
        MigrateCommand::Run(run_args) => execute_run(database_url, run_args, format).await,
        MigrateCommand::Rollback(rb_args) => {
            execute_rollback(database_url, rb_args, format).await
        }
        MigrateCommand::Status(status_args) => {
            execute_status(database_url, status_args, format).await
        }
        MigrateCommand::Validate => execute_validate(database_url, format).await,
        MigrateCommand::New(_) | MigrateCommand::Info => unreachable!(),
    }
}

/// Execute migrate run.
async fn execute_run(database_url: String, args: RunArgs, format: OutputFormat) -> Result<()> {
    use gateway_migrations::{schema, MigrationConfig, Migrator};

    let config = MigrationConfig::builder()
        .database_url(&database_url)
        .build()
        .map_err(|e| anyhow::anyhow!("Configuration error: {}", e))?;

    let mut migrator = Migrator::new(config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect: {}", e))?;

    migrator.add_migrations(schema::all_migrations());

    if args.dry_run {
        let pending = migrator
            .get_pending()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get pending: {}", e))?;

        match format {
            OutputFormat::Json => {
                let migrations: Vec<_> = pending
                    .iter()
                    .map(|m| MigrationInfo {
                        version: m.version,
                        name: m.name.clone(),
                        status: "pending".to_string(),
                        applied_at: None,
                        execution_time_ms: None,
                    })
                    .collect();

                let result = CommandResult::success(serde_json::json!({
                    "dry_run": true,
                    "pending_count": pending.len(),
                    "migrations": migrations,
                }));
                result.print(format)?;
            }
            OutputFormat::Text => {
                output::info(&format!("Dry run - {} migrations would be applied:", pending.len()));
                for m in &pending {
                    output::key_value(&format!("V{}", m.version), &m.name);
                }
            }
        }
        return Ok(());
    }

    let results = if let Some(version) = args.version {
        // Run specific migration
        let migration = schema::all_migrations()
            .into_iter()
            .find(|m| m.version == version)
            .ok_or_else(|| anyhow::anyhow!("Migration {} not found", version))?;

        let record = migrator
            .run_migration(&migration)
            .await
            .map_err(|e| anyhow::anyhow!("Migration failed: {}", e))?;

        vec![record]
    } else {
        // Run all pending
        migrator
            .run_pending()
            .await
            .map_err(|e| anyhow::anyhow!("Migrations failed: {}", e))?
    };

    match format {
        OutputFormat::Json => {
            let migrations: Vec<_> = results
                .iter()
                .map(|r| MigrationInfo {
                    version: r.version,
                    name: r.name.clone(),
                    status: r.status.to_string(),
                    applied_at: Some(r.applied_at.to_rfc3339()),
                    execution_time_ms: Some(r.execution_time_ms),
                })
                .collect();

            let result = CommandResult::success(serde_json::json!({
                "applied_count": results.len(),
                "migrations": migrations,
            }));
            result.print(format)?;
        }
        OutputFormat::Text => {
            if results.is_empty() {
                output::success("No pending migrations");
            } else {
                output::success(&format!("Applied {} migration(s)", results.len()));
                for r in &results {
                    output::key_value(
                        &format!("V{}", r.version),
                        &format!("{} ({}ms)", r.name, r.execution_time_ms),
                    );
                }
            }
        }
    }

    Ok(())
}

/// Execute migrate rollback.
async fn execute_rollback(
    database_url: String,
    args: RollbackArgs,
    format: OutputFormat,
) -> Result<()> {
    use gateway_migrations::{schema, MigrationConfig, Migrator};

    let config = MigrationConfig::builder()
        .database_url(&database_url)
        .build()
        .map_err(|e| anyhow::anyhow!("Configuration error: {}", e))?;

    let mut migrator = Migrator::new(config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect: {}", e))?;

    migrator.add_migrations(schema::all_migrations());

    if args.dry_run {
        match format {
            OutputFormat::Json => {
                let result = CommandResult::success(serde_json::json!({
                    "dry_run": true,
                    "would_rollback": args.count,
                }));
                result.print(format)?;
            }
            OutputFormat::Text => {
                output::warning(&format!(
                    "Dry run - would roll back {} migration(s)",
                    args.count
                ));
            }
        }
        return Ok(());
    }

    let results = if let Some(version) = args.version {
        let record = migrator
            .rollback(version)
            .await
            .map_err(|e| anyhow::anyhow!("Rollback failed: {}", e))?;
        vec![record]
    } else {
        migrator
            .rollback_last(args.count)
            .await
            .map_err(|e| anyhow::anyhow!("Rollback failed: {}", e))?
    };

    match format {
        OutputFormat::Json => {
            let migrations: Vec<_> = results
                .iter()
                .map(|r| MigrationInfo {
                    version: r.version,
                    name: r.name.clone(),
                    status: r.status.to_string(),
                    applied_at: None,
                    execution_time_ms: Some(r.execution_time_ms),
                })
                .collect();

            let result = CommandResult::success(serde_json::json!({
                "rolled_back_count": results.len(),
                "migrations": migrations,
            }));
            result.print(format)?;
        }
        OutputFormat::Text => {
            if results.is_empty() {
                output::info("No migrations to roll back");
            } else {
                output::success(&format!("Rolled back {} migration(s)", results.len()));
                for r in &results {
                    output::key_value(&format!("V{}", r.version), &r.name);
                }
            }
        }
    }

    Ok(())
}

/// Execute migrate status.
async fn execute_status(
    database_url: String,
    args: StatusArgs,
    format: OutputFormat,
) -> Result<()> {
    use gateway_migrations::{schema, MigrationConfig, MigrationStatus, Migrator};

    let config = MigrationConfig::builder()
        .database_url(&database_url)
        .build()
        .map_err(|e| anyhow::anyhow!("Configuration error: {}", e))?;

    let mut migrator = Migrator::new(config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect: {}", e))?;

    migrator.add_migrations(schema::all_migrations());

    let status = migrator
        .detailed_status()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get status: {}", e))?;

    let applied_count = status
        .iter()
        .filter(|(_, s, _)| *s == MigrationStatus::Applied)
        .count();
    let pending_count = status
        .iter()
        .filter(|(_, s, _)| *s == MigrationStatus::Pending)
        .count();

    let migrations: Vec<MigrationInfo> = status
        .iter()
        .filter(|(_, s, _)| args.all || *s == MigrationStatus::Pending)
        .map(|(m, s, r)| MigrationInfo {
            version: m.version,
            name: m.name.clone(),
            status: s.to_string(),
            applied_at: r.as_ref().map(|r| r.applied_at.to_rfc3339()),
            execution_time_ms: r.as_ref().map(|r| r.execution_time_ms),
        })
        .collect();

    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(MigrationStatusOutput {
                total: status.len(),
                applied: applied_count,
                pending: pending_count,
                migrations,
            });
            result.print(format)?;
        }
        OutputFormat::Text => {
            output::section("Migration Status");
            output::key_value("Total", &status.len().to_string());
            output::key_value("Applied", &applied_count.to_string());
            output::key_value("Pending", &pending_count.to_string());

            if !migrations.is_empty() {
                println!();
                output::section(if args.all { "All Migrations" } else { "Pending Migrations" });
                for m in &migrations {
                    let status_icon = match m.status.as_str() {
                        "applied" => "✓",
                        "pending" => "○",
                        "failed" => "✗",
                        _ => "?",
                    };
                    println!(
                        "  {} V{} - {}",
                        status_icon, m.version, m.name
                    );
                    if let Some(ref at) = m.applied_at {
                        println!("      Applied: {}", at);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Execute migrate validate.
async fn execute_validate(database_url: String, format: OutputFormat) -> Result<()> {
    use gateway_migrations::{schema, MigrationConfig, Migrator};

    let config = MigrationConfig::builder()
        .database_url(&database_url)
        .build()
        .map_err(|e| anyhow::anyhow!("Configuration error: {}", e))?;

    let mut migrator = Migrator::new(config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect: {}", e))?;

    migrator.add_migrations(schema::all_migrations());

    let issues = migrator
        .validate()
        .await
        .map_err(|e| anyhow::anyhow!("Validation failed: {}", e))?;

    match format {
        OutputFormat::Json => {
            let issues_json: Vec<String> = issues.iter().map(|i| i.to_string()).collect();
            let result = if issues.is_empty() {
                CommandResult::success(serde_json::json!({
                    "valid": true,
                    "issues": issues_json,
                }))
            } else {
                CommandResult {
                    success: false,
                    data: Some(serde_json::json!({
                        "valid": false,
                        "issues": issues_json,
                    })),
                    error: Some("Validation failed".to_string()),
                    message: None,
                }
            };
            result.print(format)?;
        }
        OutputFormat::Text => {
            if issues.is_empty() {
                output::success("All migrations are valid");
            } else {
                output::error(&format!("Found {} validation issue(s):", issues.len()));
                for issue in &issues {
                    output::error(&format!("  - {}", issue));
                }
            }
        }
    }

    Ok(())
}

/// Execute migrate new.
async fn execute_new(args: NewArgs, format: OutputFormat) -> Result<()> {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    let filename = format!("V{}_{}.sql", timestamp, args.name.replace(' ', "_").to_lowercase());

    let template = format!(
        r#"-- Migration: {}
-- Version: {}
-- Created: {}

-- UP Migration
-- Add your migration SQL here

-- DOWN Migration (in separate file or after delimiter)
-- Add rollback SQL here
"#,
        args.name,
        timestamp,
        chrono::Utc::now().to_rfc3339()
    );

    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(serde_json::json!({
                "filename": filename,
                "version": timestamp,
                "template": template,
            }));
            result.print(format)?;
        }
        OutputFormat::Text => {
            output::success(&format!("Migration template for: {}", filename));
            println!();
            println!("{}", template);
            output::info("Note: Migrations are defined in code in gateway-migrations/src/schema.rs");
        }
    }

    Ok(())
}

/// Execute migrate info.
async fn execute_info(format: OutputFormat) -> Result<()> {
    use gateway_migrations::schema;

    let migrations = schema::all_migrations();

    match format {
        OutputFormat::Json => {
            let info: Vec<_> = migrations
                .iter()
                .map(|m| serde_json::json!({
                    "version": m.version,
                    "name": m.name,
                    "has_rollback": m.supports_rollback(),
                    "tags": m.tags,
                }))
                .collect();

            let result = CommandResult::success(serde_json::json!({
                "total_migrations": migrations.len(),
                "migrations": info,
            }));
            result.print(format)?;
        }
        OutputFormat::Text => {
            output::section("Available Migrations");
            output::key_value("Total", &migrations.len().to_string());
            println!();

            for m in &migrations {
                let rollback = if m.supports_rollback() { "✓" } else { "✗" };
                println!(
                    "  V{} - {} [rollback: {}]",
                    m.version, m.name, rollback
                );
                if !m.tags.is_empty() {
                    println!("      Tags: {}", m.tags.join(", "));
                }
            }
        }
    }

    Ok(())
}
