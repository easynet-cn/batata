//! Consul HTTP API response metadata and query option helpers.
//!
//! Matches the original Consul `setMeta`, `parseConsistency`, and `parseWait`
//! behavior from `agent/http.go`.
//!
//! Every Consul query response MUST include the standard `X-Consul-*` headers.
//! This module provides [`ConsulResponseMeta`] and [`ConsulQueryOptions`] to
//! make that easy and consistent across all handlers.

use actix_web::HttpRequest;
use actix_web::HttpResponseBuilder;

// ---------------------------------------------------------------------------
// Query options (parsed from request)
// ---------------------------------------------------------------------------

/// Consistency mode for Consul reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsistencyMode {
    /// Default: reads from leader.
    Default,
    /// `?stale` — allow reads from any server.
    Stale,
    /// `?consistent` — force quorum-verified read.
    Consistent,
    /// `?cached` — allow agent-side cache hit.
    Cached,
}

impl ConsistencyMode {
    /// Header value for `X-Consul-Effective-Consistency`.
    pub fn as_header_value(&self) -> &'static str {
        match self {
            Self::Default => "leader",
            Self::Stale => "stale",
            Self::Consistent => "consistent",
            Self::Cached => "leader",
        }
    }
}

/// Options parsed from Consul query parameters.
///
/// Mirrors the Go `structs.QueryOptions`.
#[derive(Debug, Clone)]
pub struct ConsulQueryOptions {
    pub consistency: ConsistencyMode,
    /// Blocking query: minimum index to wait for (`?index`).
    pub min_query_index: Option<u64>,
    /// Blocking query: max wait time (`?wait`).
    pub max_query_time: Option<String>,
    /// Datacenter (`?dc` or `?datacenter`).
    pub dc: Option<String>,
    /// Namespace (`?ns` or `?namespace`).
    pub ns: Option<String>,
    /// Filter expression (`?filter`).
    pub filter: Option<String>,
    /// Node-meta filter (`?node-meta=key:value`).
    pub node_meta: Vec<(String, String)>,
    /// Near node for RTT sorting (`?near`).
    pub near: Option<String>,
}

impl ConsulQueryOptions {
    /// Parse query options from an actix-web [`HttpRequest`].
    ///
    /// Handles `?stale`, `?consistent`, `?cached`, `?index`, `?wait`,
    /// `?dc`, `?datacenter`, `?ns`, `?namespace`, `?filter`, `?near`,
    /// `?node-meta`.
    pub fn from_request(req: &HttpRequest) -> Self {
        let qs = req.query_string();

        let mut opts = Self {
            consistency: ConsistencyMode::Default,
            min_query_index: None,
            max_query_time: None,
            dc: None,
            ns: None,
            filter: None,
            node_meta: Vec::new(),
            near: None,
        };

        let mut has_stale = false;
        let mut has_consistent = false;
        let mut has_cached = false;

        for part in qs.split('&') {
            if part.is_empty() {
                continue;
            }
            let (key, value) = match part.find('=') {
                Some(pos) => (&part[..pos], &part[pos + 1..]),
                None => (part, ""),
            };
            match key {
                "stale" => has_stale = true,
                "consistent" => has_consistent = true,
                "cached" => has_cached = true,
                "index" => opts.min_query_index = value.parse().ok(),
                "wait" => {
                    if !value.is_empty() {
                        opts.max_query_time = Some(value.to_string());
                    }
                }
                "dc" | "datacenter" => {
                    if !value.is_empty() {
                        opts.dc = Some(value.to_string());
                    }
                }
                "ns" | "namespace" => {
                    if !value.is_empty() {
                        opts.ns = Some(value.to_string());
                    }
                }
                "filter" => {
                    if !value.is_empty() {
                        opts.filter = Some(value.to_string());
                    }
                }
                "near" => {
                    if !value.is_empty() {
                        opts.near = Some(value.to_string());
                    }
                }
                "node-meta" => {
                    // Format: key:value
                    if let Some(pos) = value.find(':') {
                        let k = value[..pos].to_string();
                        let v = value[pos + 1..].to_string();
                        opts.node_meta.push((k, v));
                    }
                }
                _ => {}
            }
        }

        // Resolve consistency mode (stale and consistent are mutually exclusive;
        // if both are present, Consul returns 400 — we pick stale as fallback).
        if has_consistent && !has_stale {
            opts.consistency = ConsistencyMode::Consistent;
        } else if has_stale {
            opts.consistency = ConsistencyMode::Stale;
        } else if has_cached {
            opts.consistency = ConsistencyMode::Cached;
        }

        opts
    }

