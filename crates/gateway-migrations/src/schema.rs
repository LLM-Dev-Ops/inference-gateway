//! Database schema migrations.
//!
//! This module contains all the SQL migrations for the gateway database.

use crate::migration::Migration;

/// Get all migrations in order.
#[must_use]
pub fn all_migrations() -> Vec<Migration> {
    vec![
        v001_create_tenants(),
        v002_create_api_keys(),
        v003_create_usage_records(),
        v004_create_audit_logs(),
        v005_create_rate_limits(),
        v006_create_provider_configs(),
        v007_create_model_mappings(),
        v008_create_request_cache(),
        v009_create_cost_tracking(),
        v010_add_indexes(),
    ]
}

/// V001: Create tenants table.
#[must_use]
pub fn v001_create_tenants() -> Migration {
    Migration::builder(20240101000001, "create_tenants")
        .up(r#"
            -- Tenants table for multi-tenancy support
            CREATE TABLE IF NOT EXISTS tenants (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                name VARCHAR(255) NOT NULL,
                slug VARCHAR(100) NOT NULL UNIQUE,
                status VARCHAR(20) NOT NULL DEFAULT 'active',
                tier VARCHAR(50) NOT NULL DEFAULT 'free',
                settings JSONB NOT NULL DEFAULT '{}',
                metadata JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                deleted_at TIMESTAMPTZ
            );

            -- Create trigger for updated_at
            CREATE OR REPLACE FUNCTION update_updated_at_column()
            RETURNS TRIGGER AS $$
            BEGIN
                NEW.updated_at = NOW();
                RETURN NEW;
            END;
            $$ language 'plpgsql';

            CREATE TRIGGER update_tenants_updated_at
                BEFORE UPDATE ON tenants
                FOR EACH ROW
                EXECUTE FUNCTION update_updated_at_column();

            -- Indexes
            CREATE INDEX idx_tenants_slug ON tenants(slug);
            CREATE INDEX idx_tenants_status ON tenants(status);
            CREATE INDEX idx_tenants_tier ON tenants(tier);
            CREATE INDEX idx_tenants_created_at ON tenants(created_at);

            -- Insert default tenant
            INSERT INTO tenants (name, slug, tier)
            VALUES ('Default', 'default', 'enterprise')
            ON CONFLICT (slug) DO NOTHING;
        "#)
        .down(r#"
            DROP TRIGGER IF EXISTS update_tenants_updated_at ON tenants;
            DROP FUNCTION IF EXISTS update_updated_at_column();
            DROP TABLE IF EXISTS tenants CASCADE;
        "#)
        .tag("core")
        .tag("tenants")
        .build()
}

/// V002: Create API keys table.
#[must_use]
pub fn v002_create_api_keys() -> Migration {
    Migration::builder(20240101000002, "create_api_keys")
        .up(r#"
            -- API keys table
            CREATE TABLE IF NOT EXISTS api_keys (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                name VARCHAR(255) NOT NULL,
                key_hash VARCHAR(64) NOT NULL UNIQUE,
                key_prefix VARCHAR(20) NOT NULL,
                scopes TEXT[] NOT NULL DEFAULT '{}',
                rate_limit_rpm INTEGER,
                rate_limit_tpm INTEGER,
                status VARCHAR(20) NOT NULL DEFAULT 'active',
                expires_at TIMESTAMPTZ,
                last_used_at TIMESTAMPTZ,
                usage_count BIGINT NOT NULL DEFAULT 0,
                metadata JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                revoked_at TIMESTAMPTZ,
                revoked_reason TEXT
            );

            CREATE TRIGGER update_api_keys_updated_at
                BEFORE UPDATE ON api_keys
                FOR EACH ROW
                EXECUTE FUNCTION update_updated_at_column();

            -- Indexes
            CREATE INDEX idx_api_keys_tenant_id ON api_keys(tenant_id);
            CREATE INDEX idx_api_keys_key_hash ON api_keys(key_hash);
            CREATE INDEX idx_api_keys_key_prefix ON api_keys(key_prefix);
            CREATE INDEX idx_api_keys_status ON api_keys(status);
            CREATE INDEX idx_api_keys_expires_at ON api_keys(expires_at);
            CREATE INDEX idx_api_keys_last_used_at ON api_keys(last_used_at);
        "#)
        .down(r#"
            DROP TRIGGER IF EXISTS update_api_keys_updated_at ON api_keys;
            DROP TABLE IF EXISTS api_keys CASCADE;
        "#)
        .tag("core")
        .tag("auth")
        .build()
}

/// V003: Create usage records table.
#[must_use]
pub fn v003_create_usage_records() -> Migration {
    Migration::builder(20240101000003, "create_usage_records")
        .up(r#"
            -- Usage records table for tracking API usage
            CREATE TABLE IF NOT EXISTS usage_records (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                api_key_id UUID REFERENCES api_keys(id) ON DELETE SET NULL,
                request_id VARCHAR(64) NOT NULL,
                model VARCHAR(100) NOT NULL,
                provider VARCHAR(50) NOT NULL,
                prompt_tokens INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                latency_ms INTEGER NOT NULL DEFAULT 0,
                status VARCHAR(20) NOT NULL,
                error_code VARCHAR(50),
                error_message TEXT,
                cached BOOLEAN NOT NULL DEFAULT FALSE,
                streamed BOOLEAN NOT NULL DEFAULT FALSE,
                metadata JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            -- Partitioning for better performance (monthly)
            -- Note: In production, you'd want to set up actual partitioning
            -- This creates a simple table without partitioning for compatibility

            -- Indexes for common queries
            CREATE INDEX idx_usage_records_tenant_id ON usage_records(tenant_id);
            CREATE INDEX idx_usage_records_api_key_id ON usage_records(api_key_id);
            CREATE INDEX idx_usage_records_request_id ON usage_records(request_id);
            CREATE INDEX idx_usage_records_model ON usage_records(model);
            CREATE INDEX idx_usage_records_provider ON usage_records(provider);
            CREATE INDEX idx_usage_records_status ON usage_records(status);
            CREATE INDEX idx_usage_records_created_at ON usage_records(created_at);
            CREATE INDEX idx_usage_records_tenant_created ON usage_records(tenant_id, created_at);

            -- Aggregation helper index
            CREATE INDEX idx_usage_records_aggregation ON usage_records(tenant_id, model, created_at);
        "#)
        .down(r#"
            DROP TABLE IF EXISTS usage_records CASCADE;
        "#)
        .tag("core")
        .tag("usage")
        .build()
}

/// V004: Create audit logs table.
#[must_use]
pub fn v004_create_audit_logs() -> Migration {
    Migration::builder(20240101000004, "create_audit_logs")
        .up(r#"
            -- Audit logs table for compliance and security
            CREATE TABLE IF NOT EXISTS audit_logs (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID REFERENCES tenants(id) ON DELETE SET NULL,
                actor_type VARCHAR(50) NOT NULL,
                actor_id VARCHAR(255),
                action VARCHAR(100) NOT NULL,
                resource_type VARCHAR(100) NOT NULL,
                resource_id VARCHAR(255),
                old_value JSONB,
                new_value JSONB,
                ip_address INET,
                user_agent TEXT,
                request_id VARCHAR(64),
                metadata JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            -- Indexes for audit queries
            CREATE INDEX idx_audit_logs_tenant_id ON audit_logs(tenant_id);
            CREATE INDEX idx_audit_logs_actor ON audit_logs(actor_type, actor_id);
            CREATE INDEX idx_audit_logs_action ON audit_logs(action);
            CREATE INDEX idx_audit_logs_resource ON audit_logs(resource_type, resource_id);
            CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at);
            CREATE INDEX idx_audit_logs_request_id ON audit_logs(request_id);
            CREATE INDEX idx_audit_logs_ip_address ON audit_logs(ip_address);
        "#)
        .down(r#"
            DROP TABLE IF EXISTS audit_logs CASCADE;
        "#)
        .tag("core")
        .tag("audit")
        .build()
}

/// V005: Create rate limits table.
#[must_use]
pub fn v005_create_rate_limits() -> Migration {
    Migration::builder(20240101000005, "create_rate_limits")
        .up(r#"
            -- Rate limit configurations
            CREATE TABLE IF NOT EXISTS rate_limit_configs (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
                api_key_id UUID REFERENCES api_keys(id) ON DELETE CASCADE,
                scope VARCHAR(50) NOT NULL DEFAULT 'global',
                requests_per_minute INTEGER NOT NULL DEFAULT 60,
                tokens_per_minute INTEGER NOT NULL DEFAULT 100000,
                requests_per_day INTEGER,
                tokens_per_day INTEGER,
                concurrent_requests INTEGER DEFAULT 10,
                burst_multiplier DECIMAL(3,2) DEFAULT 1.5,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                metadata JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE(tenant_id, api_key_id, scope)
            );

            CREATE TRIGGER update_rate_limit_configs_updated_at
                BEFORE UPDATE ON rate_limit_configs
                FOR EACH ROW
                EXECUTE FUNCTION update_updated_at_column();

            -- Rate limit state (for distributed rate limiting)
            CREATE TABLE IF NOT EXISTS rate_limit_state (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                key VARCHAR(255) NOT NULL UNIQUE,
                window_start TIMESTAMPTZ NOT NULL,
                request_count INTEGER NOT NULL DEFAULT 0,
                token_count INTEGER NOT NULL DEFAULT 0,
                last_request_at TIMESTAMPTZ,
                expires_at TIMESTAMPTZ NOT NULL,
                metadata JSONB NOT NULL DEFAULT '{}'
            );

            -- Indexes
            CREATE INDEX idx_rate_limit_configs_tenant_id ON rate_limit_configs(tenant_id);
            CREATE INDEX idx_rate_limit_configs_api_key_id ON rate_limit_configs(api_key_id);
            CREATE INDEX idx_rate_limit_state_key ON rate_limit_state(key);
            CREATE INDEX idx_rate_limit_state_expires ON rate_limit_state(expires_at);
        "#)
        .down(r#"
            DROP TRIGGER IF EXISTS update_rate_limit_configs_updated_at ON rate_limit_configs;
            DROP TABLE IF EXISTS rate_limit_state CASCADE;
            DROP TABLE IF EXISTS rate_limit_configs CASCADE;
        "#)
        .tag("core")
        .tag("rate_limiting")
        .build()
}

/// V006: Create provider configurations table.
#[must_use]
pub fn v006_create_provider_configs() -> Migration {
    Migration::builder(20240101000006, "create_provider_configs")
        .up(r#"
            -- Provider configurations (stored in DB for dynamic updates)
            CREATE TABLE IF NOT EXISTS provider_configs (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
                provider_id VARCHAR(100) NOT NULL,
                provider_type VARCHAR(50) NOT NULL,
                endpoint VARCHAR(500) NOT NULL,
                api_key_encrypted TEXT,
                api_key_env_var VARCHAR(100),
                models TEXT[] NOT NULL DEFAULT '{}',
                default_model VARCHAR(100),
                priority INTEGER NOT NULL DEFAULT 100,
                weight INTEGER NOT NULL DEFAULT 100,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                health_check_enabled BOOLEAN NOT NULL DEFAULT TRUE,
                timeout_ms INTEGER NOT NULL DEFAULT 30000,
                max_retries INTEGER NOT NULL DEFAULT 3,
                headers JSONB NOT NULL DEFAULT '{}',
                settings JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE(tenant_id, provider_id)
            );

            CREATE TRIGGER update_provider_configs_updated_at
                BEFORE UPDATE ON provider_configs
                FOR EACH ROW
                EXECUTE FUNCTION update_updated_at_column();

            -- Provider health status
            CREATE TABLE IF NOT EXISTS provider_health (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                provider_config_id UUID NOT NULL REFERENCES provider_configs(id) ON DELETE CASCADE,
                status VARCHAR(20) NOT NULL DEFAULT 'unknown',
                last_check_at TIMESTAMPTZ,
                last_success_at TIMESTAMPTZ,
                last_failure_at TIMESTAMPTZ,
                consecutive_failures INTEGER NOT NULL DEFAULT 0,
                error_message TEXT,
                latency_p50_ms INTEGER,
                latency_p95_ms INTEGER,
                latency_p99_ms INTEGER,
                success_rate DECIMAL(5,2),
                metadata JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE(provider_config_id)
            );

            CREATE TRIGGER update_provider_health_updated_at
                BEFORE UPDATE ON provider_health
                FOR EACH ROW
                EXECUTE FUNCTION update_updated_at_column();

            -- Indexes
            CREATE INDEX idx_provider_configs_tenant_id ON provider_configs(tenant_id);
            CREATE INDEX idx_provider_configs_provider_type ON provider_configs(provider_type);
            CREATE INDEX idx_provider_configs_enabled ON provider_configs(enabled);
            CREATE INDEX idx_provider_health_status ON provider_health(status);
        "#)
        .down(r#"
            DROP TRIGGER IF EXISTS update_provider_health_updated_at ON provider_health;
            DROP TRIGGER IF EXISTS update_provider_configs_updated_at ON provider_configs;
            DROP TABLE IF EXISTS provider_health CASCADE;
            DROP TABLE IF EXISTS provider_configs CASCADE;
        "#)
        .tag("core")
        .tag("providers")
        .build()
}

/// V007: Create model mappings table.
#[must_use]
pub fn v007_create_model_mappings() -> Migration {
    Migration::builder(20240101000007, "create_model_mappings")
        .up(r#"
            -- Model mappings for aliasing and routing
            CREATE TABLE IF NOT EXISTS model_mappings (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
                source_model VARCHAR(200) NOT NULL,
                target_provider VARCHAR(100) NOT NULL,
                target_model VARCHAR(200) NOT NULL,
                priority INTEGER NOT NULL DEFAULT 100,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                conditions JSONB NOT NULL DEFAULT '{}',
                metadata JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE(tenant_id, source_model, target_provider)
            );

            CREATE TRIGGER update_model_mappings_updated_at
                BEFORE UPDATE ON model_mappings
                FOR EACH ROW
                EXECUTE FUNCTION update_updated_at_column();

            -- Model capabilities
            CREATE TABLE IF NOT EXISTS model_capabilities (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                model VARCHAR(200) NOT NULL UNIQUE,
                provider VARCHAR(100) NOT NULL,
                capabilities JSONB NOT NULL DEFAULT '{}',
                context_window INTEGER,
                max_output_tokens INTEGER,
                supports_vision BOOLEAN NOT NULL DEFAULT FALSE,
                supports_functions BOOLEAN NOT NULL DEFAULT FALSE,
                supports_streaming BOOLEAN NOT NULL DEFAULT TRUE,
                pricing JSONB NOT NULL DEFAULT '{}',
                metadata JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE TRIGGER update_model_capabilities_updated_at
                BEFORE UPDATE ON model_capabilities
                FOR EACH ROW
                EXECUTE FUNCTION update_updated_at_column();

            -- Indexes
            CREATE INDEX idx_model_mappings_tenant_id ON model_mappings(tenant_id);
            CREATE INDEX idx_model_mappings_source_model ON model_mappings(source_model);
            CREATE INDEX idx_model_mappings_target ON model_mappings(target_provider, target_model);
            CREATE INDEX idx_model_capabilities_provider ON model_capabilities(provider);
        "#)
        .down(r#"
            DROP TRIGGER IF EXISTS update_model_capabilities_updated_at ON model_capabilities;
            DROP TRIGGER IF EXISTS update_model_mappings_updated_at ON model_mappings;
            DROP TABLE IF EXISTS model_capabilities CASCADE;
            DROP TABLE IF EXISTS model_mappings CASCADE;
        "#)
        .tag("routing")
        .tag("models")
        .build()
}

/// V008: Create request cache table.
#[must_use]
pub fn v008_create_request_cache() -> Migration {
    Migration::builder(20240101000008, "create_request_cache")
        .up(r#"
            -- Request cache for semantic caching
            CREATE TABLE IF NOT EXISTS request_cache (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
                cache_key VARCHAR(128) NOT NULL,
                model VARCHAR(100) NOT NULL,
                request_hash VARCHAR(64) NOT NULL,
                response JSONB NOT NULL,
                prompt_tokens INTEGER NOT NULL,
                completion_tokens INTEGER NOT NULL,
                hit_count INTEGER NOT NULL DEFAULT 0,
                last_hit_at TIMESTAMPTZ,
                expires_at TIMESTAMPTZ NOT NULL,
                metadata JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE(tenant_id, cache_key)
            );

            -- Indexes
            CREATE INDEX idx_request_cache_tenant_id ON request_cache(tenant_id);
            CREATE INDEX idx_request_cache_cache_key ON request_cache(cache_key);
            CREATE INDEX idx_request_cache_model ON request_cache(model);
            CREATE INDEX idx_request_cache_expires_at ON request_cache(expires_at);
            CREATE INDEX idx_request_cache_request_hash ON request_cache(request_hash);
        "#)
        .down(r#"
            DROP TABLE IF EXISTS request_cache CASCADE;
        "#)
        .tag("cache")
        .build()
}

/// V009: Create cost tracking tables.
#[must_use]
pub fn v009_create_cost_tracking() -> Migration {
    Migration::builder(20240101000009, "create_cost_tracking")
        .up(r#"
            -- Cost tracking per tenant
            CREATE TABLE IF NOT EXISTS cost_records (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                api_key_id UUID REFERENCES api_keys(id) ON DELETE SET NULL,
                usage_record_id UUID REFERENCES usage_records(id) ON DELETE SET NULL,
                model VARCHAR(100) NOT NULL,
                provider VARCHAR(50) NOT NULL,
                prompt_tokens INTEGER NOT NULL,
                completion_tokens INTEGER NOT NULL,
                prompt_cost DECIMAL(20,10) NOT NULL,
                completion_cost DECIMAL(20,10) NOT NULL,
                total_cost DECIMAL(20,10) NOT NULL,
                currency VARCHAR(3) NOT NULL DEFAULT 'USD',
                pricing_version VARCHAR(50),
                metadata JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            -- Cost budgets
            CREATE TABLE IF NOT EXISTS cost_budgets (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                api_key_id UUID REFERENCES api_keys(id) ON DELETE CASCADE,
                budget_type VARCHAR(50) NOT NULL DEFAULT 'monthly',
                budget_amount DECIMAL(20,2) NOT NULL,
                alert_threshold DECIMAL(3,2) DEFAULT 0.80,
                hard_limit BOOLEAN NOT NULL DEFAULT FALSE,
                current_spend DECIMAL(20,2) NOT NULL DEFAULT 0,
                period_start TIMESTAMPTZ NOT NULL,
                period_end TIMESTAMPTZ NOT NULL,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                metadata JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE TRIGGER update_cost_budgets_updated_at
                BEFORE UPDATE ON cost_budgets
                FOR EACH ROW
                EXECUTE FUNCTION update_updated_at_column();

            -- Indexes
            CREATE INDEX idx_cost_records_tenant_id ON cost_records(tenant_id);
            CREATE INDEX idx_cost_records_api_key_id ON cost_records(api_key_id);
            CREATE INDEX idx_cost_records_model ON cost_records(model);
            CREATE INDEX idx_cost_records_created_at ON cost_records(created_at);
            CREATE INDEX idx_cost_records_tenant_date ON cost_records(tenant_id, created_at);
            CREATE INDEX idx_cost_budgets_tenant_id ON cost_budgets(tenant_id);
            CREATE INDEX idx_cost_budgets_period ON cost_budgets(period_start, period_end);
        "#)
        .down(r#"
            DROP TRIGGER IF EXISTS update_cost_budgets_updated_at ON cost_budgets;
            DROP TABLE IF EXISTS cost_budgets CASCADE;
            DROP TABLE IF EXISTS cost_records CASCADE;
        "#)
        .tag("billing")
        .tag("cost")
        .build()
}

/// V010: Add performance indexes.
#[must_use]
pub fn v010_add_indexes() -> Migration {
    Migration::builder(20240101000010, "add_indexes")
        .up(r#"
            -- Additional indexes for common query patterns

            -- Composite indexes for usage analytics
            CREATE INDEX IF NOT EXISTS idx_usage_records_analytics
                ON usage_records(tenant_id, model, provider, created_at);

            CREATE INDEX IF NOT EXISTS idx_usage_records_billing
                ON usage_records(tenant_id, api_key_id, created_at)
                INCLUDE (prompt_tokens, completion_tokens, total_tokens);

            -- Partial indexes for active records
            CREATE INDEX IF NOT EXISTS idx_api_keys_active
                ON api_keys(tenant_id, key_hash)
                WHERE status = 'active' AND (expires_at IS NULL OR expires_at > NOW());

            CREATE INDEX IF NOT EXISTS idx_provider_configs_active
                ON provider_configs(tenant_id, provider_type)
                WHERE enabled = TRUE;

            -- Full-text search on audit logs (if needed)
            CREATE INDEX IF NOT EXISTS idx_audit_logs_action_resource
                ON audit_logs(action, resource_type, created_at);

            -- Materialized view for usage statistics (optional)
            CREATE MATERIALIZED VIEW IF NOT EXISTS usage_stats_daily AS
            SELECT
                tenant_id,
                api_key_id,
                model,
                provider,
                DATE(created_at) as date,
                COUNT(*) as request_count,
                SUM(prompt_tokens) as total_prompt_tokens,
                SUM(completion_tokens) as total_completion_tokens,
                SUM(total_tokens) as total_tokens,
                AVG(latency_ms) as avg_latency_ms,
                PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY latency_ms) as p95_latency_ms,
                SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) as success_count,
                SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END) as error_count,
                SUM(CASE WHEN cached THEN 1 ELSE 0 END) as cached_count
            FROM usage_records
            GROUP BY tenant_id, api_key_id, model, provider, DATE(created_at);

            CREATE UNIQUE INDEX IF NOT EXISTS idx_usage_stats_daily_unique
                ON usage_stats_daily(tenant_id, api_key_id, model, provider, date);
        "#)
        .down(r#"
            DROP MATERIALIZED VIEW IF EXISTS usage_stats_daily;
            DROP INDEX IF EXISTS idx_audit_logs_action_resource;
            DROP INDEX IF EXISTS idx_provider_configs_active;
            DROP INDEX IF EXISTS idx_api_keys_active;
            DROP INDEX IF EXISTS idx_usage_records_billing;
            DROP INDEX IF EXISTS idx_usage_records_analytics;
        "#)
        .tag("performance")
        .tag("indexes")
        .build()
}

/// Get migrations by tag.
#[must_use]
pub fn migrations_by_tag(tag: &str) -> Vec<Migration> {
    all_migrations()
        .into_iter()
        .filter(|m| m.tags.iter().any(|t| t == tag))
        .collect()
}

/// Get core migrations only.
#[must_use]
pub fn core_migrations() -> Vec<Migration> {
    migrations_by_tag("core")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_migrations_ordered() {
        let migrations = all_migrations();
        let mut prev_version = 0;
        for m in &migrations {
            assert!(
                m.version > prev_version,
                "Migrations must be in order: {} should be > {}",
                m.version,
                prev_version
            );
            prev_version = m.version;
        }
    }

    #[test]
    fn test_all_migrations_have_rollback() {
        let migrations = all_migrations();
        for m in &migrations {
            assert!(
                m.supports_rollback(),
                "Migration {} should have rollback SQL",
                m.version
            );
        }
    }

    #[test]
    fn test_migrations_have_valid_checksums() {
        let migrations = all_migrations();
        for m in &migrations {
            assert!(
                m.verify_checksum(),
                "Migration {} has invalid checksum",
                m.version
            );
        }
    }

    #[test]
    fn test_migrations_by_tag() {
        let core = migrations_by_tag("core");
        assert!(!core.is_empty());

        for m in &core {
            assert!(m.tags.contains(&"core".to_string()));
        }
    }

    #[test]
    fn test_migration_versions_unique() {
        let migrations = all_migrations();
        let mut versions = std::collections::HashSet::new();
        for m in &migrations {
            assert!(
                versions.insert(m.version),
                "Duplicate version: {}",
                m.version
            );
        }
    }

    #[test]
    fn test_migration_count() {
        let migrations = all_migrations();
        assert_eq!(migrations.len(), 10, "Expected 10 migrations");
    }
}
