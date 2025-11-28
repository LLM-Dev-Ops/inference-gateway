//! Cryptographic utilities.

use crate::error::{Result, SecurityError};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use hmac::{Hmac, Mac};
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256, Sha512};
use zeroize::Zeroizing;

/// HMAC-SHA256 type.
type HmacSha256 = Hmac<Sha256>;

/// Encryption service for sensitive data.
#[derive(Clone)]
pub struct Encryption {
    key: Zeroizing<[u8; 32]>,
}

impl Encryption {
    /// Create a new encryption service with the given key.
    ///
    /// # Errors
    /// Returns error if key is invalid.
    pub fn new(key: &[u8]) -> Result<Self> {
        if key.len() != 32 {
            return Err(SecurityError::Encryption(
                "Key must be 32 bytes".to_string(),
            ));
        }

        let mut key_array = Zeroizing::new([0u8; 32]);
        key_array.copy_from_slice(key);

        Ok(Self { key: key_array })
    }

    /// Create from a hex-encoded key.
    ///
    /// # Errors
    /// Returns error if key is invalid.
    pub fn from_hex(hex_key: &str) -> Result<Self> {
        let key = hex::decode(hex_key)
            .map_err(|e| SecurityError::Encryption(format!("Invalid hex key: {}", e)))?;
        Self::new(&key)
    }

    /// Create from a base64-encoded key.
    ///
    /// # Errors
    /// Returns error if key is invalid.
    pub fn from_base64(b64_key: &str) -> Result<Self> {
        let key = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64_key)
            .map_err(|e| SecurityError::Encryption(format!("Invalid base64 key: {}", e)))?;
        Self::new(&key)
    }

    /// Generate a new random key.
    #[must_use]
    pub fn generate_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }

    /// Encrypt data with AES-256-GCM.
    ///
    /// # Errors
    /// Returns error if encryption fails.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(&*self.key)
            .map_err(|e| SecurityError::Encryption(format!("Failed to create cipher: {}", e)))?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| SecurityError::Encryption(format!("Encryption failed: {}", e)))?;

        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend(ciphertext);

        Ok(result)
    }

    /// Decrypt data with AES-256-GCM.
    ///
    /// # Errors
    /// Returns error if decryption fails.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < 12 {
            return Err(SecurityError::Decryption(
                "Ciphertext too short".to_string(),
            ));
        }

        let cipher = Aes256Gcm::new_from_slice(&*self.key)
            .map_err(|e| SecurityError::Decryption(format!("Failed to create cipher: {}", e)))?;

        let nonce = Nonce::from_slice(&ciphertext[..12]);
        let ciphertext = &ciphertext[12..];

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| SecurityError::Decryption(format!("Decryption failed: {}", e)))
    }

    /// Encrypt a string and return base64.
    ///
    /// # Errors
    /// Returns error if encryption fails.
    pub fn encrypt_string(&self, plaintext: &str) -> Result<String> {
        let encrypted = self.encrypt(plaintext.as_bytes())?;
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &encrypted,
        ))
    }

    /// Decrypt a base64 string.
    ///
    /// # Errors
    /// Returns error if decryption fails.
    pub fn decrypt_string(&self, ciphertext: &str) -> Result<String> {
        let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, ciphertext)
            .map_err(|e| SecurityError::Decryption(format!("Invalid base64: {}", e)))?;
        let decrypted = self.decrypt(&data)?;
        String::from_utf8(decrypted)
            .map_err(|e| SecurityError::Decryption(format!("Invalid UTF-8: {}", e)))
    }
}

impl std::fmt::Debug for Encryption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Encryption")
            .field("key", &"[REDACTED]")
            .finish()
    }
}

/// Hashing service for passwords and data.
#[derive(Debug, Clone, Default)]
pub struct HashingService {
    /// Memory cost for Argon2.
    memory_cost: u32,
    /// Time cost for Argon2.
    time_cost: u32,
    /// Parallelism for Argon2.
    parallelism: u32,
}