    /// Returns true if this is a blocking query (`?index` > 0).
    pub fn is_blocking(&self) -> bool {
        matches!(self.min_query_index, Some(idx) if idx > 0)
    }
}

// ---------------------------------------------------------------------------
// Response metadata
// ---------------------------------------------------------------------------

/// Response metadata matching Consul's `structs.QueryMeta`.
///
/// All Consul query responses set these headers via `setMeta()` in the
/// original Go implementation (`agent/http.go:840`).
#[derive(Debug, Clone)]
pub struct ConsulResponseMeta {
    /// Current Raft index for this response. Minimum 1.
    pub index: u64,
    /// Whether a known leader exists.
    pub known_leader: bool,
    /// Milliseconds since last contact with leader.
    pub last_contact: u64,
    /// The effective consistency mode used.
    pub consistency: ConsistencyMode,
    /// Which query backend handled this request.
    pub query_backend: &'static str,
    /// Whether results were filtered by ACLs.
    pub filtered_by_acls: bool,
}

impl ConsulResponseMeta {
    /// Create response metadata with the given index.
    /// Index is clamped to >= 1 to prevent blocking-query busy loops.
    pub fn new(index: u64) -> Self {
        Self {
            index: index.max(1),
            known_leader: true,
            last_contact: 0,
            consistency: ConsistencyMode::Default,
            query_backend: "blocking-query",
            filtered_by_acls: false,
        }
    }

    /// Set consistency from parsed query options.
    pub fn with_consistency(mut self, mode: ConsistencyMode) -> Self {
        self.consistency = mode;
        self
    }

    /// Mark results as filtered by ACLs.
    pub fn with_acl_filtered(mut self) -> Self {
        self.filtered_by_acls = true;
        self
    }

