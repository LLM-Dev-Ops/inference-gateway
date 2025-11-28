//! Secrets management.

use crate::config::SecretsConfig;
use crate::crypto::Encryption;
use crate::error::{Result, SecurityError};
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use zeroize::Zeroizing;

/// A secret value with metadata.
#[derive(Clone)]
pub struct SecretValue {
    /// The secret value.
    value: SecretString,
    /// When the secret was created.
    created_at: DateTime<Utc>,
    /// When the secret expires (if any).
    expires_at: Option<DateTime<Utc>>,
    /// Metadata about the secret.
    metadata: HashMap<String, String>,
    /// Version number.
    version: u32,
}

impl SecretValue {
    /// Create a new secret value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: SecretString::new(value.into()),
            created_at: Utc::now(),
            expires_at: None,
            metadata: HashMap::new(),
            version: 1,
        }
    }

    /// Create with expiration.
    #[must_use]
    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Add metadata.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set version.
    #[must_use]
    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Get the secret value.
    #[must_use]
    pub fn expose(&self) -> &str {
        self.value.expose_secret()
    }

    /// Get the inner SecretString.
    #[must_use]
    pub fn inner(&self) -> &SecretString {
        &self.value
    }

    /// Check if the secret is expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| Utc::now() > exp)
            .unwrap_or(false)
    }

    /// Get creation time.
    #[must_use]
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Get expiration time.
    #[must_use]
    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }

    /// Get metadata.
    #[must_use]
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    /// Get version.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretValue")
            .field("value", &"[REDACTED]")
            .field("created_at", &self.created_at)
            .field("expires_at", &self.expires_at)
            .field("metadata", &self.metadata)
            .field("version", &self.version)
            .finish()
    }
}

/// Secret store for managing secrets.
#[derive(Debug)]
pub struct SecretStore {
    config: SecretsConfig,
    secrets: Arc<RwLock<HashMap<String, SecretValue>>>,
    encryption: Option<Encryption>,
}

impl SecretStore {
    /// Create a new secret store.
    #[must_use]
    pub fn new(config: SecretsConfig) -> Self {
        Self {
            config,
            secrets: Arc::new(RwLock::new(HashMap::new())),
            encryption: None,
        }
    }

    /// Create with encryption.
    ///
    /// # Errors
    /// Returns error if encryption key is invalid.
    pub fn with_encryption(mut self, key: &[u8]) -> Result<Self> {
        self.encryption = Some(Encryption::new(key)?);
        Ok(self)
    }

    /// Load secrets from environment variables.
    ///
    /// # Errors
    /// Returns error if loading fails.
    pub async fn load_from_env(&self) -> Result<()> {
        let prefix = &self.config.env_prefix;
        let mut secrets = self.secrets.write().await;

        for (key, value) in std::env::vars() {
            if key.starts_with(prefix) {
                let secret_name = key.strip_prefix(prefix).unwrap_or(&key).to_lowercase();
                let secret_value = SecretValue::new(value)
                    .with_metadata("source", "env")
                    .with_metadata("env_var", &key);
                secrets.insert(secret_name, secret_value);
            }
        }

        Ok(())
    }

    /// Get a secret by name.
    ///
    /// # Errors
    /// Returns error if secret is not found or expired.
    pub async fn get(&self, name: &str) -> Result<SecretValue> {
        let secrets = self.secrets.read().await;

        let secret = secrets
            .get(name)
            .ok_or_else(|| SecurityError::SecretNotFound(name.to_string()))?;

        if secret.is_expired() {
            return Err(SecurityError::SecretExpired(name.to_string()));
        }

        Ok(secret.clone())
    }

    /// Get a secret value as string.
    ///
    /// # Errors
    /// Returns error if secret is not found or expired.
    pub async fn get_string(&self, name: &str) -> Result<Zeroizing<String>> {
        let secret = self.get(name).await?;
        Ok(Zeroizing::new(secret.expose().to_string()))
    }

    /// Set a secret.
    pub async fn set(&self, name: impl Into<String>, value: SecretValue) {
        let mut secrets = self.secrets.write().await;
        secrets.insert(name.into(), value);
    }

    /// Set a simple string secret.
    pub async fn set_string(&self, name: impl Into<String>, value: impl Into<String>) {
        let secret = SecretValue::new(value);
        self.set(name, secret).await;
    }

    /// Delete a secret.
    pub async fn delete(&self, name: &str) -> bool {
        let mut secrets = self.secrets.write().await;
        secrets.remove(name).is_some()
    }

    /// Check if a secret exists and is not expired.
    pub async fn exists(&self, name: &str) -> bool {
        let secrets = self.secrets.read().await;
        secrets
            .get(name)
            .map(|s| !s.is_expired())
            .unwrap_or(false)
    }

