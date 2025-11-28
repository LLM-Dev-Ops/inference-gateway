//! IP filtering and blocking.

use crate::config::IpFilterSettings;
use crate::error::{Result, SecurityError};
use ipnetwork::IpNetwork;
use std::collections::HashSet;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// IP filter for allow/block list management.
#[derive(Debug)]
pub struct IpFilter {
    config: IpFilterSettings,
    allowlist: Arc<RwLock<HashSet<IpNetwork>>>,
    blocklist: Arc<RwLock<HashSet<IpNetwork>>>,
}

impl IpFilter {
    /// Create a new IP filter.
    #[must_use]
    pub fn new(config: IpFilterSettings) -> Self {
        Self {
            config,
            allowlist: Arc::new(RwLock::new(HashSet::new())),
            blocklist: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Create with default config.
    #[must_use]
    pub fn default_filter() -> Self {
        Self::new(IpFilterSettings::default())
    }

    /// Initialize from configuration.
    ///
    /// # Errors
    /// Returns error if IP/CIDR parsing fails.
    pub async fn initialize(&self) -> Result<()> {
        // Load allowlist
        let mut allowlist = self.allowlist.write().await;
        for ip_str in &self.config.allowlist {
            let network = parse_ip_or_network(ip_str)?;
            allowlist.insert(network);
        }

        // Load blocklist
        let mut blocklist = self.blocklist.write().await;
        for ip_str in &self.config.blocklist {
            let network = parse_ip_or_network(ip_str)?;
            blocklist.insert(network);
        }

        Ok(())
    }

    /// Check if an IP is allowed.
    ///
    /// # Errors
    /// Returns error if IP is blocked or not in allowlist.
    pub async fn check(&self, ip: IpAddr) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Check if IP should be blocked based on type
        if self.config.block_loopback && ip.is_loopback() {
            if !self.config.allow_localhost {
                return Err(SecurityError::IpBlocked(format!(
                    "Loopback addresses are blocked: {}",
                    ip
                )));
            }
        }

        if self.config.block_private && is_private_ip(ip) {
            return Err(SecurityError::IpBlocked(format!(
                "Private addresses are blocked: {}",
                ip
            )));
        }

        // Check blocklist first (takes priority)
        let blocklist = self.blocklist.read().await;
        for network in blocklist.iter() {
            if network.contains(ip) {
                return Err(SecurityError::IpBlocked(ip.to_string()));
            }
        }

        // Check allowlist if not empty
        let allowlist = self.allowlist.read().await;
        if !allowlist.is_empty() {
            let allowed = allowlist.iter().any(|network| network.contains(ip));
            if !allowed {
                return Err(SecurityError::IpNotAllowed(ip.to_string()));
            }
        }

        Ok(())
    }

    /// Check if an IP string is allowed.
    ///
    /// # Errors
    /// Returns error if IP is invalid or blocked.
    pub async fn check_str(&self, ip_str: &str) -> Result<()> {
        let ip = ip_str
            .parse()
            .map_err(|_| SecurityError::validation(format!("Invalid IP address: {}", ip_str)))?;
        self.check(ip).await
    }

    /// Add IP or network to allowlist.
    ///
    /// # Errors
    /// Returns error if IP/CIDR parsing fails.
    pub async fn allow(&self, ip_str: &str) -> Result<()> {
        let network = parse_ip_or_network(ip_str)?;
        let mut allowlist = self.allowlist.write().await;
        allowlist.insert(network);
        Ok(())
    }

    /// Add IP or network to blocklist.
    ///
    /// # Errors
    /// Returns error if IP/CIDR parsing fails.
    pub async fn block(&self, ip_str: &str) -> Result<()> {
        let network = parse_ip_or_network(ip_str)?;
        let mut blocklist = self.blocklist.write().await;
        blocklist.insert(network);
        Ok(())
    }

    /// Remove IP or network from allowlist.
    ///
    /// # Errors
    /// Returns error if IP/CIDR parsing fails.
    pub async fn unallow(&self, ip_str: &str) -> Result<bool> {
        let network = parse_ip_or_network(ip_str)?;
        let mut allowlist = self.allowlist.write().await;
        Ok(allowlist.remove(&network))
    }

    /// Remove IP or network from blocklist.
    ///
    /// # Errors
    /// Returns error if IP/CIDR parsing fails.
    pub async fn unblock(&self, ip_str: &str) -> Result<bool> {
        let network = parse_ip_or_network(ip_str)?;
        let mut blocklist = self.blocklist.write().await;
        Ok(blocklist.remove(&network))
    }

    /// Get current allowlist.
    pub async fn get_allowlist(&self) -> Vec<String> {
        let allowlist = self.allowlist.read().await;
        allowlist.iter().map(|n| n.to_string()).collect()
    }

    /// Get current blocklist.
    pub async fn get_blocklist(&self) -> Vec<String> {
        let blocklist = self.blocklist.read().await;
        blocklist.iter().map(|n| n.to_string()).collect()
    }

    /// Clear all lists.
    pub async fn clear(&self) {
        self.allowlist.write().await.clear();
        self.blocklist.write().await.clear();
    }

    /// Check if filtering is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

impl Clone for IpFilter {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            allowlist: Arc::clone(&self.allowlist),
            blocklist: Arc::clone(&self.blocklist),
        }
    }
}