impl HashingService {
    /// Create a new hashing service with default parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            memory_cost: 65536,   // 64 MB
            time_cost: 3,         // 3 iterations
            parallelism: 4,       // 4 threads
        }
    }

    /// Create with custom parameters.
    #[must_use]
    pub fn with_params(memory_cost: u32, time_cost: u32, parallelism: u32) -> Self {
        Self {
            memory_cost,
            time_cost,
            parallelism,
        }
    }

    /// Hash a password using Argon2id.
    ///
    /// # Errors
    /// Returns error if hashing fails.
    pub fn hash_password(&self, password: &SecretString) -> Result<String> {
        let salt = SaltString::generate(&mut rand::thread_rng());
        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon2::Params::new(self.memory_cost, self.time_cost, self.parallelism, None)
                .map_err(|e| SecurityError::Internal(format!("Invalid Argon2 params: {}", e)))?,
        );

        argon2
            .hash_password(password.expose_secret().as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| SecurityError::Internal(format!("Password hashing failed: {}", e)))
    }

    /// Verify a password against a hash.
    ///
    /// # Errors
    /// Returns error if verification fails.
    pub fn verify_password(&self, password: &SecretString, hash: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| SecurityError::Internal(format!("Invalid hash format: {}", e)))?;

        let argon2 = Argon2::default();
        Ok(argon2
            .verify_password(password.expose_secret().as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// Hash data using SHA-256.
    #[must_use]
    pub fn sha256(data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    /// Hash data using SHA-256 and return hex.
    #[must_use]
    pub fn sha256_hex(data: &[u8]) -> String {
        hex::encode(Self::sha256(data))
    }

    /// Hash data using SHA-512.
    #[must_use]
    pub fn sha512(data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha512::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    /// Hash data using SHA-512 and return hex.
    #[must_use]
    pub fn sha512_hex(data: &[u8]) -> String {
        hex::encode(Self::sha512(data))
    }

    /// Generate HMAC-SHA256.
    ///
    /// # Errors
    /// Returns error if HMAC generation fails.
    pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
        let mut mac = <HmacSha256 as KeyInit>::new_from_slice(key)
            .map_err(|e| SecurityError::Internal(format!("Invalid HMAC key: {}", e)))?;
        mac.update(data);
        Ok(mac.finalize().into_bytes().to_vec())
    }

    /// Generate HMAC-SHA256 and return hex.
    ///
    /// # Errors
    /// Returns error if HMAC generation fails.
    pub fn hmac_sha256_hex(key: &[u8], data: &[u8]) -> Result<String> {
        Self::hmac_sha256(key, data).map(|h| hex::encode(h))
    }

    /// Verify HMAC-SHA256.
    ///
    /// # Errors
    /// Returns error if verification fails.
    pub fn verify_hmac_sha256(key: &[u8], data: &[u8], signature: &[u8]) -> Result<bool> {
        let mut mac = <HmacSha256 as KeyInit>::new_from_slice(key)
            .map_err(|e| SecurityError::Internal(format!("Invalid HMAC key: {}", e)))?;
        mac.update(data);
        Ok(mac.verify_slice(signature).is_ok())
    }

    /// Constant-time comparison of two byte slices.
    #[must_use]
    pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let mut result = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            result |= x ^ y;
        }
        result == 0
    }
}

/// Key derivation service.
#[derive(Debug, Clone, Default)]
pub struct KeyDerivation {
    /// Salt length.
    salt_length: usize,
    /// Output key length.
    key_length: usize,
}

impl KeyDerivation {
    /// Create a new key derivation service.
    #[must_use]
    pub fn new() -> Self {
        Self {
            salt_length: 16,
            key_length: 32,
        }
    }

    /// Create with custom parameters.
    #[must_use]
    pub fn with_params(salt_length: usize, key_length: usize) -> Self {
        Self {
            salt_length,
            key_length,
        }
    }

    /// Derive a key from a password using Argon2id.
    ///
    /// # Errors
    /// Returns error if key derivation fails.
    pub fn derive_key(&self, password: &SecretString, salt: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let argon2 = Argon2::default();

        let mut output = Zeroizing::new(vec![0u8; self.key_length]);

        argon2
            .hash_password_into(
                password.expose_secret().as_bytes(),
                salt,
                &mut output,
            )
            .map_err(|e| SecurityError::KeyDerivation(format!("Key derivation failed: {}", e)))?;

        Ok(output)
    }

    /// Derive a key with a generated salt.
    ///
    /// Returns (derived_key, salt).
    ///
    /// # Errors
    /// Returns error if key derivation fails.
    pub fn derive_key_with_salt(
        &self,
        password: &SecretString,
    ) -> Result<(Zeroizing<Vec<u8>>, Vec<u8>)> {
        let mut salt = vec![0u8; self.salt_length];
        rand::thread_rng().fill_bytes(&mut salt);

        let key = self.derive_key(password, &salt)?;
        Ok((key, salt))
    }

    /// Generate a random salt.
    #[must_use]
    pub fn generate_salt(&self) -> Vec<u8> {
        let mut salt = vec![0u8; self.salt_length];
        rand::thread_rng().fill_bytes(&mut salt);
        salt
    }
}

