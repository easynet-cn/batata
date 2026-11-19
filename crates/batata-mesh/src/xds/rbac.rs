//! Intentions -> Envoy RBAC translation.
//!
//! This module is deliberately free of any `batata-plugin-consul` dependency:
//! the dependency direction is `consul -> mesh`, so the plugin converts its
//! `Intention` rows into the native [`IntentionsForService`] input here, and
//! this module produces byte-for-byte Envoy RBAC filter configs.
//!
//! The precedence-reduction algorithm mirrors upstream Consul
//! `agent/xds/rbac.go` (`intentionListToIntermediateRBACForm`,
//! `removeSourcePrecedence`, `removePermissionPrecedence`, `makeRBACRules`),
//! adapted to batata's simpler intention model (no partition/peer/JWT/XFCC):
//!
//! * Intentions are sorted by descending precedence; lower-precedence entries
//!   sharing the same source are dropped.
//! * Each retained entry's principal is `source AND NOT <each higher-precedence
//!   source that matches it>`, so the RBAC policies become order-independent.
//! * Because batata's default policy is ALLOW, the emitted RBAC action is DENY
//!   (block-list): policies enumerate exactly the principals/permissions that
//!   must be rejected; everything else is allowed by Envoy.
//! * An L7 intention with permissions has no L4 representation; in the network
//!   filter it is treated as DENY (matching upstream), and its per-permission
//!   rules are emitted in the HTTP filter.

use std::collections::{HashMap, HashSet};

use prost::Message as _;

use crate::xds::generated::rbac as pb;

/// Source/destination service name plus its precedence-derived metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacService {
    /// Service name as used in intentions (`*` means wildcard).
    pub name: String,
}

/// Top-level action of an intention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentionAction {
    /// Explicitly allow the connection/request.
    Allow,
    /// Explicitly deny the connection/request.
    Deny,
}

/// One L7 HTTP permission match, mirroring batata's `IntentionHTTPPermission`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpPermissionMatch {
    /// Exact path match (mutually exclusive with prefix/regex).
    pub path_exact: Option<String>,
    /// Prefix path match.
    pub path_prefix: Option<String>,
    /// Regex path match (RE2-compatible).
    pub path_regex: Option<String>,
    /// HTTP methods (any-of semantics).
    pub methods: Vec<String>,
}

/// One permission within an L7 intention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacPermission {
    /// Whether this permission allows or denies a matching request.
    pub action: IntentionAction,
    /// Optional HTTP match conditions (None = matches any request).
    pub http: Option<HttpPermissionMatch>,
}

/// One intention, converted from the plugin's store into a dependency-free form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacIntention {
    /// Source service name (`*` for wildcard).
    pub source: String,
    /// Intention precedence (higher wins).
    pub precedence: i32,
    /// Top-level L4 action.
    pub action: IntentionAction,
    /// L7 permissions (non-empty means this is an L7 intention).
    pub permissions: Vec<RbacPermission>,
}

/// All intentions that target one destination service, plus the SPIFFE inputs
/// needed to build source principals.
#[derive(Debug, Clone)]
pub struct IntentionsForService {
    /// Destination service name (used only for diagnostics / naming).
    pub destination: String,
    /// Trust domain used in SPIFFE IDs (batata uses `consul`).
    pub trust_domain: String,
    /// Datacenter used in SPIFFE IDs.
    pub datacenter: String,
    /// Intentions targeting this service.
    pub intentions: Vec<RbacIntention>,
}

/// Result of the translation: byte-encoded network and HTTP RBAC filter configs.
#[derive(Debug, Clone)]
pub struct EncodedRbacFilters {
    /// Serialized `envoy.extensions.filters.network.rbac.v3.RBAC` config.
    pub network_config: Vec<u8>,
    /// Serialized `envoy.extensions.filters.http.rbac.v3.RBAC` config, when the
    /// destination has any L7 intention.
    pub http_config: Option<Vec<u8>>,
    /// True when the network filter has at least one policy (worth emitting).
    pub has_network_policies: bool,
    /// True when the HTTP filter has at least one policy.
    pub has_http_policies: bool,
}

// ---------------------------------------------------------------------------
// Native RBAC tree builders
// ---------------------------------------------------------------------------