    /// List all secret names.
    pub async fn list(&self) -> Vec<String> {
        let secrets = self.secrets.read().await;
        secrets.keys().cloned().collect()
    }

    /// List all non-expired secret names.
    pub async fn list_valid(&self) -> Vec<String> {
        let secrets = self.secrets.read().await;
        secrets
            .iter()
            .filter(|(_, v)| !v.is_expired())
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Encrypt a value for storage.
    ///
    /// # Errors
    /// Returns error if encryption fails.
    pub fn encrypt(&self, value: &str) -> Result<String> {
        let enc = self
            .encryption
            .as_ref()
            .ok_or_else(|| SecurityError::Config("Encryption not configured".to_string()))?;
        enc.encrypt_string(value)
    }

    /// Decrypt a stored value.
    ///
    /// # Errors
    /// Returns error if decryption fails.
    pub fn decrypt(&self, encrypted: &str) -> Result<Zeroizing<String>> {
        let enc = self
            .encryption
            .as_ref()
            .ok_or_else(|| SecurityError::Config("Encryption not configured".to_string()))?;
        Ok(Zeroizing::new(enc.decrypt_string(encrypted)?))
    }

    /// Rotate a secret to a new value.
    ///
    /// # Errors
    /// Returns error if secret is not found.
    pub async fn rotate(&self, name: &str, new_value: impl Into<String>) -> Result<u32> {
        let mut secrets = self.secrets.write().await;

        let old_secret = secrets
            .get(name)
            .ok_or_else(|| SecurityError::SecretNotFound(name.to_string()))?;

        let new_version = old_secret.version + 1;
        let new_secret = SecretValue::new(new_value)
            .with_version(new_version)
            .with_metadata("rotated_from", &old_secret.version.to_string());

        secrets.insert(name.to_string(), new_secret);

        Ok(new_version)
    }

    /// Clean up expired secrets.
    pub async fn cleanup_expired(&self) -> usize {
        let mut secrets = self.secrets.write().await;
        let before = secrets.len();
        secrets.retain(|_, v| !v.is_expired());
        before - secrets.len()
    }
}

impl Default for SecretStore {
    fn default() -> Self {
        Self::new(SecretsConfig::default())
    }
}

/// Secret reference for lazy loading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRef {
    /// Secret name.
    pub name: String,
    /// Optional key for nested secrets.
    pub key: Option<String>,
    /// Default value if secret not found.
    pub default: Option<String>,
}

impl SecretRef {
    /// Create a new secret reference.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            key: None,
            default: None,
        }
    }

    /// Set the key for nested access.
    #[must_use]
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Set a default value.
    #[must_use]
    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default = Some(default.into());
        self
    }

    /// Resolve the secret from a store.
    ///
    /// # Errors
    /// Returns error if secret is not found and no default.
    pub async fn resolve(&self, store: &SecretStore) -> Result<Zeroizing<String>> {
        match store.get_string(&self.name).await {
            Ok(value) => Ok(value),
            Err(_) if self.default.is_some() => {
                Ok(Zeroizing::new(self.default.clone().unwrap()))
            }
            Err(e) => Err(e),
        }
    }
}

/// Builder for creating secret stores.
#[derive(Debug, Default)]
pub struct SecretStoreBuilder {
    config: SecretsConfig,
    encryption_key: Option<Vec<u8>>,
    initial_secrets: HashMap<String, String>,
}

