//! Admin API path constants following Nacos `AdminApiPath`.

pub mod admin_api_path {
    // Namespace
    //! Admin API path constants.
    /// The `NAMESPACE_LIST` constant.
    pub const NAMESPACE_LIST: &str = "/v3/admin/core/namespace/list";
    /// The `NAMESPACE` constant.
    pub const NAMESPACE: &str = "/v3/admin/core/namespace";
    /// The `NAMESPACE_EXIST` constant.
    pub const NAMESPACE_EXIST: &str = "/v3/admin/core/namespace/exist";

    // Config
    /// The `CONFIG` constant.
    pub const CONFIG: &str = "/v3/admin/cs/config";
    /// The `CONFIG_LIST` constant.
    pub const CONFIG_LIST: &str = "/v3/admin/cs/config/list";
    /// The `CONFIG_BETA` constant.
    pub const CONFIG_BETA: &str = "/v3/admin/cs/config/beta";
    /// The `CONFIG_EXPORT` constant.
    pub const CONFIG_EXPORT: &str = "/v3/admin/cs/config/export";
    /// The `CONFIG_IMPORT` constant.
    pub const CONFIG_IMPORT: &str = "/v3/admin/cs/config/import";
    /// The `CONFIG_CLONE` constant.
    pub const CONFIG_CLONE: &str = "/v3/admin/cs/config/clone";
    /// The `CONFIG_BATCH_DELETE` constant.
    pub const CONFIG_BATCH_DELETE: &str = "/v3/admin/cs/config";
    /// The `CONFIG_SEARCH` constant.
    pub const CONFIG_SEARCH: &str = "/v3/admin/cs/config/searchDetail";
    /// The `CONFIG_METADATA` constant.
    pub const CONFIG_METADATA: &str = "/v3/admin/cs/config/metadata";

    // Config Beta
    /// The `CONFIG_BETA_PUBLISH` constant.
    pub const CONFIG_BETA_PUBLISH: &str = "/v3/admin/cs/config/beta";
    /// The `CONFIG_BETA_STOP` constant.
    pub const CONFIG_BETA_STOP: &str = "/v3/admin/cs/config/beta";

    // Config Ops
    /// The `CONFIG_OPS` constant.
    pub const CONFIG_OPS: &str = "/v3/admin/cs/ops";
    /// The `CONFIG_OPS_LOG` constant.
    pub const CONFIG_OPS_LOG: &str = "/v3/admin/cs/ops/log";
    /// The `CONFIG_OPS_DERBY` constant.
    pub const CONFIG_OPS_DERBY: &str = "/v3/admin/cs/ops/derby";
    /// The `CONFIG_OPS_LOCAL_CACHE` constant.
    pub const CONFIG_OPS_LOCAL_CACHE: &str = "/v3/admin/cs/ops/localCache";

    // History
    /// The `CONFIG_HISTORY` constant.
    pub const CONFIG_HISTORY: &str = "/v3/admin/cs/history";
    /// The `CONFIG_HISTORY_LIST` constant.
    pub const CONFIG_HISTORY_LIST: &str = "/v3/admin/cs/history/list";
    /// The `CONFIG_HISTORY_CONFIGS` constant.
    pub const CONFIG_HISTORY_CONFIGS: &str = "/v3/admin/cs/history/configs";
    /// The `CONFIG_HISTORY_PREVIOUS` constant.
    pub const CONFIG_HISTORY_PREVIOUS: &str = "/v3/admin/cs/history/previous";

    // Listener
    /// The `CONFIG_LISTENER` constant.
    pub const CONFIG_LISTENER: &str = "/v3/admin/cs/listener";
    /// The `CONFIG_LISTENER_IP` constant.
    pub const CONFIG_LISTENER_IP: &str = "/v3/admin/cs/listener";