/// Generate a secure random token.
#[must_use]
pub fn generate_token(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Generate a secure random API key.
#[must_use]
pub fn generate_api_key() -> String {
    format!("llm_{}", generate_token(32))
}

/// Generate a UUID v4.
#[must_use]
pub fn generate_uuid() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);

    // Set version (4) and variant (RFC 4122)
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u16::from_be_bytes([bytes[4], bytes[5]]),
        u16::from_be_bytes([bytes[6], bytes[7]]),
        u16::from_be_bytes([bytes[8], bytes[9]]),
        u64::from_be_bytes([0, 0, bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]])
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_roundtrip() {
        let key = Encryption::generate_key();
        let enc = Encryption::new(&key).unwrap();

        let plaintext = b"Hello, World!";
        let ciphertext = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_encryption_string() {
        let key = Encryption::generate_key();
        let enc = Encryption::new(&key).unwrap();

        let plaintext = "Secret message";
        let encrypted = enc.encrypt_string(plaintext).unwrap();
        let decrypted = enc.decrypt_string(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encryption_wrong_key() {
        let key1 = Encryption::generate_key();
        let key2 = Encryption::generate_key();

        let enc1 = Encryption::new(&key1).unwrap();
        let enc2 = Encryption::new(&key2).unwrap();

        let ciphertext = enc1.encrypt(b"Secret").unwrap();
        let result = enc2.decrypt(&ciphertext);

        assert!(result.is_err());
    }

    #[test]
    fn test_hashing_sha256() {
        let hash = HashingService::sha256_hex(b"hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_hashing_sha512() {
        let hash = HashingService::sha512(b"hello");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hmac_sha256() {
        let key = b"secret_key";
        let data = b"message";

        let signature = HashingService::hmac_sha256(key, data).unwrap();
        let verified = HashingService::verify_hmac_sha256(key, data, &signature).unwrap();

        assert!(verified);
    }

    #[test]
    fn test_hmac_sha256_invalid() {
        let key = b"secret_key";
        let data = b"message";

        let signature = HashingService::hmac_sha256(key, data).unwrap();
        let verified = HashingService::verify_hmac_sha256(key, b"other", &signature).unwrap();

        assert!(!verified);
    }

    #[test]
    fn test_password_hashing() {
        let service = HashingService::new();
        let password = SecretString::new("my_password".to_string());

        let hash = service.hash_password(&password).unwrap();
        assert!(hash.starts_with("$argon2id$"));

        let verified = service.verify_password(&password, &hash).unwrap();
        assert!(verified);

        let wrong_password = SecretString::new("wrong_password".to_string());
        let not_verified = service.verify_password(&wrong_password, &hash).unwrap();
        assert!(!not_verified);
    }

    #[test]
    fn test_key_derivation() {
        let kd = KeyDerivation::new();
        let password = SecretString::new("password".to_string());
        let salt = kd.generate_salt();

        let key1 = kd.derive_key(&password, &salt).unwrap();
        let key2 = kd.derive_key(&password, &salt).unwrap();

        assert_eq!(key1.as_slice(), key2.as_slice());
        assert_eq!(key1.len(), 32);
    }

    #[test]
    fn test_key_derivation_different_salts() {
        let kd = KeyDerivation::new();
        let password = SecretString::new("password".to_string());

        let (key1, salt1) = kd.derive_key_with_salt(&password).unwrap();
        let (key2, salt2) = kd.derive_key_with_salt(&password).unwrap();

        assert_ne!(salt1, salt2);
        assert_ne!(key1.as_slice(), key2.as_slice());
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(HashingService::constant_time_eq(b"hello", b"hello"));
        assert!(!HashingService::constant_time_eq(b"hello", b"world"));
        assert!(!HashingService::constant_time_eq(b"hello", b"hell"));
    }

    #[test]
    fn test_generate_token() {
        let token = generate_token(32);
        assert_eq!(token.len(), 64); // hex encoding doubles the length
    }

    #[test]
    fn test_generate_api_key() {
        let key = generate_api_key();
        assert!(key.starts_with("llm_"));
        assert_eq!(key.len(), 68); // "llm_" + 64 hex chars
    }

    #[test]
    fn test_generate_uuid() {
        let uuid = generate_uuid();
        assert_eq!(uuid.len(), 36);
        assert!(uuid.chars().nth(8) == Some('-'));
        assert!(uuid.chars().nth(13) == Some('-'));
        assert!(uuid.chars().nth(18) == Some('-'));
        assert!(uuid.chars().nth(23) == Some('-'));
    }
}
