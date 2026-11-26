//! Contract test for the public API-model façade.
//!
//! `crates/batata-server/src/api/config/model.rs` is only a re-export chain:
//! server → batata-config → batata-server-common. External clients see that
//! façade, so the upstream `ConfigBasicInfo` wire shape must be locked here,
//! not only inside the crate that happens to own the struct.

use batata_server::api::config::model::{ConfigBasicInfo, ConfigDetailInfo};

fn basic_info() -> ConfigBasicInfo {
    ConfigBasicInfo {
        id: 42,
        namespace_id: "public".into(),
        group_name: "DEFAULT_GROUP".into(),
        data_id: "app.yaml".into(),
        md5: "md5".into(),
        r#type: "yaml".into(),
        app_name: "app".into(),
        create_time: 1,
        modify_time: 2,
        desc: "hello".into(),
        config_tags: "tag1,tag2".into(),
    }
}

#[test]
fn api_facade_config_basic_info_uses_upstream_wire_shape() {
    let json = serde_json::to_string(&basic_info()).expect("ConfigBasicInfo must serialize");

    // Upstream @JsonFormat(shape = STRING): a numeric id is not acceptable on
    // the wire even though deserialization still accepts it for old clients.
    assert!(json.contains(r#""id":"42""#), "{json}");
    assert!(!json.contains(r#""id":42"#), "{json}");
    assert!(json.contains(r#""desc":"hello""#), "{json}");
    assert!(json.contains(r#""configTags":"tag1,tag2""#), "{json}");
    // All eleven upstream fields are present on the façade type.
    for field in [
        "id",
        "namespaceId",
        "groupName",
        "dataId",
        "md5",
        "type",
        "appName",
        "createTime",
        "modifyTime",
        "desc",
        "configTags",
    ] {
        assert!(
            json.contains(&format!("\"{field}\"")),
            "missing upstream field {field}: {json}"
        );
    }

    let upstream = r#"{"id":"42","namespaceId":"public","groupName":"DEFAULT_GROUP","dataId":"app.yaml","md5":"md5","type":"yaml","appName":"app","createTime":1,"modifyTime":2,"desc":"hello","configTags":"tag1,tag2"}"#;
    let decoded: ConfigBasicInfo =
        serde_json::from_str(upstream).expect("genuine upstream JSON must deserialize");
    assert_eq!(decoded.id, 42);
    assert_eq!(decoded.desc, "hello");
    assert_eq!(decoded.config_tags, "tag1,tag2");

    let legacy = upstream.replace(r#""id":"42""#, r#""id":42"#);
    let legacy: ConfigBasicInfo =
        serde_json::from_str(&legacy).expect("numeric id stays backward compatible");
    assert_eq!(legacy.id, 42);

    let metadata_less: ConfigBasicInfo = serde_json::from_str(
        r#"{"id":"42","namespaceId":"public","groupName":"G","dataId":"d","md5":"","type":"","appName":"","createTime":0,"modifyTime":0}"#,
    )
    .expect("upstream-added fields must default when absent");
    assert_eq!(metadata_less.desc, "");
    assert_eq!(metadata_less.config_tags, "");
}

#[test]
fn api_facade_config_detail_info_flattens_inherited_metadata_once() {
    let detail = ConfigDetailInfo {
        config_basic_info: basic_info(),
        content: "key: value".into(),
        encrypted_data_key: String::new(),
        create_user: "u".into(),
        create_ip: "127.0.0.1".into(),
    };
    let json = serde_json::to_string(&detail).expect("ConfigDetailInfo must serialize");

    // Upstream ConfigDetailInfo inherits these fields from ConfigBasicInfo.
    // The façade must emit one key each, not one from basic and one from detail.
    assert_eq!(json.matches("\"desc\"").count(), 1, "{json}");
    assert_eq!(json.matches("\"configTags\"").count(), 1, "{json}");
    assert!(json.contains(r#""id":"42""#), "{json}");

    let decoded: ConfigDetailInfo =
        serde_json::from_str(&json).expect("flattened façade JSON must round-trip");
    assert_eq!(decoded.config_basic_info.desc, "hello");
    assert_eq!(decoded.config_basic_info.config_tags, "tag1,tag2");
    assert_eq!(decoded.content, "key: value");
}