fn any_permission() -> pb::Permission {
    pb::Permission {
        rule: Some(pb::permission::Rule::Any(true)),
    }
}

fn and_principals(ids: Vec<pb::Principal>) -> pb::Principal {
    match ids.len() {
        1 => ids.into_iter().next().unwrap(),
        _ => pb::Principal {
            identifier: Some(pb::principal::Identifier::AndIds(pb::principal::Set {
                ids,
            })),
        },
    }
}

fn not_principal(id: pb::Principal) -> pb::Principal {
    pb::Principal {
        identifier: Some(pb::principal::Identifier::NotId(Box::new(id))),
    }
}

fn and_permissions(rules: Vec<pb::Permission>) -> pb::Permission {
    match rules.len() {
        0 => any_permission(),
        1 => rules.into_iter().next().unwrap(),
        _ => pb::Permission {
            rule: Some(pb::permission::Rule::AndRules(pb::permission::Set {
                rules,
            })),
        },
    }
}

fn or_permissions(rules: Vec<pb::Permission>) -> pb::Permission {
    match rules.len() {
        0 => any_permission(),
        1 => rules.into_iter().next().unwrap(),
        _ => pb::Permission {
            rule: Some(pb::permission::Rule::OrRules(pb::permission::Set { rules })),
        },
    }
}

fn not_permission(rule: pb::Permission) -> pb::Permission {
    pb::Permission {
        rule: Some(pb::permission::Rule::NotRule(Box::new(rule))),
    }
}