    // Cluster
    /// The `CLUSTER_NODE_LIST` constant.
    pub const CLUSTER_NODE_LIST: &str = "/v3/admin/core/cluster/node/list";
    /// The `CLUSTER_SELF_HEALTH` constant.
    pub const CLUSTER_SELF_HEALTH: &str = "/v3/admin/core/cluster/node/self/health";
    /// The `CLUSTER_SELF` constant.
    pub const CLUSTER_SELF: &str = "/v3/admin/core/cluster/node/self";
    /// The `CLUSTER_LOOKUP` constant.
    pub const CLUSTER_LOOKUP: &str = "/v3/admin/core/cluster/lookup";

    // Server state
    /// The `SERVER_STATE` constant.
    pub const SERVER_STATE: &str = "/v3/admin/core/state";
    /// The `SERVER_LIVENESS` constant.
    pub const SERVER_LIVENESS: &str = "/v3/admin/core/state/liveness";
    /// The `SERVER_READINESS` constant.
    pub const SERVER_READINESS: &str = "/v3/admin/core/state/readiness";

    // Core ops
    /// The `CORE_OPS` constant.
    pub const CORE_OPS: &str = "/v3/admin/core/ops";
    /// The `CORE_OPS_RAFT` constant.
    pub const CORE_OPS_RAFT: &str = "/v3/admin/core/ops/raft";
    /// The `CORE_OPS_ID_GENERATOR` constant.
    pub const CORE_OPS_ID_GENERATOR: &str = "/v3/admin/core/ops/ids";
    /// The `CORE_OPS_LOG` constant.
    pub const CORE_OPS_LOG: &str = "/v3/admin/core/ops/log";

    // Core loader
    /// The `CORE_LOADER` constant.
    pub const CORE_LOADER: &str = "/v3/admin/core/loader";
    /// The `CORE_LOADER_CURRENT` constant.
    pub const CORE_LOADER_CURRENT: &str = "/v3/admin/core/loader/current";
    /// The `CORE_LOADER_METRICS` constant.
    pub const CORE_LOADER_METRICS: &str = "/v3/admin/core/loader/cluster";
    /// The `CORE_LOADER_RELOAD` constant.
    pub const CORE_LOADER_RELOAD: &str = "/v3/admin/core/loader/reloadCurrent";
    /// The `CORE_LOADER_SMART_RELOAD` constant.
    pub const CORE_LOADER_SMART_RELOAD: &str = "/v3/admin/core/loader/smartReloadCluster";
    /// The `CORE_LOADER_RELOAD_CLIENT` constant.
    pub const CORE_LOADER_RELOAD_CLIENT: &str = "/v3/admin/core/loader/reloadClient";

    // Service
    /// The `SERVICE` constant.
    pub const SERVICE: &str = "/v3/admin/ns/service";
    /// The `SERVICE_LIST` constant.
    pub const SERVICE_LIST: &str = "/v3/admin/ns/service/list";
    /// The `SERVICE_DETAIL` constant.
    pub const SERVICE_DETAIL: &str = "/v3/admin/ns/service";
    /// The `SERVICE_LIST_DETAIL` constant.
    pub const SERVICE_LIST_DETAIL: &str = "/v3/admin/ns/service/list/withDetail";
    /// The `SERVICE_SELECTOR_TYPES` constant.
    pub const SERVICE_SELECTOR_TYPES: &str = "/v3/admin/ns/service/selector/types";

    // Instance
    /// The `INSTANCE` constant.
    pub const INSTANCE: &str = "/v3/admin/ns/instance";
    /// The `INSTANCE_LIST` constant.
    pub const INSTANCE_LIST: &str = "/v3/admin/ns/instance/list";
    /// The `INSTANCE_DETAIL` constant.
    pub const INSTANCE_DETAIL: &str = "/v3/admin/ns/instance";
    /// The `INSTANCE_METADATA_BATCH` constant.
    pub const INSTANCE_METADATA_BATCH: &str = "/v3/admin/ns/instance/metadata/batch";

    // Naming Cluster
    /// The `NAMING_CLUSTER` constant.
    pub const NAMING_CLUSTER: &str = "/v3/admin/ns/cluster";

