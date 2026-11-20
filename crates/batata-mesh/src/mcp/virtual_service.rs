//! Projection of tagged Nacos config entries into Envoy RDS resources.
//!
//! Upstream Nacos publishes an in-process config event when a config carries
//! the `virtual-service` or `destination-rule` tag. Its Istio module validates
//! the YAML envelope and compiles `VirtualService` configs into
//! `RouteConfiguration` resources served over ADS on port 18848. Batata keeps
//! the same wire result: tagged configs are parsed here and projected into the
//! shared xDS [`ResourceSnapshot`], merged with naming-derived routes.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::warn;

use crate::conversion::NacosService;
use crate::snapshot::ResourceSnapshot;
use crate::sync::{SyncBridgeConfig, build_resource_snapshot};
use crate::xds::types::{
    Route, RouteAction, RouteConfiguration, RouteDestination, RouteMatch, VirtualHost,
};

use super::types::{
    Destination, HttpMatchRequest, HttpRoute, HttpRouteDestination, ResourceMetadata, StringMatch,
    VirtualService, VirtualServiceSpec,
};

/// Required upstream Istio apiVersion for config-derived VirtualServices.
pub const VIRTUAL_SERVICE_API_VERSION: &str = "networking.istio.io/v1alpha3";
/// Only this upstream Istio kind is compiled into RDS.
pub const VIRTUAL_SERVICE_KIND: &str = "VirtualService";
/// Upstream Nacos config tag that enables Istio config processing.
pub const VIRTUAL_SERVICE_TAG: &str = "virtual-service";
/// Upstream Nacos config tag that is validated but does not produce RDS.
pub const DESTINATION_RULE_TAG: &str = "destination-rule";
/// Suffix used by upstream Nacos for config-derived route configurations.
pub const ROUTE_CONFIG_SUFFIX: &str = "_route_config";

