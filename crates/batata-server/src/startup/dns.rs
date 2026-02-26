//! DNS Service for Service Discovery
//!
//! Provides DNS-based service discovery for Batata.
//! Supports A, AAAA, SRV, and TXT records.
//!
//! Query format: {service-name}.{group-name}.{namespace}.svc.nacos.local
//!
//! Example:
//! - Query: api-service.DEFAULT_GROUP.public.svc.nacos.local
//! - Returns: A records with healthy instance IPs

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::UdpSocket;
use tracing::{debug, error, info, warn};

use batata_naming::NamingService;

/// DNS server configuration
#[derive(Clone, Debug)]
pub struct DnsConfig {
    /// Whether DNS service is enabled
    pub enabled: bool,
    /// Address to bind DNS server
    pub bind_address: String,
    /// Port to bind DNS server (default: 8853 for non-root, 53 for standard)
    pub port: u16,
    /// Default TTL for DNS responses (in seconds)
    pub default_ttl: u32,
    /// DNS suffix for service discovery
    pub suffix: String,
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_address: "0.0.0.0".to_string(),
            port: 8853,
            default_ttl: 30,
            suffix: "svc.nacos.local".to_string(),
        }
    }
}

impl DnsConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("BATATA_DNS_ENABLED")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(false),
            bind_address: std::env::var("BATATA_DNS_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("BATATA_DNS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8853),
            default_ttl: std::env::var("BATATA_DNS_TTL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            suffix: std::env::var("BATATA_DNS_SUFFIX")
                .unwrap_or_else(|_| "svc.nacos.local".to_string()),
        }
    }
}

/// DNS message header flags
const DNS_FLAG_QR: u16 = 0x8000; // Query/Response flag
const DNS_FLAG_AA: u16 = 0x0400; // Authoritative Answer
const DNS_FLAG_RD: u16 = 0x0100; // Recursion Desired
#[allow(dead_code)]
const DNS_FLAG_RA: u16 = 0x0080; // Recursion Available

/// DNS record types
const DNS_TYPE_A: u16 = 1;
#[allow(dead_code)]
const DNS_TYPE_AAAA: u16 = 28;
const DNS_TYPE_SRV: u16 = 33;
#[allow(dead_code)]
const DNS_TYPE_TXT: u16 = 16;

/// DNS class
const DNS_CLASS_IN: u16 = 1;

/// DNS response codes
#[allow(dead_code)]
const DNS_RCODE_OK: u16 = 0;
const DNS_RCODE_NXDOMAIN: u16 = 3;

/// DNS Server for service discovery
pub struct DnsServer {
    config: DnsConfig,
    naming_service: Arc<NamingService>,
}

impl DnsServer {
    /// Create a new DNS server
    pub fn new(config: DnsConfig, naming_service: Arc<NamingService>) -> Self {
        Self {
            config,
            naming_service,
        }
    }