/// IP filter configuration builder.
#[derive(Debug, Default, Clone)]
pub struct IpFilterConfig {
    enabled: bool,
    allowlist: Vec<String>,
    blocklist: Vec<String>,
    block_private: bool,
    block_loopback: bool,
    allow_localhost: bool,
}

impl IpFilterConfig {
    /// Create a new IP filter config builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: true,
            allow_localhost: true,
            ..Default::default()
        }
    }

    /// Enable or disable the filter.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Add to allowlist.
    #[must_use]
    pub fn allow(mut self, ip: impl Into<String>) -> Self {
        self.allowlist.push(ip.into());
        self
    }

    /// Add multiple to allowlist.
    #[must_use]
    pub fn allow_all(mut self, ips: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allowlist.extend(ips.into_iter().map(Into::into));
        self
    }

    /// Add to blocklist.
    #[must_use]
    pub fn block(mut self, ip: impl Into<String>) -> Self {
        self.blocklist.push(ip.into());
        self
    }

    /// Add multiple to blocklist.
    #[must_use]
    pub fn block_all(mut self, ips: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.blocklist.extend(ips.into_iter().map(Into::into));
        self
    }

    /// Block private IP ranges.
    #[must_use]
    pub fn block_private(mut self) -> Self {
        self.block_private = true;
        self
    }

    /// Block loopback addresses.
    #[must_use]
    pub fn block_loopback(mut self) -> Self {
        self.block_loopback = true;
        self
    }

    /// Allow localhost even if loopback is blocked.
    #[must_use]
    pub fn allow_localhost(mut self, allow: bool) -> Self {
        self.allow_localhost = allow;
        self
    }

    /// Build the IP filter.
    ///
    /// # Errors
    /// Returns error if initialization fails.
    pub async fn build(self) -> Result<IpFilter> {
        let settings = IpFilterSettings {
            enabled: self.enabled,
            allowlist: self.allowlist,
            blocklist: self.blocklist,
            block_private: self.block_private,
            block_loopback: self.block_loopback,
            allow_localhost: self.allow_localhost,
        };

        let filter = IpFilter::new(settings);
        filter.initialize().await?;
        Ok(filter)
    }
}

/// Parse an IP address or CIDR network.
fn parse_ip_or_network(s: &str) -> Result<IpNetwork> {
    // Try parsing as network first
    if let Ok(network) = IpNetwork::from_str(s) {
        return Ok(network);
    }

    // Try parsing as IP address
    if let Ok(ip) = IpAddr::from_str(s) {
        return match ip {
            IpAddr::V4(v4) => Ok(IpNetwork::V4(
                ipnetwork::Ipv4Network::new(v4, 32)
                    .map_err(|e| SecurityError::config(e.to_string()))?,
            )),
            IpAddr::V6(v6) => Ok(IpNetwork::V6(
                ipnetwork::Ipv6Network::new(v6, 128)
                    .map_err(|e| SecurityError::config(e.to_string()))?,
            )),
        };
    }

    Err(SecurityError::config(format!(
        "Invalid IP address or network: {}",
        s
    )))
}

