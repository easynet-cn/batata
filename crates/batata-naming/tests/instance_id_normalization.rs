//! Instance ID construction and gRPC-path normalisation.
//!
//! Anchored on upstream Nacos:
//! * `DefaultInstanceIdGenerator` — `ip#port#clusterName#group@@serviceName`
//!   (4 segments, `NAMING_INSTANCE_ID_SEG_COUNT = 4`).
//! * `InstanceUtil.setInstanceIdIfEmpty` — the gRPC path fills only the
//!   cluster default and the blank instance ID; namespace / group / service
//!   name are left exactly as the SDK sent them.
//! * `Constants.CUSTOM_INSTANCE_ID` — a client-supplied ID must survive
//!   verbatim, because clients reconcile instances by it.

use batata_api::model::{
    NAMING_INSTANCE_ID_SEG_COUNT, NAMING_INSTANCE_ID_SPLITTER, SERVICE_INFO_SPLITER,
};
use batata_api::naming::model::{
    Instance, InstanceParamError, MAX_PORT, default_instance_id, generate_instance_id,
    grouped_service_name, normalize_instance, validate_instance,
};

const NS_G: &str = "DEFAULT_GROUP";

/// Stand-in for what the gRPC handler receives: the SDK sends ip/port and very
/// often leaves `cluster_name` and `instance_id` blank.
fn grpc_instance() -> Instance {
    Instance {
        ip: "10.0.0.1".to_string(),
        port: 8080,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        ..Default::default()
    }
}

#[test]
fn grouped_service_name_joins_with_double_at() {
    assert_eq!(grouped_service_name("MY_GROUP", "svc"), "MY_GROUP@@svc");
}

/// An empty group falls back to `DEFAULT_GROUP`, matching the HTTP layer's
/// `group_name_or_default()` so both channels agree.
#[test]
fn grouped_service_name_defaults_empty_group() {
    assert_eq!(grouped_service_name("", "svc"), "DEFAULT_GROUP@@svc");
}

/// SDKs routinely send `group@@name` as the service name. Re-grouping that
/// would produce `DEFAULT_GROUP@@group@@name`, so it must pass through.
#[test]
fn already_grouped_service_name_is_not_regrouped() {
    assert_eq!(
        grouped_service_name("OTHER_GROUP", "MY_GROUP@@svc"),
        "MY_GROUP@@svc"
    );
}

#[test]
fn instance_id_has_four_segments_with_grouped_service() {
    let id = default_instance_id("10.0.0.1", 8080, "DEFAULT", "MY_GROUP", "svc");
    assert_eq!(id, "10.0.0.1#8080#DEFAULT#MY_GROUP@@svc");

    let segments: Vec<&str> = id.split(NAMING_INSTANCE_ID_SPLITTER).collect();
    assert_eq!(segments.len(), NAMING_INSTANCE_ID_SEG_COUNT as usize);
    // The 4th segment splits back into group and service name.
    let tail: Vec<&str> = segments[3].split(SERVICE_INFO_SPLITER).collect();
    assert_eq!(tail, vec!["MY_GROUP", "svc"]);
}

#[test]
fn instance_id_defaults_empty_cluster() {
    assert_eq!(
        default_instance_id("10.0.0.1", 8080, "", NS_G, "svc"),
        "10.0.0.1#8080#DEFAULT#DEFAULT_GROUP@@svc"
    );
}

/// `generate_instance_id` is the low-level form: the 4th segment is used
/// verbatim, so callers are expected to hand it a grouped name.
#[test]
fn generate_instance_id_uses_the_service_name_verbatim() {
    assert_eq!(
        generate_instance_id("10.0.0.1", 8080, "DEFAULT", "svc"),
        "10.0.0.1#8080#DEFAULT#svc"
    );
}

// ---------------------------------------------------------------------------
// gRPC-path normalisation
// ---------------------------------------------------------------------------

#[test]
fn normalize_fills_cluster_and_instance_id() {
    let mut instance = grpc_instance();
    normalize_instance(NS_G, "svc", &mut instance);

    assert_eq!(instance.cluster_name, "DEFAULT");
    assert_eq!(
        instance.instance_id,
        "10.0.0.1#8080#DEFAULT#DEFAULT_GROUP@@svc"
    );
    // The service name is echoed back grouped, like upstream's
    // `setInstanceIdIfEmpty(instance, service.getGroupedServiceName())`.
    assert_eq!(instance.service_name, "DEFAULT_GROUP@@svc");
}

#[test]
fn normalize_keeps_an_explicit_cluster() {
    let mut instance = grpc_instance();
    instance.cluster_name = "C1".to_string();
    normalize_instance(NS_G, "svc", &mut instance);

    assert_eq!(instance.cluster_name, "C1");
    assert_eq!(instance.instance_id, "10.0.0.1#8080#C1#DEFAULT_GROUP@@svc");
}

