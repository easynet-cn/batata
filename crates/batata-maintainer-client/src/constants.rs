// Admin API path constants following Nacos AdminApiPath

pub mod admin_api_path {
    // Namespace
    pub const NAMESPACE_LIST: &str = "/v3/admin/core/namespace/list";
    pub const NAMESPACE: &str = "/v3/admin/core/namespace";
    pub const NAMESPACE_EXIST: &str = "/v3/admin/core/namespace/exist";

    // Config
    pub const CONFIG: &str = "/v3/admin/cs/config";
    pub const CONFIG_LIST: &str = "/v3/admin/cs/config/list";
    pub const CONFIG_BETA: &str = "/v3/admin/cs/config/beta";
    pub const CONFIG_EXPORT: &str = "/v3/admin/cs/config/export";
    pub const CONFIG_IMPORT: &str = "/v3/admin/cs/config/import";
    pub const CONFIG_CLONE: &str = "/v3/admin/cs/config/clone";
    pub const CONFIG_BATCH_DELETE: &str = "/v3/admin/cs/config";
    pub const CONFIG_SEARCH: &str = "/v3/admin/cs/config/searchDetail";
    pub const CONFIG_METADATA: &str = "/v3/admin/cs/config/metadata";

    // Config Beta
    pub const CONFIG_BETA_PUBLISH: &str = "/v3/admin/cs/config/beta";
    pub const CONFIG_BETA_STOP: &str = "/v3/admin/cs/config/beta";

    // Config Ops
    pub const CONFIG_OPS: &str = "/v3/admin/cs/ops";
    pub const CONFIG_OPS_LOG: &str = "/v3/admin/cs/ops/log";
    pub const CONFIG_OPS_DERBY: &str = "/v3/admin/cs/ops/derby";
    pub const CONFIG_OPS_LOCAL_CACHE: &str = "/v3/admin/cs/ops/localCache";

    // History
    pub const CONFIG_HISTORY: &str = "/v3/admin/cs/history";
    pub const CONFIG_HISTORY_LIST: &str = "/v3/admin/cs/history/list";
    pub const CONFIG_HISTORY_CONFIGS: &str = "/v3/admin/cs/history/configs";
    pub const CONFIG_HISTORY_PREVIOUS: &str = "/v3/admin/cs/history/previous";

    // Listener
    pub const CONFIG_LISTENER: &str = "/v3/admin/cs/listener";
    pub const CONFIG_LISTENER_IP: &str = "/v3/admin/cs/listener";

    // Cluster
    pub const CLUSTER_NODE_LIST: &str = "/v3/admin/core/cluster/node/list";
    pub const CLUSTER_SELF_HEALTH: &str = "/v3/admin/core/cluster/node/self/health";
    pub const CLUSTER_SELF: &str = "/v3/admin/core/cluster/node/self";
    pub const CLUSTER_LOOKUP: &str = "/v3/admin/core/cluster/lookup";

    // Server state
    pub const SERVER_STATE: &str = "/v3/admin/core/state";
    pub const SERVER_LIVENESS: &str = "/v3/admin/core/state/liveness";
    pub const SERVER_READINESS: &str = "/v3/admin/core/state/readiness";

    // Core ops
    pub const CORE_OPS: &str = "/v3/admin/core/ops";
    pub const CORE_OPS_RAFT: &str = "/v3/admin/core/ops/raft";
    pub const CORE_OPS_ID_GENERATOR: &str = "/v3/admin/core/ops/ids";
    pub const CORE_OPS_LOG: &str = "/v3/admin/core/ops/log";

    // Core loader
    pub const CORE_LOADER: &str = "/v3/admin/core/loader";
    pub const CORE_LOADER_CURRENT: &str = "/v3/admin/core/loader/current";
    pub const CORE_LOADER_METRICS: &str = "/v3/admin/core/loader/cluster";
    pub const CORE_LOADER_RELOAD: &str = "/v3/admin/core/loader/reloadCurrent";
    pub const CORE_LOADER_SMART_RELOAD: &str = "/v3/admin/core/loader/smartReloadCluster";
    pub const CORE_LOADER_RELOAD_CLIENT: &str = "/v3/admin/core/loader/reloadClient";

    // Service
    pub const SERVICE: &str = "/v3/admin/ns/service";
    pub const SERVICE_LIST: &str = "/v3/admin/ns/service/list";
    pub const SERVICE_DETAIL: &str = "/v3/admin/ns/service";
    pub const SERVICE_LIST_DETAIL: &str = "/v3/admin/ns/service/list/withDetail";
    pub const SERVICE_SELECTOR_TYPES: &str = "/v3/admin/ns/service/selector/types";

    // Instance
    pub const INSTANCE: &str = "/v3/admin/ns/instance";
    pub const INSTANCE_LIST: &str = "/v3/admin/ns/instance/list";
    pub const INSTANCE_DETAIL: &str = "/v3/admin/ns/instance";
    pub const INSTANCE_METADATA_BATCH: &str = "/v3/admin/ns/instance/metadata/batch";

    // Naming Cluster
    pub const NAMING_CLUSTER: &str = "/v3/admin/ns/cluster";

    // Naming Health
    pub const NAMING_HEALTH: &str = "/v3/admin/ns/health";
    pub const NAMING_HEALTH_CHECKERS: &str = "/v3/admin/ns/health/checkers";
    pub const NAMING_HEALTH_INSTANCE: &str = "/v3/admin/ns/health/instance";

    // Naming Client
    pub const NAMING_CLIENT_LIST: &str = "/v3/admin/ns/client/list";
    pub const NAMING_CLIENT: &str = "/v3/admin/ns/client";
    pub const NAMING_CLIENT_PUBLISH: &str = "/v3/admin/ns/client/publish/list";
    pub const NAMING_CLIENT_SUBSCRIBE: &str = "/v3/admin/ns/client/subscribe/list";
    pub const NAMING_CLIENT_SERVICE_PUBLISH: &str = "/v3/admin/ns/client/service/publisher/list";
    pub const NAMING_CLIENT_SERVICE_SUBSCRIBE: &str = "/v3/admin/ns/client/service/subscriber/list";

    // Subscriber
    pub const SUBSCRIBER_LIST: &str = "/v3/admin/ns/client/subscribe/list";

    // Naming Ops
    pub const NAMING_OPS: &str = "/v3/admin/ns/ops";
    pub const NAMING_OPS_LOG: &str = "/v3/admin/ns/ops/log";
    pub const NAMING_OPS_METRICS: &str = "/v3/admin/ns/ops/metrics";

    // AI MCP
    pub const AI_MCP: &str = "/v3/admin/ai/mcp";
    pub const AI_MCP_LIST: &str = "/v3/admin/ai/mcp/list";

    // AI Agent (A2A)
    pub const AI_AGENT: &str = "/v3/admin/ai/a2a";
    pub const AI_AGENT_VERSION_LIST: &str = "/v3/admin/ai/a2a/version/list";
    pub const AI_AGENT_LIST: &str = "/v3/admin/ai/a2a/list";

    // Auth
    pub const AUTH_LOGIN: &str = "/v3/auth/user/login";
}