/// Check if an IP is in a private range.
#[must_use]
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            // 10.0.0.0/8
            v4.octets()[0] == 10
            // 172.16.0.0/12
            || (v4.octets()[0] == 172 && (v4.octets()[1] >= 16 && v4.octets()[1] <= 31))
            // 192.168.0.0/16
            || (v4.octets()[0] == 192 && v4.octets()[1] == 168)
            // 169.254.0.0/16 (link-local)
            || (v4.octets()[0] == 169 && v4.octets()[1] == 254)
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            // fc00::/7 (unique local)
            (segments[0] & 0xfe00) == 0xfc00
            // fe80::/10 (link-local)
            || (segments[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Check if an IP is a loopback address.
#[must_use]
pub fn is_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
}

/// Check if an IP is a multicast address.
#[must_use]
pub fn is_multicast(ip: IpAddr) -> bool {
    ip.is_multicast()
}

/// Get the real IP from X-Forwarded-For header.
#[must_use]
pub fn get_real_ip(xff_header: Option<&str>, remote_addr: IpAddr) -> IpAddr {
    xff_header
        .and_then(|xff| {
            xff.split(',')
                .next()
                .and_then(|ip| ip.trim().parse().ok())
        })
        .unwrap_or(remote_addr)
}

/// Extract IP from forwarded headers with trust chain.
pub struct TrustedProxies {
    proxies: HashSet<IpNetwork>,
}

impl TrustedProxies {
    /// Create a new trusted proxies config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            proxies: HashSet::new(),
        }
    }

    /// Add a trusted proxy.
    ///
    /// # Errors
    /// Returns error if IP/CIDR parsing fails.
    pub fn add(&mut self, proxy: &str) -> Result<()> {
        let network = parse_ip_or_network(proxy)?;
        self.proxies.insert(network);
        Ok(())
    }

    /// Check if an IP is a trusted proxy.
    #[must_use]
    pub fn is_trusted(&self, ip: IpAddr) -> bool {
        self.proxies.iter().any(|network| network.contains(ip))
    }

    /// Get the real client IP from headers.
    #[must_use]
    pub fn get_client_ip(&self, xff_header: Option<&str>, remote_addr: IpAddr) -> IpAddr {
        // If no XFF header or remote addr is not trusted, return remote addr
        let Some(xff) = xff_header else {
            return remote_addr;
        };

        if !self.is_trusted(remote_addr) {
            return remote_addr;
        }

        // Walk backwards through the XFF chain
        let ips: Vec<&str> = xff.split(',').map(str::trim).collect();

        for ip_str in ips.iter().rev() {
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                if !self.is_trusted(ip) {
                    return ip;
                }
            }
        }

        // All IPs are trusted proxies, return the first one
        ips.first()
            .and_then(|ip| ip.parse().ok())
            .unwrap_or(remote_addr)
    }
}

impl Default for TrustedProxies {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ip_filter_disabled() {
        let filter = IpFilter::new(IpFilterSettings {
            enabled: false,
            ..Default::default()
        });