/// A minimal, crate-local view of a Nacos config entry.
///
/// Keeping this type in `batata-mesh` avoids a dependency on the persistence
/// crate while still carrying the upstream-relevant tag and content fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IstioConfigSourceEntry {
    /// Nacos `dataId` of the config, used only for diagnostics and ordering.
    pub data_id: String,
    /// Nacos `group` of the config, used only for diagnostics and ordering.
    pub group: String,
    /// Nacos namespace/tenant of the config.
    pub namespace: String,
    /// Comma-separated Nacos advanced `config_tags` value.
    pub config_tags: String,
    /// Raw config content. Both YAML and JSON are accepted.
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct IstioConfigEnvelope {
    #[serde(rename = "apiVersion")]
    api_version: String,
    kind: String,
    metadata: ConfigResourceMetadata,
    spec: ConfigVirtualServiceSpec,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ConfigVirtualServiceSpec {
    #[serde(default)]
    hosts: Vec<String>,
    #[serde(default)]
    http: Vec<ConfigHttpRoute>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ConfigHttpRoute {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, rename = "match")]
    match_requests: Vec<ConfigHttpMatchRequest>,
    #[serde(default)]
    route: Vec<ConfigHttpRouteDestination>,
    #[serde(default)]
    redirect: Option<JsonValue>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ConfigHttpMatchRequest {
    #[serde(default)]
    uri: Option<ConfigStringMatch>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ConfigStringMatch {
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    exact: Option<String>,
    #[serde(default)]
    regex: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ConfigHttpRouteDestination {
    #[serde(default)]
    destination: ConfigDestination,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ConfigDestination {
    #[serde(default)]
    host: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ConfigResourceMetadata {
    #[serde(default)]
    name: String,
    #[serde(default)]
    namespace: String,
    #[serde(default)]
    labels: HashMap<String, String>,
    #[serde(default)]
    annotations: HashMap<String, String>,
    #[serde(default, rename = "resourceVersion")]
    resource_version: String,
}

/// Return whether a Nacos `config_tags` value enables upstream Istio processing.
///
/// Upstream strips every hyphen and compares case-insensitively, so
/// `virtual-service`, `VirtualService` and `virtualservice` are equivalent.
pub fn is_istio_config_tag(config_tags: &str) -> bool {
    config_tags.split(',').any(|tag| {
        let normalized: String = tag
            .trim()
            .chars()
            .filter(|c| *c != '-')
            .flat_map(char::to_lowercase)
            .collect();
        normalized == "virtualservice" || normalized == "destinationrule"
    })
}

/// Parse a tagged Nacos config into an Istio `VirtualService`.
///
/// The envelope check mirrors upstream `IstioConfigProcessor.isContentValid`:
/// non-blank YAML/JSON with the exact `networking.istio.io/v1alpha3`
/// apiVersion, `VirtualService` kind, metadata and spec.
pub fn parse_virtual_service_config(content: &str) -> Result<VirtualService, String> {
    if content.trim().is_empty() {
        return Err("empty Istio config content".to_string());
    }

    let envelope: IstioConfigEnvelope = yaml_serde::from_str(content)
        .map_err(|error| format!("invalid Istio config YAML: {error}"))?;

    if envelope.api_version != VIRTUAL_SERVICE_API_VERSION {
        return Err(format!(
            "unsupported Istio apiVersion: {}",
            envelope.api_version
        ));
    }
    if envelope.kind != VIRTUAL_SERVICE_KIND {
        return Err(format!("unsupported Istio kind: {}", envelope.kind));
    }
    if envelope.metadata.name.trim().is_empty() {
        return Err("Istio VirtualService metadata.name is required".to_string());
    }

    Ok(VirtualService {
        metadata: ResourceMetadata {
            name: envelope.metadata.name,
            namespace: envelope.metadata.namespace,
            labels: envelope.metadata.labels,
            annotations: envelope.metadata.annotations,
            resource_version: envelope.metadata.resource_version,
        },
        spec: VirtualServiceSpec {
            hosts: envelope.spec.hosts,
            http: envelope
                .spec
                .http
                .into_iter()
                .map(config_http_route_to_dto)
                .collect(),
            ..Default::default()
        },
    })
}

fn config_http_route_to_dto(config: ConfigHttpRoute) -> HttpRoute {
    HttpRoute {
        name: config.name,
        r#match: config
            .match_requests
            .into_iter()
            .filter_map(|matcher| {
                matcher.uri.and_then(|uri| {
                    let uri = if let Some(value) = uri.prefix {
                        StringMatch::Prefix(value)
                    } else if let Some(value) = uri.exact {
                        StringMatch::Exact(value)
                    } else {
                        uri.regex.map(StringMatch::Regex)?
                    };
                    Some(HttpMatchRequest {
                        uri: Some(uri),
                        ..Default::default()
                    })
                })
            })
            .collect(),
        route: config
            .route
            .into_iter()
            .map(|destination| HttpRouteDestination {
                destination: Destination {
                    host: destination.destination.host,
                    ..Default::default()
                },
                ..Default::default()
            })
            .collect(),
        // The DTO has no dedicated "redirect present" flag; a default value is
        // enough for the converter to skip routes the minimal RDS proto cannot
        // encode.
        redirect: config.redirect.map(|_| Default::default()),
        ..Default::default()
    }
}

/// Compile one `VirtualService` into an Envoy `RouteConfiguration`.
///
/// Returns `None` when the config cannot be represented by the checked-in xDS
/// protobuf subset (for example a regex-only match or redirect route). Callers
/// must skip such resources rather than guessing a wire encoding.
pub fn virtual_service_to_route_config(
    virtual_service: &VirtualService,
    services: &[NacosService],
) -> Option<RouteConfiguration> {
    let name = virtual_service.metadata.name.trim();
    if name.is_empty() {
        return None;
    }

    let domains = virtual_service_domains(&virtual_service.spec.hosts, services);
    if domains.is_empty() {
        return None;
    }

    let routes: Vec<Route> = virtual_service
        .spec
        .http
        .iter()
        .enumerate()
        .filter_map(|(index, http)| http_route_to_route(http, name, index, services))
        .collect();
    if routes.is_empty() {
        return None;
    }

    let mut virtual_host = VirtualHost::new(name.to_string(), domains);
    virtual_host.routes = routes;
    Some(
        RouteConfiguration::new(format!("{name}{ROUTE_CONFIG_SUFFIX}"))
            .with_virtual_host(virtual_host),
    )
}

/// Build the complete 18848 snapshot: naming-derived resources plus config RDS.
///
/// Config-derived routes are keyed by their `RouteConfiguration.name`, so they
/// deterministically override a naming-derived route with the same name.
pub fn build_istio_mcp_snapshot(
    services: &[NacosService],
    configs: &[IstioConfigSourceEntry],
    sync_config: &SyncBridgeConfig,
) -> ResourceSnapshot {
    let mut snapshot = build_resource_snapshot(services, sync_config);

    let mut sorted_services = services.to_vec();
    sorted_services.sort_by(|a, b| a.xds_cluster_name().cmp(&b.xds_cluster_name()));

    let mut tagged: Vec<&IstioConfigSourceEntry> = configs
        .iter()
        .filter(|entry| is_istio_config_tag(&entry.config_tags))
        .collect();
    tagged.sort_by(|a, b| {
        (&a.namespace, &a.group, &a.data_id).cmp(&(&b.namespace, &b.group, &b.data_id))
    });

    let mut config_routes = Vec::new();
    for entry in tagged {
        match parse_virtual_service_config(&entry.content) {
            Ok(virtual_service) => {
                if let Some(route) =
                    virtual_service_to_route_config(&virtual_service, &sorted_services)
                {
                    config_routes.push(route);
                }
            }
            Err(error) => {
                warn!(
                    namespace = %entry.namespace,
                    group = %entry.group,
                    data_id = %entry.data_id,
                    error = %error,
                    "Skipping invalid Istio VirtualService config"
                );
            }
        }
    }

    for route in &config_routes {
        snapshot.routes.insert(route.name.clone(), route.clone());
    }
    merge_config_routes_into_default(&mut snapshot, &config_routes);

    snapshot
}

fn merge_config_routes_into_default(
    snapshot: &mut ResourceSnapshot,
    config_routes: &[RouteConfiguration],
) {
    let Some(default) = snapshot.routes.get_mut("default-routes") else {
        return;
    };

    let insert_at = default
        .virtual_hosts
        .iter()
        .position(|host| host.name == "catch-all")
        .unwrap_or(default.virtual_hosts.len());

    for route in config_routes {
        for host in &route.virtual_hosts {
            if host.domains.is_empty()
                || default
                    .virtual_hosts
                    .iter()
                    .any(|existing| existing.domains.iter().any(|d| host.domains.contains(d)))
            {
                continue;
            }
            default.virtual_hosts.insert(insert_at, host.clone());
        }
    }
}

fn virtual_service_domains(hosts: &[String], _services: &[NacosService]) -> Vec<String> {
    let mut domains = Vec::new();

    for host in hosts {
        if host == "*" {
            push_unique(&mut domains, "*");
        } else {
            // Upstream Nacos uses the declared host plus its domain suffix as
            // the virtual-host domain. Keeping only these suffixed domains also
            // avoids collisions with naming-derived virtual hosts in the
            // shared `default-routes` configuration.
            push_unique(&mut domains, &format!("{host}.nacos"));
        }
    }

    domains
}

fn http_route_to_route(
    http: &HttpRoute,
    virtual_service_name: &str,
    index: usize,
    services: &[NacosService],
) -> Option<Route> {
    if http.redirect.is_some() {
        // RedirectAction is not in the checked-in route protobuf subset.
        return None;
    }

    let match_config = http_route_match(http)?;
    let destination = http.route.first()?;
    let host = destination.destination.host.trim();
    let cluster = resolve_cluster(host, services)?;
    let name = http
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{virtual_service_name}-http-{index}"));

    Some(Route::new(
        name,
        match_config,
        RouteAction::Route(RouteDestination::cluster(cluster)),
    ))
}

fn http_route_match(http: &HttpRoute) -> Option<RouteMatch> {
    let mut route_match = RouteMatch::prefix("/");

    // Upstream calls setMatch in a loop; the last URI match wins.
    for matcher in &http.r#match {
        let Some(uri) = matcher.uri.as_ref() else {
            continue;
        };
        route_match = match uri {
            StringMatch::Prefix(value) => RouteMatch::prefix(value),
            StringMatch::Exact(value) => RouteMatch::path(value),
            // RegexMatcher is deliberately absent from the minimal RDS proto.
            StringMatch::Regex(_) => return None,
        };
    }

    Some(route_match)
}

fn resolve_cluster(host: &str, services: &[NacosService]) -> Option<String> {
    services
        .iter()
        .find(|service| {
            let full_name = service.full_name();
            service.service_name == host
                || full_name == host
                || full_name.contains(host)
                || host.contains(&service.service_name)
        })
        .map(NacosService::xds_cluster_name)
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion::{NacosInstance, NacosService};
    use crate::xds::types::PathMatch;

    fn service(name: &str) -> NacosService {
        NacosService {
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: name.to_string(),
            protect_threshold: 0.0,
            metadata: HashMap::new(),
            instances: vec![NacosInstance {
                instance_id: format!("{name}-1"),
                ip: "127.0.0.1".to_string(),
                port: 8080,
                weight: 1.0,
                healthy: true,
                enabled: true,
                ephemeral: true,
                cluster_name: String::new(),
                service_name: name.to_string(),
                metadata: HashMap::new(),
            }],
        }
    }

    fn virtual_service_yaml(prefix: &str) -> String {
        format!(
            concat!(
                "apiVersion: networking.istio.io/v1alpha3\n",
                "kind: VirtualService\n",
                "metadata:\n",
                "  name: web\n",
                "spec:\n",
                "  hosts:\n",
                "  - web\n",
                "  http:\n",
                "  - name: web-route\n",
                "    match:\n",
                "      - uri:\n",
                "          prefix: {prefix}\n",
                "    route:\n",
                "      - destination:\n",
                "          host: web\n",
            ),
            prefix = prefix
        )
    }

    #[test]
    fn config_tags_follow_upstream_normalization() {
        assert!(is_istio_config_tag("virtual-service"));
        assert!(is_istio_config_tag("other, VirtualService"));
        assert!(is_istio_config_tag("destination-rule"));
        assert!(!is_istio_config_tag("canary,gray"));
    }

    #[test]
    fn parses_upstream_virtual_service_envelope() {
        let parsed = parse_virtual_service_config(&virtual_service_yaml("/api"))
            .expect("valid VirtualService YAML");

        assert_eq!(parsed.metadata.name, "web");
        assert_eq!(parsed.spec.hosts, vec!["web"]);
        assert_eq!(parsed.spec.http.len(), 1);
        assert_eq!(parsed.spec.http[0].name.as_deref(), Some("web-route"));
    }

    #[test]
    fn rejects_wrong_api_version_and_kind() {
        let wrong_version = virtual_service_yaml("/").replace(
            "networking.istio.io/v1alpha3",
            "networking.istio.io/v1beta1",
        );
        assert!(parse_virtual_service_config(&wrong_version).is_err());

        let wrong_kind = virtual_service_yaml("/").replace("VirtualService", "DestinationRule");
        assert!(parse_virtual_service_config(&wrong_kind).is_err());
    }

    #[test]
    fn compiles_virtual_service_to_rds() {
        let parsed = parse_virtual_service_config(&virtual_service_yaml("/api"))
            .expect("valid VirtualService YAML");
        let route = virtual_service_to_route_config(&parsed, &[service("web")])
            .expect("RDS route configuration");

        assert_eq!(route.name, "web_route_config");
        assert_eq!(route.virtual_hosts.len(), 1);
        assert!(
            route.virtual_hosts[0]
                .domains
                .contains(&"web.nacos".to_string())
        );
        assert_eq!(route.virtual_hosts[0].routes.len(), 1);
        assert_eq!(route.virtual_hosts[0].routes[0].name, "web-route");
        assert!(matches!(
            route.virtual_hosts[0].routes[0].match_config.path_match,
            PathMatch::Prefix(ref value) if value == "/api"
        ));
        match &route.virtual_hosts[0].routes[0].action {
            RouteAction::Route(destination) => {
                assert_eq!(destination.cluster.as_deref(), Some("web"));
            }
            other => panic!("unexpected route action: {other:?}"),
        }
    }

    #[test]
    fn skips_unsupported_regex_route() {
        let content = virtual_service_yaml("/").replace("prefix: /", "regex: /api/.*");
        let parsed = parse_virtual_service_config(&content).expect("valid envelope");

        assert!(virtual_service_to_route_config(&parsed, &[service("web")]).is_none());
    }

    #[test]
    fn snapshot_merges_naming_and_config_routes() {
        let services = vec![service("web")];
        let configs = vec![IstioConfigSourceEntry {
            data_id: "web.yaml".to_string(),
            group: "DEFAULT".to_string(),
            namespace: "public".to_string(),
            config_tags: "virtual-service".to_string(),
            content: virtual_service_yaml("/api"),
        }];

        let snapshot = build_istio_mcp_snapshot(&services, &configs, &SyncBridgeConfig::default());

        assert_eq!(snapshot.clusters.len(), 1);
        assert!(snapshot.routes.contains_key("default-routes"));
        assert!(snapshot.routes.contains_key("web_route_config"));
        let default = &snapshot.routes["default-routes"];
        assert!(
            default
                .virtual_hosts
                .iter()
                .any(|host| host.name == "web" && host.domains == vec!["web.nacos"])
        );
        assert_eq!(
            default.virtual_hosts.last().expect("catch-all").name,
            "catch-all"
        );
    }

    #[test]
    fn untagged_config_is_ignored() {
        let mut configs = vec![IstioConfigSourceEntry {
            data_id: "web.yaml".to_string(),
            group: "DEFAULT".to_string(),
            namespace: "public".to_string(),
            config_tags: "canary".to_string(),
            content: virtual_service_yaml("/api"),
        }];
        configs[0].config_tags = String::new();

        let snapshot =
            build_istio_mcp_snapshot(&[service("web")], &configs, &SyncBridgeConfig::default());

        assert!(!snapshot.routes.contains_key("web_route_config"));
    }
}