/// Escape regex metacharacters in a user-controlled service name. This is a
/// hand-rolled equivalent of Go's `regexp.QuoteMeta` for the ASCII RE2 subset.
/// Failing to escape would let a service name like `web|admin` broaden the
/// RBAC match and bypass intention enforcement.
fn quote_meta(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        if matches!(
            ch,
            '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' | '\\'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Build the SPIFFE regex pattern for a source service. The datacenter segment
/// is intentionally a wildcard (`[^/]+`) because Envoy RBAC does not verify DC
/// (upstream Consul does the same).
fn spiffe_pattern(src: &str, trust_domain: &str) -> String {
    let host = quote_meta(trust_domain);
    let svc = if src == "*" {
        "[^/]+".to_string()
    } else {
        quote_meta(src)
    };
    format!(
        "^spiffe://{host}/ns/default/dc/[^/]+/svc/{svc}$",
        host = host,
        svc = svc
    )
}

/// A `Principal_Authenticated` with a SafeRegex match on the SPIFFE URI SAN.
fn spiffe_principal(src: &str, trust_domain: &str) -> pb::Principal {
    let pattern = spiffe_pattern(src, trust_domain);
    pb::Principal {
        identifier: Some(pb::principal::Identifier::Authenticated(
            pb::principal::Authenticated {
                principal_name: Some(pb::StringMatcher {
                    match_pattern: Some(pb::string_matcher::MatchPattern::SafeRegex(
                        pb::RegexMatcher {
                            engine_type: Some(pb::regex_matcher::EngineType::GoogleRe2(
                                pb::regex_matcher::GoogleRe2 {},
                            )),
                            regex: pattern,
                        },
                    )),
                    ignore_case: false,
                }),
            },
        )),
    }
}

/// Does source `tester` fall within the wildcard scope of `against`?
fn source_matches(against: &str, tester: &str) -> bool {
    against == "*" || against == tester
}

/// Compute precedence exactly as batata's `compute_precedence` does.
pub fn compute_precedence(source: &str, destination: &str) -> i32 {
    match (source, destination) {
        ("*", "*") => 1,
        (_, "*") => 2,
        ("*", _) => 3,
        (_, _) => 4,
    }
}

// ---------------------------------------------------------------------------
// Precedence reduction (ported from upstream rbac.go)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct IntermediateIntention {
    source: String,
    /// Higher-precedence sources that must NOT also match this intention.
    not_sources: Vec<String>,
    action: IntentionAction,
    /// L7 permissions, already annotated with their own not-rules.
    permissions: Vec<IntermediatePermission>,
    is_l7: bool,
    computed_principal: Option<pb::Principal>,
}

#[derive(Debug, Clone)]
struct IntermediatePermission {
    action: IntentionAction,
    permission: pb::Permission,
    not_permissions: Vec<pb::Permission>,
    computed: Option<pb::Permission>,
}

/// Sort by descending precedence, then drop lower-precedence entries that
/// share a source with a higher-precedence one (upstream
/// `removeSameSourceIntentions`).
fn sort_and_dedup(intentions: &[RbacIntention]) -> Vec<RbacIntention> {
    let mut sorted: Vec<RbacIntention> = intentions.to_vec();
    sorted.sort_by(|a, b| b.precedence.cmp(&a.precedence));
    let mut seen = HashSet::new();
    sorted
        .into_iter()
        .filter(|i| seen.insert(i.source.clone()))
        .collect()
}

fn to_intermediate(i: &RbacIntention) -> IntermediateIntention {
    let is_l7 = !i.permissions.is_empty();
    let permissions = i
        .permissions
        .iter()
        .map(|p| IntermediatePermission {
            action: p.action,
            permission: permission_to_proto(p.http.as_ref()),
            not_permissions: Vec::new(),
            computed: None,
        })
        .collect();
    IntermediateIntention {
        source: i.source.clone(),
        not_sources: Vec::new(),
        // In the L4 (network) filter an L7 intention is treated as DENY.
        action: if is_l7 {
            IntentionAction::Deny
        } else {
            i.action
        },
        permissions,
        is_l7,
        computed_principal: None,
    }
}

/// Reduce source precedence: walk backwards, adding each higher-precedence
/// source as a `NOT` constraint on every lower-precedence entry it matches;
/// skip entries whose action equals the default (allow) action because the
/// catch-all default already covers them.
// The reduction algorithm walks a reversed outer index and a forward inner
// index over the same slice, distributing higher-precedence entries into
// lower ones. That bidirectional indexed access is clearer as explicit loops
// than as split_at/iter_mut gymnastics.
#[allow(clippy::needless_range_loop)]
fn remove_source_precedence(
    mut ixns: Vec<IntermediateIntention>,
    default_action: IntentionAction,
    trust_domain: &str,
) -> Vec<IntermediateIntention> {
    let n = ixns.len();
    for i in (0..n).rev() {
        let higher_source = ixns[i].source.clone();
        for j in (i + 1)..n {
            // `source_matches(against, tester)`: is `tester` covered by
            // `against`'s wildcard? Here the higher-precedence source is the
            // tester and the lower-precedence one is `against`. If covered,
            // the higher one must be excluded from the lower one's principal.
            if source_matches(&ixns[j].source, &higher_source) {
                ixns[j].not_sources.push(higher_source.clone());
            }
        }
    }
    let mut out = Vec::new();
    for mut ixn in ixns {
        if ixn.action == default_action && !ixn.is_l7 {
            // L4 entry with the default action is handled by the catch-all.
            continue;
        }
        let mut ids = vec![spiffe_principal(&ixn.source, trust_domain)];
        for src in &ixn.not_sources {
            ids.push(not_principal(spiffe_principal(src, trust_domain)));
        }
        ixn.computed_principal = Some(and_principals(ids));
        out.push(ixn);
    }
    out
}

#[allow(clippy::needless_range_loop)]
fn remove_permission_precedence(
    mut perms: Vec<IntermediatePermission>,
    default_action: IntentionAction,
) -> Vec<IntermediatePermission> {
    let n = perms.len();
    for i in (0..n).rev() {
        let higher_perm = perms[i].permission.clone();
        for j in (i + 1)..n {
            perms[j].not_permissions.push(higher_perm.clone());
        }
    }
    let mut out = Vec::new();
    for mut perm in perms {
        if perm.action == default_action {
            continue;
        }
        let mut rules = vec![perm.permission.clone()];
        for not in &perm.not_permissions {
            rules.push(not_permission(not.clone()));
        }
        perm.computed = Some(and_permissions(rules));
        out.push(perm);
    }
    out
}

fn reduce_intentions(
    mut ixns: Vec<IntermediateIntention>,
    default_action: IntentionAction,
    trust_domain: &str,
) -> Vec<IntermediateIntention> {
    ixns = remove_source_precedence(ixns, default_action, trust_domain);
    for ixn in &mut ixns {
        ixn.permissions =
            remove_permission_precedence(std::mem::take(&mut ixn.permissions), default_action);
    }
    // L7 intentions are kept even when all their HTTP permissions collapse
    // to the default action: their principal still contributes a network-layer
    // DENY (raw TCP from that source must be rejected). The HTTP policy itself
    // is only emitted when at least one permission survives (see caller).
    ixns
}

// ---------------------------------------------------------------------------
// Permission proto construction
// ---------------------------------------------------------------------------

fn string_matcher_exact(s: &str) -> pb::StringMatcher {
    pb::StringMatcher {
        match_pattern: Some(pb::string_matcher::MatchPattern::Exact(s.to_string())),
        ignore_case: false,
    }
}

fn string_matcher_prefix(s: &str) -> pb::StringMatcher {
    pb::StringMatcher {
        match_pattern: Some(pb::string_matcher::MatchPattern::Prefix(s.to_string())),
        ignore_case: false,
    }
}

fn regex_matcher(regex: String) -> pb::RegexMatcher {
    pb::RegexMatcher {
        engine_type: Some(pb::regex_matcher::EngineType::GoogleRe2(
            pb::regex_matcher::GoogleRe2 {},
        )),
        regex,
    }
}

fn string_matcher_regex(regex: String) -> pb::StringMatcher {
    pb::StringMatcher {
        match_pattern: Some(pb::string_matcher::MatchPattern::SafeRegex(regex_matcher(
            regex,
        ))),
        ignore_case: false,
    }
}

/// Build one Envoy Permission for a single HTTP permission match. All match
/// dimensions present are ANDed together; absent dimensions match anything.
fn permission_to_proto(http: Option<&HttpPermissionMatch>) -> pb::Permission {
    let Some(http) = http else {
        return any_permission();
    };

    let mut parts: Vec<pb::Permission> = Vec::new();

    // Path: exact / prefix / regex are mutually exclusive, mirroring both
    // batata's evaluation and upstream's switch statement.
    if let Some(path) = &http.path_exact {
        parts.push(pb::Permission {
            rule: Some(pb::permission::Rule::UrlPath(pb::PathMatcher {
                rule: Some(pb::path_matcher::Rule::Path(string_matcher_exact(path))),
            })),
        });
    } else if let Some(path) = &http.path_prefix {
        parts.push(pb::Permission {
            rule: Some(pb::permission::Rule::UrlPath(pb::PathMatcher {
                rule: Some(pb::path_matcher::Rule::Path(string_matcher_prefix(path))),
            })),
        });
    } else if let Some(path) = &http.path_regex {
        parts.push(pb::Permission {
            rule: Some(pb::permission::Rule::UrlPath(pb::PathMatcher {
                rule: Some(pb::path_matcher::Rule::Path(string_matcher_regex(
                    path.clone(),
                ))),
            })),
        });
    }

    // Methods: one exact `:method` matcher per method, ORed together. This
    // avoids depending on the newer HeaderMatcher.string_match field and
    // reuses the stable exact_match=4 tag. It matches batata's "any-of" method
    // semantics (HTTP methods are conventionally upper-case).
    if !http.methods.is_empty() {
        let mut method_rules = Vec::new();
        for method in &http.methods {
            method_rules.push(pb::Permission {
                rule: Some(pb::permission::Rule::Header(pb::HeaderMatcher {
                    name: ":method".to_string(),
                    header_match_specifier: Some(
                        pb::header_matcher::HeaderMatchSpecifier::ExactMatch(method.clone()),
                    ),
                    invert_match: false,
                })),
            });
        }
        parts.push(or_permissions(method_rules));
    }

    and_permissions(parts)
}

// ---------------------------------------------------------------------------
// Top-level translation
// ---------------------------------------------------------------------------

/// Translate all intentions targeting one service into encoded Envoy RBAC
/// filter configs. Returns `None` only when there are no intentions at all
/// (in which case batata's default-allow means no filter should be emitted).
pub fn encode_rbac_filters(input: &IntentionsForService) -> Option<EncodedRbacFilters> {
    if input.intentions.is_empty() {
        return None;
    }

    // L4 and L7 are two independent RBAC generation passes (upstream Consul
    // calls makeRBACRules twice with isHTTP=false/true for the same intentions).
    // They intentionally use opposite actions because batata has a mixed
    // default policy: overall default ALLOW (L4) but per-L7-intention default
    // DENY when permissions are present.
    //
    //   network filter: ALLOW is the global default -> DENY action (blacklist
    //                   of L4-denied source principals).
    //   HTTP filter:    L7 default is deny          -> ALLOW action (whitelist
    //                   of L7 permission matches), plus a catch-all that lets
    //                   through sources with no L7 intention.
    let deduped = sort_and_dedup(&input.intentions);

    // ---- L4 pass: DENY action, default action = Allow -----------------------
    let l4_only: Vec<RbacIntention> = deduped
        .iter()
        .filter(|i| i.permissions.is_empty())
        .cloned()
        .collect();
    let mut l4_principals: Vec<pb::Principal> = Vec::new();
    if !l4_only.is_empty() {
        let intermediates: Vec<IntermediateIntention> =
            l4_only.iter().map(to_intermediate).collect();
        let reduced = reduce_intentions(intermediates, IntentionAction::Allow, &input.trust_domain);
        for ixn in reduced {
            if let Some(principal) = ixn.computed_principal {
                l4_principals.push(principal);
            }
        }
        l4_principals.dedup();
    }

    let mut network_rules = pb::Rbac {
        action: pb::rbac::Action::Deny as i32,
        policies: HashMap::new(),
    };
    let has_network_policies = !l4_principals.is_empty();
    if has_network_policies {
        network_rules.policies.insert(
            "consul-intentions-layer4".to_string(),
            pb::Policy {
                permissions: vec![any_permission()],
                principals: l4_principals,
            },
        );
    }
    let network_config = pb::NetworkRbacFilterConfig {
        rules: Some(network_rules),
        shadow_rules: None,
        stat_prefix: "connect_authz".to_string(),
    }
    .encode_to_vec();

    // ---- L7 pass: ALLOW action, default action = Deny ----------------------
    let l7_only: Vec<RbacIntention> = deduped
        .iter()
        .filter(|i| !i.permissions.is_empty())
        .cloned()
        .collect();
    let mut http_policies: HashMap<String, pb::Policy> = HashMap::new();
    let mut l7_sources: Vec<String> = Vec::new();
    if !l7_only.is_empty() {
        let intermediates: Vec<IntermediateIntention> =
            l7_only.iter().map(to_intermediate).collect();
        let reduced = reduce_intentions(intermediates, IntentionAction::Deny, &input.trust_domain);
        let mut l7_count = 0usize;
        for ixn in reduced {
            l7_sources.push(ixn.source.clone());
            let permissions: Vec<pb::Permission> = ixn
                .permissions
                .into_iter()
                .filter_map(|p| p.computed)
                .collect();
            let Some(principal) = ixn.computed_principal else {
                continue;
            };
            if permissions.is_empty() {
                continue;
            }
            let key = format!("consul-intentions-layer7-{}", l7_count);
            l7_count += 1;
            http_policies.insert(
                key,
                pb::Policy {
                    permissions,
                    principals: vec![principal],
                },
            );
        }

        // Sources that have no L7 intention must keep the overall default-allow
        // behavior. Emit a catch-all policy whose principal is
        // `NOT(l7_source_1) AND NOT(l7_source_2) ...`, matching any downstream
        // that is not governed by an L7 intention.
        if !l7_sources.is_empty() {
            let mut not_ids: Vec<pb::Principal> = l7_sources
                .iter()
                .map(|s| not_principal(spiffe_principal(s, &input.trust_domain)))
                .collect();
            // AND NOT all L7 sources; collapse wildcard-first for clarity.
            let catch_all = if not_ids.len() == 1 {
                not_ids.remove(0)
            } else {
                and_principals(not_ids)
            };
            http_policies.insert(
                "consul-intentions-default-allow".to_string(),
                pb::Policy {
                    permissions: vec![any_permission()],
                    principals: vec![catch_all],
                },
            );
        }
    }

    let has_http_policies = !http_policies.is_empty();
    let http_config = if has_http_policies {
        Some(
            pb::HttpRbac {
                rules: Some(pb::Rbac {
                    action: pb::rbac::Action::Allow as i32,
                    policies: http_policies,
                }),
                shadow_rules: None,
            }
            .encode_to_vec(),
        )
    } else {
        None
    };

    Some(EncodedRbacFilters {
        network_config,
        http_config,
        has_network_policies,
        has_http_policies,
    })
}

/// Decode an encoded network RBAC filter config. Primarily used by callers
/// and integration tests to assert the emitted rules without depending on
/// prost directly.
pub fn decode_network_filter_config(
    bytes: &[u8],
) -> Result<pb::NetworkRbacFilterConfig, prost::DecodeError> {
    pb::NetworkRbacFilterConfig::decode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(intentions: Vec<RbacIntention>) -> IntentionsForService {
        IntentionsForService {
            destination: "api".to_string(),
            trust_domain: "consul".to_string(),
            datacenter: "dc1".to_string(),
            intentions,
        }
    }

    fn l4(source: &str, action: IntentionAction, precedence: i32) -> RbacIntention {
        RbacIntention {
            source: source.to_string(),
            precedence,
            action,
            permissions: vec![],
        }
    }

    #[test]
    fn test_quote_meta_prevents_regex_injection_in_service_name() {
        // A service named "web|admin" must not produce a pattern that matches
        // either "web" or "admin"; the pipe has to be escaped literally.
        let pattern = spiffe_pattern("web|admin", "consul");
        assert!(
            pattern.contains(r"web\|admin"),
            "service name must be regex-escaped: {pattern}"
        );
        assert!(pattern.ends_with("$"));
        assert!(pattern.starts_with("^spiffe://consul/ns/default/dc/[^/]+/svc/"));
    }

    #[test]
    fn test_wildcard_source_uses_any_path() {
        let pattern = spiffe_pattern("*", "consul");
        assert!(
            pattern.contains("/svc/[^/]+$"),
            "wildcard source must map to [^/]+: {pattern}"
        );
    }

    #[test]
    fn test_empty_intentions_returns_none() {
        assert!(encode_rbac_filters(&input(vec![])).is_none());
    }

    #[test]
    fn test_default_allow_emits_deny_action() {
        // batata defaults to allow, so RBAC must be a DENY (block) list.
        let out = encode_rbac_filters(&input(vec![l4("web", IntentionAction::Deny, 4)]))
            .expect("one intention must produce a filter");
        let network = pb::NetworkRbacFilterConfig::decode(out.network_config.as_slice()).unwrap();
        let rules = network.rules.unwrap();
        assert_eq!(rules.action, pb::rbac::Action::Deny as i32);
        assert_eq!(rules.policies.len(), 1);
        let policy = &rules.policies["consul-intentions-layer4"];
        assert_eq!(policy.permissions.len(), 1);
        assert!(matches!(
            policy.permissions[0].rule,
            Some(pb::permission::Rule::Any(true))
        ));
        assert_eq!(network.stat_prefix, "connect_authz");
    }

    #[test]
    fn test_allow_intention_with_default_action_is_not_emitted() {
        // Because default is allow, an explicit ALLOW L4 intention adds no
        // deny rule; with no other intentions there are no network policies.
        let out = encode_rbac_filters(&input(vec![l4("web", IntentionAction::Allow, 4)]))
            .expect("filter config still encoded");
        assert!(
            !out.has_network_policies,
            "an ALLOW-only L4 set must produce no deny policy"
        );
    }

    #[test]
    fn test_precedence_denies_higher_priority_source_via_not_id() {
        // web->api DENY (prec 4) vs *->api ALLOW (prec 3): the wildcard ALLOW
        // is the default action and is skipped, while the deny stays. To prove
        // precedence reduction, use two DENYs where the higher one must be
        // excluded from the lower one's match.
        let out = encode_rbac_filters(&input(vec![
            l4("web", IntentionAction::Deny, 4),
            l4("*", IntentionAction::Deny, 3),
        ]))
        .unwrap();
        let network = pb::NetworkRbacFilterConfig::decode(out.network_config.as_slice()).unwrap();
        let policy = &network.rules.unwrap().policies["consul-intentions-layer4"];

        // Wildcard (lower precedence) principal must be `* AND NOT web`.
        let wildcard_principal = policy
            .principals
            .iter()
            .find(|p| matches!(p.identifier, Some(pb::principal::Identifier::AndIds(_))))
            .expect("lower-precedence wildcard must carry a NOT constraint");
        let Some(pb::principal::Identifier::AndIds(set)) = &wildcard_principal.identifier else {
            unreachable!()
        };
        assert!(
            set.ids
                .iter()
                .any(|p| matches!(p.identifier, Some(pb::principal::Identifier::NotId(_)))),
            "expected a NotId principal excluding the higher-precedence source"
        );
    }

    #[test]
    fn test_l7_permission_emits_http_filter_with_path_and_method() {
        let intention = RbacIntention {
            source: "web".to_string(),
            precedence: 4,
            action: IntentionAction::Allow,
            permissions: vec![RbacPermission {
                action: IntentionAction::Allow,
                http: Some(HttpPermissionMatch {
                    path_prefix: Some("/admin".to_string()),
                    methods: vec!["POST".to_string(), "DELETE".to_string()],
                    ..Default::default()
                }),
            }],
        };
        let out = encode_rbac_filters(&input(vec![intention])).unwrap();
        assert!(out.has_http_policies);
        let http = pb::HttpRbac::decode(out.http_config.unwrap().as_slice()).unwrap();
        let rules = http.rules.unwrap();
        // HTTP filter is a whitelist (ALLOW action) because L7 default is deny.
        assert_eq!(rules.action, pb::rbac::Action::Allow as i32);
        let policy = rules
            .policies
            .get("consul-intentions-layer7-0")
            .expect("one L7 policy expected");
        // permission = AND(path_prefix, OR(methods))
        let and = match &policy.permissions[0].rule {
            Some(pb::permission::Rule::AndRules(set)) => set,
            other => panic!("expected AndRules, got {other:?}"),
        };
        assert_eq!(and.rules.len(), 2);
        assert!(matches!(
            and.rules[0].rule,
            Some(pb::permission::Rule::UrlPath(_))
        ));
        assert!(matches!(
            and.rules[1].rule,
            Some(pb::permission::Rule::OrRules(_))
        ));
    }

    #[test]
    fn test_l7_http_filter_has_default_allow_catch_all() {
        // Sources without an L7 intention must keep the overall default-allow
        // behavior; the HTTP whitelist must carry a NOT(l7_source) catch-all.
        let intention = RbacIntention {
            source: "web".to_string(),
            precedence: 4,
            action: IntentionAction::Allow,
            permissions: vec![RbacPermission {
                action: IntentionAction::Allow,
                http: Some(HttpPermissionMatch {
                    path_exact: Some("/pay".to_string()),
                    ..Default::default()
                }),
            }],
        };
        let out = encode_rbac_filters(&input(vec![intention])).unwrap();
        let http = pb::HttpRbac::decode(out.http_config.unwrap().as_slice()).unwrap();
        let rules = http.rules.unwrap();
        let catch_all = rules
            .policies
            .get("consul-intentions-default-allow")
            .expect("catch-all policy required");
        assert!(matches!(
            catch_all.principals[0].identifier,
            Some(pb::principal::Identifier::NotId(_))
        ));
        assert!(matches!(
            catch_all.permissions[0].rule,
            Some(pb::permission::Rule::Any(true))
        ));
    }

    #[test]
    fn test_l7_intention_does_not_emit_network_policy() {
        // L7 intentions are enforced in the HTTP filter only. A network-layer
        // DENY would reject ALL traffic from that source, including the
        // requests its ALLOW permission intends to permit (a network RBAC
        // filter cannot distinguish HTTP from raw TCP).
        let intention = RbacIntention {
            source: "web".to_string(),
            precedence: 4,
            action: IntentionAction::Allow,
            permissions: vec![RbacPermission {
                action: IntentionAction::Allow,
                http: None,
            }],
        };
        let out = encode_rbac_filters(&input(vec![intention])).unwrap();
        assert!(
            !out.has_network_policies,
            "pure L7 intention must not produce a network DENY policy"
        );
        assert!(out.http_config.is_some());
    }
}