        // Should allow any IP when disabled
        let result = filter.check("1.2.3.4".parse().unwrap()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ip_filter_blocklist() {
        let filter = IpFilterConfig::new()
            .block("192.168.1.100")
            .block("10.0.0.0/8")
            .build()
            .await
            .unwrap();

        // Blocked IP
        let result = filter.check("192.168.1.100".parse().unwrap()).await;
        assert!(matches!(result, Err(SecurityError::IpBlocked(_))));

        // IP in blocked range
        let result = filter.check("10.1.2.3".parse().unwrap()).await;
        assert!(matches!(result, Err(SecurityError::IpBlocked(_))));

        // Not blocked
        let result = filter.check("8.8.8.8".parse().unwrap()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ip_filter_allowlist() {
        let filter = IpFilterConfig::new()
            .allow("192.168.1.0/24")
            .build()
            .await
            .unwrap();

        // In allowlist
        let result = filter.check("192.168.1.50".parse().unwrap()).await;
        assert!(result.is_ok());

        // Not in allowlist
        let result = filter.check("192.168.2.50".parse().unwrap()).await;
        assert!(matches!(result, Err(SecurityError::IpNotAllowed(_))));
    }

    #[tokio::test]
    async fn test_ip_filter_blocklist_priority() {
        let filter = IpFilterConfig::new()
            .allow("192.168.0.0/16")
            .block("192.168.1.100")
            .build()
            .await
            .unwrap();

        // In both lists - blocklist takes priority
        let result = filter.check("192.168.1.100".parse().unwrap()).await;
        assert!(matches!(result, Err(SecurityError::IpBlocked(_))));

        // Only in allowlist
        let result = filter.check("192.168.1.50".parse().unwrap()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ip_filter_private() {
        let filter = IpFilter::new(IpFilterSettings {
            enabled: true,
            block_private: true,
            ..Default::default()
        });

        let result = filter.check("192.168.1.1".parse().unwrap()).await;
        assert!(matches!(result, Err(SecurityError::IpBlocked(_))));

        let result = filter.check("10.0.0.1".parse().unwrap()).await;
        assert!(matches!(result, Err(SecurityError::IpBlocked(_))));

        let result = filter.check("8.8.8.8".parse().unwrap()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ip_filter_dynamic() {
        let filter = IpFilterConfig::new().build().await.unwrap();

        // Initially allowed
        let ip = "1.2.3.4";
        assert!(filter.check_str(ip).await.is_ok());

        // Block it
        filter.block(ip).await.unwrap();
        assert!(filter.check_str(ip).await.is_err());

        // Unblock it
        filter.unblock(ip).await.unwrap();
        assert!(filter.check_str(ip).await.is_ok());
    }

    #[test]
    fn test_is_private_ip() {
        assert!(is_private_ip("192.168.1.1".parse().unwrap()));
        assert!(is_private_ip("10.0.0.1".parse().unwrap()));
        assert!(is_private_ip("172.16.0.1".parse().unwrap()));
        assert!(is_private_ip("172.31.255.255".parse().unwrap()));
        assert!(is_private_ip("169.254.1.1".parse().unwrap()));

        assert!(!is_private_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip("172.15.0.1".parse().unwrap()));
        assert!(!is_private_ip("172.32.0.1".parse().unwrap()));
    }

    #[test]
    fn test_get_real_ip() {
        let remote = "10.0.0.1".parse().unwrap();

        // No XFF header
        assert_eq!(get_real_ip(None, remote), remote);

        // With XFF header
        let client: IpAddr = "1.2.3.4".parse().unwrap();
        assert_eq!(get_real_ip(Some("1.2.3.4"), remote), client);

        // Multiple IPs in XFF
        assert_eq!(get_real_ip(Some("1.2.3.4, 10.0.0.2"), remote), client);
    }

    #[test]
    fn test_trusted_proxies() {
        let mut proxies = TrustedProxies::new();
        proxies.add("10.0.0.0/8").unwrap();

        assert!(proxies.is_trusted("10.0.0.1".parse().unwrap()));
        assert!(!proxies.is_trusted("192.168.1.1".parse().unwrap()));

        // Get client IP through trusted proxy
        let remote: IpAddr = "10.0.0.1".parse().unwrap();
        let client = proxies.get_client_ip(Some("1.2.3.4, 10.0.0.2"), remote);
        assert_eq!(client, "1.2.3.4".parse::<IpAddr>().unwrap());

        // Untrusted remote - don't trust XFF
        let untrusted: IpAddr = "192.168.1.1".parse().unwrap();
        let client = proxies.get_client_ip(Some("1.2.3.4"), untrusted);
        assert_eq!(client, untrusted);
    }

    #[tokio::test]
    async fn test_get_lists() {
        let filter = IpFilterConfig::new()
            .allow("192.168.1.0/24")
            .block("10.0.0.0/8")
            .build()
            .await
            .unwrap();

        let allowlist = filter.get_allowlist().await;
        assert_eq!(allowlist.len(), 1);

        let blocklist = filter.get_blocklist().await;
        assert_eq!(blocklist.len(), 1);
    }

    #[tokio::test]
    async fn test_clear_lists() {
        let filter = IpFilterConfig::new()
            .allow("192.168.1.0/24")
            .block("10.0.0.0/8")
            .build()
            .await
            .unwrap();

        filter.clear().await;

        assert!(filter.get_allowlist().await.is_empty());
        assert!(filter.get_blocklist().await.is_empty());
    }

    #[test]
    fn test_parse_ip_or_network() {
        // Single IP
        let network = parse_ip_or_network("192.168.1.1").unwrap();
        assert!(network.contains("192.168.1.1".parse().unwrap()));
        assert!(!network.contains("192.168.1.2".parse().unwrap()));

        // CIDR network
        let network = parse_ip_or_network("192.168.1.0/24").unwrap();
        assert!(network.contains("192.168.1.1".parse().unwrap()));
        assert!(network.contains("192.168.1.255".parse().unwrap()));
        assert!(!network.contains("192.168.2.1".parse().unwrap()));

        // Invalid
        assert!(parse_ip_or_network("invalid").is_err());
    }

    #[tokio::test]
    async fn test_ipv6() {
        let filter = IpFilterConfig::new()
            .allow("2001:db8::/32")
            .build()
            .await
            .unwrap();

        let result = filter.check("2001:db8::1".parse().unwrap()).await;
        assert!(result.is_ok());

        let result = filter.check("2001:db9::1".parse().unwrap()).await;
        assert!(matches!(result, Err(SecurityError::IpNotAllowed(_))));
    }
}