    /// Apply all standard `X-Consul-*` response headers to the builder.
    ///
    /// This matches the original Consul `setMeta()` function which sets:
    /// - `X-Consul-Index`
    /// - `X-Consul-KnownLeader`
    /// - `X-Consul-LastContact`
    /// - `X-Consul-Effective-Consistency`
    /// - `X-Consul-Query-Backend`
    /// - `X-Consul-Results-Filtered-By-ACLs` (only when true)
    pub fn apply_headers(&self, builder: &mut HttpResponseBuilder) {
        builder.insert_header(("X-Consul-Index", self.index.to_string()));
        builder.insert_header((
            "X-Consul-KnownLeader",
            if self.known_leader { "true" } else { "false" },
        ));
        builder.insert_header(("X-Consul-LastContact", self.last_contact.to_string()));
        builder.insert_header((
            "X-Consul-Effective-Consistency",
            self.consistency.as_header_value(),
        ));
        builder.insert_header(("X-Consul-Query-Backend", self.query_backend));
        if self.filtered_by_acls {
            builder.insert_header(("X-Consul-Results-Filtered-By-ACLs", "true"));
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience: build an HttpResponse with consul headers pre-applied
// ---------------------------------------------------------------------------

/// Start building an HTTP 200 response with all Consul metadata headers set.
///
/// Usage:
/// ```ignore
/// let meta = ConsulResponseMeta::new(index);
/// consul_ok(&meta).json(data)
/// ```
pub fn consul_ok(meta: &ConsulResponseMeta) -> HttpResponseBuilder {
    let mut builder = actix_web::HttpResponse::Ok();
    meta.apply_headers(&mut builder);
    builder
}

/// Start building an HTTP 404 response with Consul metadata headers set.
///
/// Some Consul endpoints return 404 with X-Consul-Index when keys don't exist.
pub fn consul_not_found(meta: &ConsulResponseMeta) -> HttpResponseBuilder {
    let mut builder = actix_web::HttpResponse::NotFound();
    meta.apply_headers(&mut builder);
    builder
}

// ---------------------------------------------------------------------------
// KV raw mode security headers
// ---------------------------------------------------------------------------

/// Apply security headers for KV raw mode responses.
///
/// Matches Consul's `setHeaders` in `kvs_endpoint.go` which prevents
/// user-supplied content from being executed as scripts.
pub fn apply_kv_raw_security_headers(builder: &mut HttpResponseBuilder) {
    builder.insert_header(("X-Content-Type-Options", "nosniff"));
    builder.insert_header(("Content-Security-Policy", "sandbox"));
}

// ---------------------------------------------------------------------------
// Multi-value query parameter parsing
// ---------------------------------------------------------------------------

/// Parse all values of a repeated query parameter (e.g., `?tag=v1&tag=v2`).
///
/// Standard serde deserialization only captures the last value for repeated params.
/// Consul allows multiple `?tag=` params with AND semantics.
pub fn parse_multi_param(req: &HttpRequest, param_name: &str) -> Vec<String> {
    let qs = req.query_string();
    let mut values = Vec::new();
    for part in qs.split('&') {
        if part.is_empty() {
            continue;
        }
        let (key, value) = match part.find('=') {
            Some(pos) => (&part[..pos], &part[pos + 1..]),
            None => (part, ""),
        };
        if key == param_name && !value.is_empty() {
            values.push(value.to_string());
        }
    }
    values
}

// ---------------------------------------------------------------------------
// Go time.Duration compatible parsing
// ---------------------------------------------------------------------------

/// Parse a Go `time.Duration` string to `std::time::Duration`.
///
/// Supports all Go duration units: `ns`, `us`/`µs`, `ms`, `s`, `m`, `h`.
/// Supports compound formats: `1h30m`, `2h0m30s`, `500ms`, etc.
/// Supports bare numeric strings interpreted as seconds (Consul convention).
///
/// This is the **single canonical implementation** — all Consul duration parsing
/// in Batata should use this function.
pub fn parse_go_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // If it's a bare number, treat as seconds (Consul convention)
    if let Ok(secs) = s.parse::<f64>() {
        return if secs >= 0.0 {
            Some(std::time::Duration::from_secs_f64(secs))
        } else {
            None
        };
    }

    let mut total_nanos: u128 = 0;
    let mut current_num = String::new();
    let mut chars = s.chars().peekable();
    let mut matched_any = false;

    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() || ch == '.' {
            current_num.push(ch);
            chars.next();
        } else {
            if current_num.is_empty() {
                return None; // unit without number
            }
            let num: f64 = current_num.parse().ok()?;
            current_num.clear();
            matched_any = true;

            // Collect unit string (may be multi-char: "ms", "us", "ns", "µs")
            let mut unit = String::new();
            unit.push(ch);
            chars.next();

            // Check for two-char units
            if let Some(&next_ch) = chars.peek()
                && ((ch == 'm' && next_ch == 's')
                    || (ch == 'u' && next_ch == 's')
                    || (ch == 'n' && next_ch == 's')
                    || (ch == 'µ' && next_ch == 's'))
            {
                unit.push(next_ch);
                chars.next();
            }

            let nanos_per_unit: f64 = match unit.as_str() {
                "ns" => 1.0,
                "us" | "µs" => 1_000.0,
                "ms" => 1_000_000.0,
                "s" => 1_000_000_000.0,
                "m" => 60_000_000_000.0,
                "h" => 3_600_000_000_000.0,
                _ => return None,
            };

            total_nanos += (num * nanos_per_unit) as u128;
        }
    }

    // Handle trailing number without unit (treat as seconds)
    if !current_num.is_empty() {
        if matched_any {
            return None; // e.g., "5s30" — trailing number after units is invalid
        }
        let num: f64 = current_num.parse().ok()?;
        total_nanos += (num * 1_000_000_000.0) as u128;
        matched_any = true;
    }

    if !matched_any {
        return None;
    }

    Some(std::time::Duration::from_nanos(total_nanos as u64))
}

/// Parse a Go duration string and return seconds (u64).
///
/// Convenience wrapper for code that needs seconds instead of Duration.
pub fn parse_go_duration_secs(s: &str) -> Option<u64> {
    parse_go_duration(s).map(|d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    #[test]
    fn test_parse_default_consistency() {
        let req = TestRequest::with_uri("/v1/kv/test").to_http_request();
        let opts = ConsulQueryOptions::from_request(&req);
        assert_eq!(opts.consistency, ConsistencyMode::Default);
        assert!(!opts.is_blocking());
    }

    #[test]
    fn test_parse_stale() {
        let req = TestRequest::with_uri("/v1/kv/test?stale").to_http_request();
        let opts = ConsulQueryOptions::from_request(&req);
        assert_eq!(opts.consistency, ConsistencyMode::Stale);
    }

    #[test]
    fn test_parse_consistent() {
        let req = TestRequest::with_uri("/v1/kv/test?consistent").to_http_request();
        let opts = ConsulQueryOptions::from_request(&req);
        assert_eq!(opts.consistency, ConsistencyMode::Consistent);
    }

    #[test]
    fn test_parse_cached() {
        let req = TestRequest::with_uri("/v1/kv/test?cached").to_http_request();
        let opts = ConsulQueryOptions::from_request(&req);
        assert_eq!(opts.consistency, ConsistencyMode::Cached);
    }

    #[test]
    fn test_parse_blocking_query() {
        let req = TestRequest::with_uri("/v1/kv/test?index=42&wait=5m").to_http_request();
        let opts = ConsulQueryOptions::from_request(&req);
        assert!(opts.is_blocking());
        assert_eq!(opts.min_query_index, Some(42));
        assert_eq!(opts.max_query_time.as_deref(), Some("5m"));
    }

    #[test]
    fn test_parse_dc_datacenter() {
        let req = TestRequest::with_uri("/v1/kv/test?dc=us-east-1").to_http_request();
        let opts = ConsulQueryOptions::from_request(&req);
        assert_eq!(opts.dc.as_deref(), Some("us-east-1"));

        let req = TestRequest::with_uri("/v1/kv/test?datacenter=us-west-2").to_http_request();
        let opts = ConsulQueryOptions::from_request(&req);
        assert_eq!(opts.dc.as_deref(), Some("us-west-2"));
    }

    #[test]
    fn test_parse_node_meta() {
        let req = TestRequest::with_uri("/v1/catalog/nodes?node-meta=env:prod").to_http_request();
        let opts = ConsulQueryOptions::from_request(&req);
        assert_eq!(opts.node_meta.len(), 1);
        assert_eq!(opts.node_meta[0], ("env".to_string(), "prod".to_string()));
    }

    #[test]
    fn test_response_meta_index_minimum_one() {
        let meta = ConsulResponseMeta::new(0);
        assert_eq!(meta.index, 1);
    }

    #[test]
    fn test_response_meta_preserves_index() {
        let meta = ConsulResponseMeta::new(42);
        assert_eq!(meta.index, 42);
    }

    // -----------------------------------------------------------------------
    // Go duration parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_go_duration_simple_units() {
        // Nanoseconds
        assert_eq!(
            parse_go_duration("100ns"),
            Some(std::time::Duration::from_nanos(100))
        );
        // Microseconds
        assert_eq!(
            parse_go_duration("50us"),
            Some(std::time::Duration::from_micros(50))
        );
        assert_eq!(
            parse_go_duration("50µs"),
            Some(std::time::Duration::from_micros(50))
        );
        // Milliseconds
        assert_eq!(
            parse_go_duration("500ms"),
            Some(std::time::Duration::from_millis(500))
        );
        assert_eq!(
            parse_go_duration("100ms"),
            Some(std::time::Duration::from_millis(100))
        );
        // Seconds
        assert_eq!(
            parse_go_duration("30s"),
            Some(std::time::Duration::from_secs(30))
        );
        // Minutes
        assert_eq!(
            parse_go_duration("5m"),
            Some(std::time::Duration::from_secs(300))
        );
        // Hours
        assert_eq!(
            parse_go_duration("2h"),
            Some(std::time::Duration::from_secs(7200))
        );
    }

    #[test]
    fn test_parse_go_duration_compound() {
        // 1h30m
        assert_eq!(
            parse_go_duration("1h30m"),
            Some(std::time::Duration::from_secs(5400))
        );
        // 1h0m30s
        assert_eq!(
            parse_go_duration("1h0m30s"),
            Some(std::time::Duration::from_secs(3630))
        );
        // 2h30m45s
        assert_eq!(
            parse_go_duration("2h30m45s"),
            Some(std::time::Duration::from_secs(9045))
        );
        // 1m30s
        assert_eq!(
            parse_go_duration("1m30s"),
            Some(std::time::Duration::from_secs(90))
        );
        // 500ms mixed
        assert_eq!(
            parse_go_duration("1s500ms"),
            Some(std::time::Duration::from_millis(1500))
        );
    }

    #[test]
    fn test_parse_go_duration_bare_number() {
        // Bare number = seconds (Consul convention)
        assert_eq!(
            parse_go_duration("60"),
            Some(std::time::Duration::from_secs(60))
        );
        assert_eq!(
            parse_go_duration("0"),
            Some(std::time::Duration::from_secs(0))
        );
    }

    #[test]
    fn test_parse_go_duration_edge_cases() {
        assert_eq!(parse_go_duration(""), None);
        assert_eq!(parse_go_duration("   "), None);
        assert_eq!(parse_go_duration("abc"), None);
        assert_eq!(
            parse_go_duration("0s"),
            Some(std::time::Duration::from_secs(0))
        );
    }

    #[test]
    fn test_parse_go_duration_secs_helper() {
        assert_eq!(parse_go_duration_secs("30s"), Some(30));
        assert_eq!(parse_go_duration_secs("5m"), Some(300));
        assert_eq!(parse_go_duration_secs("2h"), Some(7200));
        assert_eq!(parse_go_duration_secs("1h30m"), Some(5400));
        // ms rounds down to 0 seconds
        assert_eq!(parse_go_duration_secs("500ms"), Some(0));
        assert_eq!(parse_go_duration_secs(""), None);
    }
}