/// The regression guard: `Constants.CUSTOM_INSTANCE_ID` means a client-chosen
/// ID is authoritative. Regenerating it would silently re-key every instance
/// the client is tracking.
#[test]
fn normalize_preserves_a_client_supplied_instance_id() {
    let mut instance = grpc_instance();
    instance.instance_id = "my-own-id".to_string();
    normalize_instance(NS_G, "svc", &mut instance);

    assert_eq!(instance.instance_id, "my-own-id");
    // Everything else is still normalised.
    assert_eq!(instance.cluster_name, "DEFAULT");
}

/// Namespace, group and service name are NOT defaulted on the gRPC path —
/// upstream `InstanceRequestHandler` passes whatever the SDK sent.
#[test]
fn normalize_leaves_an_explicit_service_name_alone() {
    let mut instance = grpc_instance();
    instance.service_name = "explicit".to_string();
    normalize_instance("MY_GROUP", "svc", &mut instance);

    assert_eq!(instance.service_name, "explicit");
    // The ID still carries the request's service, which is authoritative.
    assert_eq!(instance.instance_id, "10.0.0.1#8080#DEFAULT#MY_GROUP@@svc");
}

#[test]
fn normalize_is_idempotent() {
    let mut instance = grpc_instance();
    normalize_instance(NS_G, "svc", &mut instance);
    let once = instance.clone();

    normalize_instance(NS_G, "svc", &mut instance);
    assert_eq!(instance.instance_id, once.instance_id);
    assert_eq!(instance.cluster_name, once.cluster_name);
    assert_eq!(instance.service_name, once.service_name);
}

/// Two instances that differ only by cluster must not collide: the cluster is
/// part of the ID (and of the internal storage key).
#[test]
fn instance_ids_disambiguate_cluster_and_group() {
    let a = default_instance_id("10.0.0.1", 8080, "C1", NS_G, "svc");
    let b = default_instance_id("10.0.0.1", 8080, "C2", NS_G, "svc");
    assert_ne!(a, b);

    let c = default_instance_id("10.0.0.1", 8080, "C1", "OTHER_GROUP", "svc");
    assert_ne!(a, c);
}

// ---------------------------------------------------------------------------
// Parameter validation (`Instance#validate`)
// ---------------------------------------------------------------------------

#[test]
fn blank_ip_is_rejected() {
    let instance = grpc_instance();
    assert_eq!(
        validate_instance(&instance),
        Ok(()),
        "a normal instance passes"
    );

    let mut blank = grpc_instance();
    blank.ip = "".to_string();
    assert_eq!(
        validate_instance(&blank),
        Err(InstanceParamError::MissingIp)
    );

    // Upstream uses `StringUtils.isBlank`, so whitespace counts as blank.
    let mut spaces = grpc_instance();
    spaces.ip = "   ".to_string();
    assert_eq!(
        validate_instance(&spaces),
        Err(InstanceParamError::MissingIp)
    );
}

/// Upstream accepts `port == 0` (`port < 0 || port > MAX_PORT`); only the HTTP
/// layer is stricter. The gRPC path must not invent extra rules.
#[test]
fn port_zero_is_accepted_but_out_of_range_is_not() {
    for port in [0, MAX_PORT] {
        let mut instance = grpc_instance();
        instance.port = port;
        assert_eq!(validate_instance(&instance), Ok(()), "port {port} is legal");
    }
    for port in [-1, MAX_PORT + 1] {
        let mut instance = grpc_instance();
        instance.port = port;
        assert_eq!(
            validate_instance(&instance),
            Err(InstanceParamError::InvalidPort(port))
        );
    }
}

#[test]
fn param_errors_carry_the_upstream_message() {
    assert_eq!(
        InstanceParamError::MissingIp.message(),
        "Required parameter 'ip' type String is not present"
    );
    assert_eq!(
        InstanceParamError::InvalidPort(-1).message(),
        "Required parameter 'port' type int is require 0 ~ 65535"
    );
}

/// Normalisation must not mask a rejected instance: a blank ip still fails
/// validation after the cluster default has been filled in.
#[test]
fn normalization_does_not_rescue_an_invalid_instance() {
    let mut instance = grpc_instance();
    instance.ip = "".to_string();
    normalize_instance(NS_G, "svc", &mut instance);

    assert_eq!(instance.cluster_name, "DEFAULT");
    assert_eq!(
        validate_instance(&instance),
        Err(InstanceParamError::MissingIp)
    );
}

/// The internal storage key stays 3 segments and service-scoped; the
/// client-facing ID is the 4-segment one. Both exist on purpose.
#[test]
fn internal_key_and_client_id_are_distinct() {
    let mut instance = grpc_instance();
    normalize_instance(NS_G, "svc", &mut instance);

    assert_eq!(instance.key(), "10.0.0.1#8080#DEFAULT");
    assert_eq!(
        instance.instance_id,
        "10.0.0.1#8080#DEFAULT#DEFAULT_GROUP@@svc"
    );
}