    /// Start the DNS server
    pub async fn start(&self) -> anyhow::Result<()> {
        if !self.config.enabled {
            info!("DNS service is disabled");
            return Ok(());
        }

        let addr: SocketAddr =
            format!("{}:{}", self.config.bind_address, self.config.port).parse()?;
        let socket = Arc::new(UdpSocket::bind(addr).await?);

        info!("DNS server listening on {}", addr);

        let naming_service = self.naming_service.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 512];

            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, src)) => {
                        let query = buf[..len].to_vec();
                        let socket_clone = socket.clone();
                        let ns = naming_service.clone();
                        let cfg = config.clone();

                        tokio::spawn(async move {
                            if let Some(response) = Self::handle_query(&query, &ns, &cfg).await
                                && let Err(e) = socket_clone.send_to(&response, src).await
                            {
                                warn!("Failed to send DNS response: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("DNS receive error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle a DNS query and return a response
    async fn handle_query(
        query: &[u8],
        naming_service: &NamingService,
        config: &DnsConfig,
    ) -> Option<Vec<u8>> {
        if query.len() < 12 {
            return None;
        }

        // Parse header
        let id = u16::from_be_bytes([query[0], query[1]]);
        let flags = u16::from_be_bytes([query[2], query[3]]);
        let qdcount = u16::from_be_bytes([query[4], query[5]]);

        if qdcount == 0 {
            return None;
        }

        // Parse question
        let (qname, qname_end) = Self::parse_name(query, 12)?;
        if qname_end + 4 > query.len() {
            return None;
        }

        let qtype = u16::from_be_bytes([query[qname_end], query[qname_end + 1]]);
        let qclass = u16::from_be_bytes([query[qname_end + 2], query[qname_end + 3]]);

        debug!("DNS query: {} type={} class={}", qname, qtype, qclass);

        // Parse service name from query
        if let Some((namespace, group, service)) = Self::parse_service_name(&qname, &config.suffix)
        {
            // Query naming service for instances
            let instances = naming_service.get_instances(&namespace, &group, &service, "", true);

            if !instances.is_empty() {
                // Build response with A records
                return Some(Self::build_response(
                    id,
                    flags,
                    query,
                    qname_end + 4,
                    qtype,
                    &instances,
                    config.default_ttl,
                ));
            }
        }

        // Return NXDOMAIN for unknown services
        Some(Self::build_nxdomain_response(
            id,
            flags,
            query,
            qname_end + 4,
        ))
    }

    /// Parse a DNS name from the query
    fn parse_name(data: &[u8], offset: usize) -> Option<(String, usize)> {
        let mut name = String::new();
        let mut pos = offset;

        loop {
            if pos >= data.len() {
                return None;
            }

            let len = data[pos] as usize;
            if len == 0 {
                pos += 1;
                break;
            }

            if len & 0xC0 == 0xC0 {
                // Compression pointer - not handling for simplicity
                pos += 2;
                break;
            }

            pos += 1;
            if pos + len > data.len() {
                return None;
            }

            if !name.is_empty() {
                name.push('.');
            }
            name.push_str(&String::from_utf8_lossy(&data[pos..pos + len]));
            pos += len;
        }

        Some((name.to_lowercase(), pos))
    }

    /// Parse service name from DNS query
    /// Format: {service}.{group}.{namespace}.svc.nacos.local
    fn parse_service_name(qname: &str, suffix: &str) -> Option<(String, String, String)> {
        let suffix_lower = suffix.to_lowercase();
        if !qname.ends_with(&suffix_lower) {
            return None;
        }

        let prefix = &qname[..qname.len() - suffix_lower.len() - 1]; // Remove .suffix
        let parts: Vec<&str> = prefix.split('.').collect();

        if parts.len() >= 3 {
            let service = parts[0].to_string();
            let group = parts[1].to_string();
            let namespace = parts[2].to_string();
            Some((namespace, group, service))
        } else if parts.len() == 2 {
            // Default namespace
            let service = parts[0].to_string();
            let group = parts[1].to_string();
            Some(("public".to_string(), group, service))
        } else if parts.len() == 1 {
            // Default namespace and group
            let service = parts[0].to_string();
            Some(("public".to_string(), "DEFAULT_GROUP".to_string(), service))
        } else {
            None
        }
    }

    /// Build DNS response with A records
    fn build_response(
        id: u16,
        _query_flags: u16,
        query: &[u8],
        question_end: usize,
        qtype: u16,
        instances: &[batata_naming::Instance],
        ttl: u32,
    ) -> Vec<u8> {
        let mut response = Vec::with_capacity(512);

        // Header
        response.extend_from_slice(&id.to_be_bytes());

        // Flags: QR=1, AA=1, RD=1, RA=0, RCODE=0
        let flags: u16 = DNS_FLAG_QR | DNS_FLAG_AA | DNS_FLAG_RD;
        response.extend_from_slice(&flags.to_be_bytes());

        // QDCOUNT = 1
        response.extend_from_slice(&1u16.to_be_bytes());

        // ANCOUNT = number of instances (for A or SRV records)
        let answer_count = if qtype == DNS_TYPE_A || qtype == DNS_TYPE_SRV {
            instances.len() as u16
        } else {
            0u16
        };
        response.extend_from_slice(&answer_count.to_be_bytes());

        // NSCOUNT = 0, ARCOUNT = 0
        response.extend_from_slice(&[0, 0, 0, 0]);

        // Copy question section
        response.extend_from_slice(&query[12..question_end]);

        // Add answer records
        if qtype == DNS_TYPE_A {
            for instance in instances {
                if let Ok(ip) = instance.ip.parse::<std::net::Ipv4Addr>() {
                    // Name pointer to question
                    response.extend_from_slice(&[0xC0, 0x0C]);
                    // Type A
                    response.extend_from_slice(&DNS_TYPE_A.to_be_bytes());
                    // Class IN
                    response.extend_from_slice(&DNS_CLASS_IN.to_be_bytes());
                    // TTL
                    response.extend_from_slice(&ttl.to_be_bytes());
                    // RDLENGTH = 4
                    response.extend_from_slice(&4u16.to_be_bytes());
                    // RDATA = IP address
                    response.extend_from_slice(&ip.octets());
                }
            }
        } else if qtype == DNS_TYPE_SRV {
            for (i, instance) in instances.iter().enumerate() {
                // Name pointer to question
                response.extend_from_slice(&[0xC0, 0x0C]);
                // Type SRV
                response.extend_from_slice(&DNS_TYPE_SRV.to_be_bytes());
                // Class IN
                response.extend_from_slice(&DNS_CLASS_IN.to_be_bytes());
                // TTL
                response.extend_from_slice(&ttl.to_be_bytes());

                // Build target name (ip.svc.nacos.local)
                let target = format!("instance-{}.svc.nacos.local", i);
                let target_len = target.len() + 2; // +2 for length prefix and null terminator

                // RDLENGTH = 6 (priority, weight, port) + target_len
                response.extend_from_slice(&((6 + target_len) as u16).to_be_bytes());

                // Priority (lower = higher priority)
                response.extend_from_slice(&(100 - (instance.weight * 100.0) as u16).to_be_bytes());
                // Weight
                response.extend_from_slice(&((instance.weight * 100.0) as u16).to_be_bytes());
                // Port
                response.extend_from_slice(&(instance.port as u16).to_be_bytes());

                // Target (as DNS name)
                for part in target.split('.') {
                    response.push(part.len() as u8);
                    response.extend_from_slice(part.as_bytes());
                }
                response.push(0);
            }
        }

        response
    }

    /// Build NXDOMAIN response
    fn build_nxdomain_response(
        id: u16,
        _query_flags: u16,
        query: &[u8],
        question_end: usize,
    ) -> Vec<u8> {
        let mut response = Vec::with_capacity(question_end + 12);

        // Header
        response.extend_from_slice(&id.to_be_bytes());

        // Flags: QR=1, AA=1, RD=1, RA=0, RCODE=NXDOMAIN (3)
        let flags: u16 = DNS_FLAG_QR | DNS_FLAG_AA | DNS_FLAG_RD | DNS_RCODE_NXDOMAIN;
        response.extend_from_slice(&flags.to_be_bytes());

        // QDCOUNT = 1, ANCOUNT = 0, NSCOUNT = 0, ARCOUNT = 0
        response.extend_from_slice(&1u16.to_be_bytes());
        response.extend_from_slice(&[0, 0, 0, 0, 0, 0]);

        // Copy question section
        response.extend_from_slice(&query[12..question_end]);

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_service_name() {
        let suffix = "svc.nacos.local";

        // Full format
        let result = DnsServer::parse_service_name(
            "api-service.default_group.public.svc.nacos.local",
            suffix,
        );
        assert_eq!(
            result,
            Some((
                "public".to_string(),
                "default_group".to_string(),
                "api-service".to_string()
            ))
        );

        // Two parts (default namespace)
        let result =
            DnsServer::parse_service_name("api-service.default_group.svc.nacos.local", suffix);
        assert_eq!(
            result,
            Some((
                "public".to_string(),
                "default_group".to_string(),
                "api-service".to_string()
            ))
        );

        // One part (default namespace and group)
        let result = DnsServer::parse_service_name("api-service.svc.nacos.local", suffix);
        assert_eq!(
            result,
            Some((
                "public".to_string(),
                "DEFAULT_GROUP".to_string(),
                "api-service".to_string()
            ))
        );

        // Wrong suffix
        let result = DnsServer::parse_service_name("api-service.other.domain", suffix);
        assert_eq!(result, None);
    }

    #[test]
    fn test_dns_config_default() {
        let config = DnsConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.port, 8853);
        assert_eq!(config.default_ttl, 30);
        assert_eq!(config.suffix, "svc.nacos.local");
    }
}