    // Naming Health
    /// The `NAMING_HEALTH` constant.
    pub const NAMING_HEALTH: &str = "/v3/admin/ns/health";
    /// The `NAMING_HEALTH_CHECKERS` constant.
    pub const NAMING_HEALTH_CHECKERS: &str = "/v3/admin/ns/health/checkers";
    /// The `NAMING_HEALTH_INSTANCE` constant.
    pub const NAMING_HEALTH_INSTANCE: &str = "/v3/admin/ns/health/instance";

    // Naming Client
    /// The `NAMING_CLIENT_LIST` constant.
    pub const NAMING_CLIENT_LIST: &str = "/v3/admin/ns/client/list";
    /// The `NAMING_CLIENT` constant.
    pub const NAMING_CLIENT: &str = "/v3/admin/ns/client";
    /// The `NAMING_CLIENT_PUBLISH` constant.
    pub const NAMING_CLIENT_PUBLISH: &str = "/v3/admin/ns/client/publish/list";
    /// The `NAMING_CLIENT_SUBSCRIBE` constant.
    pub const NAMING_CLIENT_SUBSCRIBE: &str = "/v3/admin/ns/client/subscribe/list";
    /// The `NAMING_CLIENT_SERVICE_PUBLISH` constant.
    pub const NAMING_CLIENT_SERVICE_PUBLISH: &str = "/v3/admin/ns/client/service/publisher/list";
    /// The `NAMING_CLIENT_SERVICE_SUBSCRIBE` constant.
    pub const NAMING_CLIENT_SERVICE_SUBSCRIBE: &str = "/v3/admin/ns/client/service/subscriber/list";

    // Subscriber
    /// The `SUBSCRIBER_LIST` constant.
    pub const SUBSCRIBER_LIST: &str = "/v3/admin/ns/client/subscribe/list";

    // Naming Ops
    /// The `NAMING_OPS` constant.
    pub const NAMING_OPS: &str = "/v3/admin/ns/ops";
    /// The `NAMING_OPS_LOG` constant.
    pub const NAMING_OPS_LOG: &str = "/v3/admin/ns/ops/log";
    /// The `NAMING_OPS_METRICS` constant.
    pub const NAMING_OPS_METRICS: &str = "/v3/admin/ns/ops/metrics";

    // AI MCP
    /// The `AI_MCP` constant.
    pub const AI_MCP: &str = "/v3/admin/ai/mcp";
    /// The `AI_MCP_LIST` constant.
    pub const AI_MCP_LIST: &str = "/v3/admin/ai/mcp/list";

    // AI Agent (A2A)
    /// The `AI_AGENT` constant.
    pub const AI_AGENT: &str = "/v3/admin/ai/a2a";
    /// The `AI_AGENT_VERSION_LIST` constant.
    pub const AI_AGENT_VERSION_LIST: &str = "/v3/admin/ai/a2a/version/list";
    /// The `AI_AGENT_LIST` constant.
    pub const AI_AGENT_LIST: &str = "/v3/admin/ai/a2a/list";

    // Plugin Management
    /// The `PLUGIN_LIST` constant.
    pub const PLUGIN_LIST: &str = "/v3/admin/core/plugin/list";
    /// The `PLUGIN_DETAIL` constant.
    pub const PLUGIN_DETAIL: &str = "/v3/admin/core/plugin/detail";
    /// The `PLUGIN_STATUS` constant.
    pub const PLUGIN_STATUS: &str = "/v3/admin/core/plugin/status";
    /// The `PLUGIN_CONFIG` constant.
    pub const PLUGIN_CONFIG: &str = "/v3/admin/core/plugin/config";
    /// The `PLUGIN_AVAILABILITY` constant.
    pub const PLUGIN_AVAILABILITY: &str = "/v3/admin/core/plugin/availability";

    // Auth
    /// The `AUTH_LOGIN` constant.
    pub const AUTH_LOGIN: &str = "/v3/auth/user/login";
}