impl SecretStoreBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set configuration.
    #[must_use]
    pub fn config(mut self, config: SecretsConfig) -> Self {
        self.config = config;
        self
    }

    /// Set encryption key.
    #[must_use]
    pub fn encryption_key(mut self, key: Vec<u8>) -> Self {
        self.encryption_key = Some(key);
        self
    }

    /// Add an initial secret.
    #[must_use]
    pub fn secret(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.initial_secrets.insert(name.into(), value.into());
        self
    }

    /// Build the secret store.
    ///
    /// # Errors
    /// Returns error if encryption key is invalid.
    pub async fn build(self) -> Result<SecretStore> {
        let mut store = SecretStore::new(self.config);

        if let Some(key) = self.encryption_key {
            store = store.with_encryption(&key)?;
        }

        for (name, value) in self.initial_secrets {
            store.set_string(name, value).await;
        }

        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_value() {
        let secret = SecretValue::new("my_secret")
            .with_metadata("type", "api_key")
            .with_version(2);

        assert_eq!(secret.expose(), "my_secret");
        assert!(!secret.is_expired());
        assert_eq!(secret.version(), 2);
        assert_eq!(secret.metadata().get("type").unwrap(), "api_key");
    }

    #[test]
    fn test_secret_expiration() {
        let expired = SecretValue::new("secret")
            .with_expiry(Utc::now() - chrono::Duration::hours(1));

        assert!(expired.is_expired());

        let valid = SecretValue::new("secret")
            .with_expiry(Utc::now() + chrono::Duration::hours(1));

        assert!(!valid.is_expired());
    }

    #[test]
    fn test_secret_value_debug() {
        let secret = SecretValue::new("sensitive_data");
        let debug = format!("{:?}", secret);

        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("sensitive_data"));
    }

    #[tokio::test]
    async fn test_secret_store_basic() {
        let store = SecretStore::default();

        store.set_string("api_key", "secret_value").await;

        assert!(store.exists("api_key").await);
        assert!(!store.exists("nonexistent").await);

        let value = store.get_string("api_key").await.unwrap();
        assert_eq!(&*value, "secret_value");
    }

    #[tokio::test]
    async fn test_secret_store_not_found() {
        let store = SecretStore::default();

        let result = store.get("nonexistent").await;
        assert!(matches!(result, Err(SecurityError::SecretNotFound(_))));
    }

    #[tokio::test]
    async fn test_secret_store_expired() {
        let store = SecretStore::default();

        let expired_secret = SecretValue::new("value")
            .with_expiry(Utc::now() - chrono::Duration::seconds(1));

        store.set("expired", expired_secret).await;

        let result = store.get("expired").await;
        assert!(matches!(result, Err(SecurityError::SecretExpired(_))));
    }

    #[tokio::test]
    async fn test_secret_store_delete() {
        let store = SecretStore::default();

        store.set_string("to_delete", "value").await;
        assert!(store.exists("to_delete").await);

        let deleted = store.delete("to_delete").await;
        assert!(deleted);
        assert!(!store.exists("to_delete").await);

        let not_deleted = store.delete("nonexistent").await;
        assert!(!not_deleted);
    }

    #[tokio::test]
    async fn test_secret_store_list() {
        let store = SecretStore::default();

        store.set_string("secret1", "value1").await;
        store.set_string("secret2", "value2").await;

        let list = store.list().await;
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"secret1".to_string()));
        assert!(list.contains(&"secret2".to_string()));
    }

    #[tokio::test]
    async fn test_secret_store_rotation() {
        let store = SecretStore::default();

        store.set_string("rotating", "initial").await;

        let new_version = store.rotate("rotating", "rotated").await.unwrap();
        assert_eq!(new_version, 2);

        let secret = store.get("rotating").await.unwrap();
        assert_eq!(secret.expose(), "rotated");
        assert_eq!(secret.version(), 2);
    }

    #[tokio::test]
    async fn test_secret_store_cleanup() {
        let store = SecretStore::default();

        // Add valid secret
        store.set_string("valid", "value").await;

        // Add expired secret
        let expired = SecretValue::new("expired_value")
            .with_expiry(Utc::now() - chrono::Duration::seconds(1));
        store.set("expired", expired).await;

        let cleaned = store.cleanup_expired().await;
        assert_eq!(cleaned, 1);

        assert!(store.exists("valid").await);
        assert!(!store.exists("expired").await);
    }

    #[tokio::test]
    async fn test_secret_store_with_encryption() {
        let key = crate::crypto::Encryption::generate_key();
        let store = SecretStore::new(SecretsConfig::default())
            .with_encryption(&key)
            .unwrap();

        let encrypted = store.encrypt("sensitive data").unwrap();
        let decrypted = store.decrypt(&encrypted).unwrap();

        assert_eq!(&*decrypted, "sensitive data");
    }

    #[tokio::test]
    async fn test_secret_ref() {
        let store = SecretStore::default();
        store.set_string("my_secret", "secret_value").await;

        let secret_ref = SecretRef::new("my_secret");
        let value = secret_ref.resolve(&store).await.unwrap();
        assert_eq!(&*value, "secret_value");
    }

    #[tokio::test]
    async fn test_secret_ref_with_default() {
        let store = SecretStore::default();

        let secret_ref = SecretRef::new("nonexistent")
            .with_default("default_value");

        let value = secret_ref.resolve(&store).await.unwrap();
        assert_eq!(&*value, "default_value");
    }

    #[tokio::test]
    async fn test_secret_store_builder() {
        let store = SecretStoreBuilder::new()
            .secret("key1", "value1")
            .secret("key2", "value2")
            .build()
            .await
            .unwrap();

        assert_eq!(&*store.get_string("key1").await.unwrap(), "value1");
        assert_eq!(&*store.get_string("key2").await.unwrap(), "value2");
    }

    #[tokio::test]
    async fn test_list_valid_secrets() {
        let store = SecretStore::default();

        store.set_string("valid1", "value1").await;
        store.set_string("valid2", "value2").await;

        let expired = SecretValue::new("expired")
            .with_expiry(Utc::now() - chrono::Duration::seconds(1));
        store.set("expired", expired).await;

        let valid = store.list_valid().await;
        assert_eq!(valid.len(), 2);
        assert!(!valid.contains(&"expired".to_string()));
    }
}
