/*
 * Copyright 1999-2018 Alibaba Group Holding Ltd.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

-- PostgreSQL Schema for Batata (Nacos-compatible)
-- This file contains the PostgreSQL-compatible schema definitions

/******************************************/
/*   Table: config_info                   */
/******************************************/
CREATE TABLE config_info (
    id BIGSERIAL PRIMARY KEY,
    data_id VARCHAR(255) NOT NULL,
    group_id VARCHAR(128) DEFAULT NULL,
    content TEXT NOT NULL,
    md5 VARCHAR(32) DEFAULT NULL,
    gmt_create TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    gmt_modified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    src_user TEXT,
    src_ip VARCHAR(50) DEFAULT NULL,
    app_name VARCHAR(128) DEFAULT NULL,
    tenant_id VARCHAR(128) DEFAULT '',
    c_desc VARCHAR(256) DEFAULT NULL,
    c_use VARCHAR(64) DEFAULT NULL,
    effect VARCHAR(64) DEFAULT NULL,
    type VARCHAR(64) DEFAULT NULL,
    c_schema TEXT,
    encrypted_data_key VARCHAR(1024) NOT NULL DEFAULT '',
    CONSTRAINT uk_configinfo_datagrouptenant UNIQUE (data_id, group_id, tenant_id)
);

COMMENT ON TABLE config_info IS 'Configuration information table';
COMMENT ON COLUMN config_info.id IS 'Primary key ID';
COMMENT ON COLUMN config_info.data_id IS 'Configuration data ID';
COMMENT ON COLUMN config_info.group_id IS 'Configuration group ID';
COMMENT ON COLUMN config_info.content IS 'Configuration content';
COMMENT ON COLUMN config_info.md5 IS 'MD5 hash of content';
COMMENT ON COLUMN config_info.tenant_id IS 'Tenant/Namespace ID';

/******************************************/
/*   Table: config_info_gray (since 2.5.0)*/
/******************************************/
CREATE TABLE config_info_gray (
    id BIGSERIAL PRIMARY KEY,
    data_id VARCHAR(255) NOT NULL,
    group_id VARCHAR(128) NOT NULL,
    content TEXT NOT NULL,
    md5 VARCHAR(32) DEFAULT NULL,
    src_user TEXT,
    src_ip VARCHAR(100) DEFAULT NULL,
    gmt_create TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    gmt_modified TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    app_name VARCHAR(128) DEFAULT NULL,
    tenant_id VARCHAR(128) DEFAULT '',
    gray_name VARCHAR(128) NOT NULL,
    gray_rule TEXT NOT NULL,
    encrypted_data_key VARCHAR(256) NOT NULL DEFAULT '',
    CONSTRAINT uk_configinfogray_datagrouptenantgray UNIQUE (data_id, group_id, tenant_id, gray_name)
);

CREATE INDEX idx_gray_dataid_gmt_modified ON config_info_gray(data_id, gmt_modified);
CREATE INDEX idx_gray_gmt_modified ON config_info_gray(gmt_modified);

COMMENT ON TABLE config_info_gray IS 'Gray release configuration table';

/******************************************/
/*   Table: config_tags_relation          */
/******************************************/
CREATE TABLE config_tags_relation (
    id BIGINT NOT NULL,
    tag_name VARCHAR(128) NOT NULL,
    tag_type VARCHAR(64) DEFAULT NULL,
    data_id VARCHAR(255) NOT NULL,
    group_id VARCHAR(128) NOT NULL,
    tenant_id VARCHAR(128) DEFAULT '',
    nid BIGSERIAL PRIMARY KEY,
    CONSTRAINT uk_configtagrelation_configidtag UNIQUE (id, tag_name, tag_type)
);

CREATE INDEX idx_tags_tenant_id ON config_tags_relation(tenant_id);

COMMENT ON TABLE config_tags_relation IS 'Configuration tag relation table';

/******************************************/
/*   Table: group_capacity                */
/******************************************/
CREATE TABLE group_capacity (
    id BIGSERIAL PRIMARY KEY,
    group_id VARCHAR(128) NOT NULL DEFAULT '',
    quota INTEGER NOT NULL DEFAULT 0,
    usage INTEGER NOT NULL DEFAULT 0,
    max_size INTEGER NOT NULL DEFAULT 0,
    max_aggr_count INTEGER NOT NULL DEFAULT 0,
    max_aggr_size INTEGER NOT NULL DEFAULT 0,
    max_history_count INTEGER NOT NULL DEFAULT 0,
    gmt_create TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    gmt_modified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_group_id UNIQUE (group_id)
);

COMMENT ON TABLE group_capacity IS 'Group capacity information table';

/******************************************/
/*   Table: his_config_info               */
/******************************************/
CREATE TABLE his_config_info (
    id BIGINT NOT NULL,
    nid BIGSERIAL PRIMARY KEY,
    data_id VARCHAR(255) NOT NULL,
    group_id VARCHAR(128) NOT NULL,
    app_name VARCHAR(128) DEFAULT NULL,
    content TEXT NOT NULL,
    md5 VARCHAR(32) DEFAULT NULL,
    gmt_create TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    gmt_modified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    src_user TEXT,
    src_ip VARCHAR(50) DEFAULT NULL,
    op_type CHAR(10) DEFAULT NULL,
    tenant_id VARCHAR(128) DEFAULT '',
    encrypted_data_key VARCHAR(1024) NOT NULL DEFAULT '',
    publish_type VARCHAR(50) DEFAULT 'formal',
    gray_name VARCHAR(50) DEFAULT NULL,
    ext_info TEXT DEFAULT NULL
);

CREATE INDEX idx_his_gmt_create ON his_config_info(gmt_create);
CREATE INDEX idx_his_gmt_modified ON his_config_info(gmt_modified);
CREATE INDEX idx_his_did ON his_config_info(data_id);

COMMENT ON TABLE his_config_info IS 'Configuration history table';

/******************************************/
/*   Table: tenant_capacity               */
/******************************************/
CREATE TABLE tenant_capacity (
    id BIGSERIAL PRIMARY KEY,
    tenant_id VARCHAR(128) NOT NULL DEFAULT '',
    quota INTEGER NOT NULL DEFAULT 0,
    usage INTEGER NOT NULL DEFAULT 0,
    max_size INTEGER NOT NULL DEFAULT 0,
    max_aggr_count INTEGER NOT NULL DEFAULT 0,
    max_aggr_size INTEGER NOT NULL DEFAULT 0,
    max_history_count INTEGER NOT NULL DEFAULT 0,
    gmt_create TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    gmt_modified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_tenant_id UNIQUE (tenant_id)
);

COMMENT ON TABLE tenant_capacity IS 'Tenant capacity information table';

/******************************************/
/*   Table: tenant_info                   */
/******************************************/
CREATE TABLE tenant_info (
    id BIGSERIAL PRIMARY KEY,
    kp VARCHAR(128) NOT NULL,
    tenant_id VARCHAR(128) DEFAULT '',
    tenant_name VARCHAR(128) DEFAULT '',
    tenant_desc VARCHAR(256) DEFAULT NULL,
    create_source VARCHAR(32) DEFAULT NULL,
    gmt_create BIGINT NOT NULL,
    gmt_modified BIGINT NOT NULL,
    CONSTRAINT uk_tenant_info_kptenantid UNIQUE (kp, tenant_id)
);

CREATE INDEX idx_tenant_id ON tenant_info(tenant_id);

COMMENT ON TABLE tenant_info IS 'Tenant/Namespace information table';

/******************************************/
/*   Table: users                         */
/******************************************/
CREATE TABLE users (
    username VARCHAR(50) NOT NULL PRIMARY KEY,
    password VARCHAR(500) NOT NULL,
    enabled BOOLEAN NOT NULL
);

COMMENT ON TABLE users IS 'User account table';

/******************************************/
/*   Table: roles                         */
/******************************************/
CREATE TABLE roles (
    username VARCHAR(50) NOT NULL,
    role VARCHAR(50) NOT NULL,
    CONSTRAINT uk_user_role UNIQUE (username, role)
);

COMMENT ON TABLE roles IS 'User role mapping table';

/******************************************/
/*   Table: permissions                   */
/******************************************/
CREATE TABLE permissions (
    role VARCHAR(50) NOT NULL,
    resource VARCHAR(128) NOT NULL,
    action VARCHAR(8) NOT NULL,
    CONSTRAINT uk_role_permission UNIQUE (role, resource, action)
);

COMMENT ON TABLE permissions IS 'Role permission table';

/******************************************/
/*   Consul ACL Tables                    */
/******************************************/

-- Consul ACL Tokens
CREATE TABLE consul_acl_tokens (
    id BIGSERIAL PRIMARY KEY,
    accessor_id VARCHAR(36) NOT NULL,
    secret_id VARCHAR(36) NOT NULL,
    description VARCHAR(256) DEFAULT '',
    policies TEXT,
    roles TEXT,
    local BOOLEAN NOT NULL DEFAULT FALSE,
    expiration_time TIMESTAMP DEFAULT NULL,
    create_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    modify_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_accessor_id UNIQUE (accessor_id),
    CONSTRAINT uk_secret_id UNIQUE (secret_id)
);

COMMENT ON TABLE consul_acl_tokens IS 'Consul ACL Tokens';

-- Consul ACL Policies
CREATE TABLE consul_acl_policies (
    id BIGSERIAL PRIMARY KEY,
    policy_id VARCHAR(36) NOT NULL,
    name VARCHAR(128) NOT NULL,
    description VARCHAR(256) DEFAULT '',
    rules TEXT NOT NULL,
    datacenters TEXT,
    create_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    modify_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_policy_id UNIQUE (policy_id),
    CONSTRAINT uk_policy_name UNIQUE (name)
);

COMMENT ON TABLE consul_acl_policies IS 'Consul ACL Policies';

/******************************************/
/*   Initial Data                         */
/******************************************/

-- Bootstrap token (management token)
INSERT INTO consul_acl_tokens (accessor_id, secret_id, description, policies, roles, local)
VALUES ('00000000-0000-0000-0000-000000000001', 'root', 'Bootstrap Token (Management)', '["global-management"]', '[]', FALSE);

-- Global management policy
INSERT INTO consul_acl_policies (policy_id, name, description, rules)
VALUES ('00000000-0000-0000-0000-000000000001', 'global-management', 'Builtin global management policy',
'agent_prefix "" { policy = "write" }
key_prefix "" { policy = "write" }
node_prefix "" { policy = "write" }
service_prefix "" { policy = "write" }
session_prefix "" { policy = "write" }
query_prefix "" { policy = "write" }');

-- Create update trigger function for modify_time
CREATE OR REPLACE FUNCTION update_modify_time()
RETURNS TRIGGER AS $$
BEGIN
    NEW.modify_time = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply trigger to tables with modify_time
CREATE TRIGGER update_consul_acl_tokens_modify_time
    BEFORE UPDATE ON consul_acl_tokens
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_consul_acl_policies_modify_time
    BEFORE UPDATE ON consul_acl_policies
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

/******************************************/
/*   Service Discovery Tables             */
/******************************************/

-- Service metadata table
CREATE TABLE service_info (
    id BIGSERIAL PRIMARY KEY,
    namespace_id VARCHAR(128) NOT NULL DEFAULT 'public',
    group_name VARCHAR(128) NOT NULL DEFAULT 'DEFAULT_GROUP',
    service_name VARCHAR(255) NOT NULL,
    protect_threshold REAL NOT NULL DEFAULT 0.0,
    metadata TEXT,
    selector TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    gmt_create TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    gmt_modified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_service UNIQUE (namespace_id, group_name, service_name)
);

CREATE INDEX idx_service_namespace ON service_info(namespace_id);

COMMENT ON TABLE service_info IS 'Service metadata table';
COMMENT ON COLUMN service_info.namespace_id IS 'Namespace ID';
COMMENT ON COLUMN service_info.group_name IS 'Service group name';
COMMENT ON COLUMN service_info.service_name IS 'Service name';
COMMENT ON COLUMN service_info.protect_threshold IS 'Protection threshold (0.0-1.0)';
COMMENT ON COLUMN service_info.metadata IS 'Service metadata (JSON)';
COMMENT ON COLUMN service_info.selector IS 'Service selector (JSON)';
COMMENT ON COLUMN service_info.enabled IS 'Whether service is enabled';

-- Cluster metadata table
CREATE TABLE cluster_info (
    id BIGSERIAL PRIMARY KEY,
    service_id BIGINT NOT NULL REFERENCES service_info(id) ON DELETE CASCADE,
    cluster_name VARCHAR(128) NOT NULL DEFAULT 'DEFAULT',
    health_check_type VARCHAR(32) DEFAULT 'TCP',
    health_check_port INTEGER DEFAULT 0,
    health_check_path VARCHAR(255) DEFAULT '',
    use_instance_port BOOLEAN DEFAULT TRUE,
    metadata TEXT,
    gmt_create TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    gmt_modified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_cluster UNIQUE (service_id, cluster_name)
);

COMMENT ON TABLE cluster_info IS 'Cluster metadata table';
COMMENT ON COLUMN cluster_info.service_id IS 'Service ID (FK to service_info)';
COMMENT ON COLUMN cluster_info.cluster_name IS 'Cluster name';
COMMENT ON COLUMN cluster_info.health_check_type IS 'Health check type (TCP/HTTP/MYSQL/NONE)';
COMMENT ON COLUMN cluster_info.health_check_port IS 'Health check port (0 means use instance port)';
COMMENT ON COLUMN cluster_info.health_check_path IS 'Health check path (for HTTP)';
COMMENT ON COLUMN cluster_info.use_instance_port IS 'Whether to use instance port for health check';
COMMENT ON COLUMN cluster_info.metadata IS 'Cluster metadata (JSON)';

-- Instance table (for persistent instances)
CREATE TABLE instance_info (
    id BIGSERIAL PRIMARY KEY,
    instance_id VARCHAR(255) NOT NULL,
    service_id BIGINT NOT NULL REFERENCES service_info(id) ON DELETE CASCADE,
    cluster_name VARCHAR(128) NOT NULL DEFAULT 'DEFAULT',
    ip VARCHAR(45) NOT NULL,
    port INTEGER NOT NULL,
    weight DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    healthy BOOLEAN NOT NULL DEFAULT TRUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    ephemeral BOOLEAN NOT NULL DEFAULT TRUE,
    metadata TEXT,
    heartbeat_interval INTEGER DEFAULT 5000,
    heartbeat_timeout INTEGER DEFAULT 15000,
    ip_delete_timeout INTEGER DEFAULT 30000,
    last_heartbeat TIMESTAMP DEFAULT NULL,
    gmt_create TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    gmt_modified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_instance UNIQUE (instance_id)
);

CREATE INDEX idx_instance_service_cluster ON instance_info(service_id, cluster_name);
CREATE INDEX idx_instance_ip_port ON instance_info(ip, port);
CREATE INDEX idx_instance_healthy ON instance_info(healthy);
CREATE INDEX idx_instance_ephemeral ON instance_info(ephemeral);

COMMENT ON TABLE instance_info IS 'Service instance table';
COMMENT ON COLUMN instance_info.instance_id IS 'Instance ID (unique identifier)';
COMMENT ON COLUMN instance_info.service_id IS 'Service ID (FK to service_info)';
COMMENT ON COLUMN instance_info.cluster_name IS 'Cluster name';
COMMENT ON COLUMN instance_info.ip IS 'Instance IP address';
COMMENT ON COLUMN instance_info.port IS 'Instance port';
COMMENT ON COLUMN instance_info.weight IS 'Instance weight';
COMMENT ON COLUMN instance_info.healthy IS 'Whether instance is healthy';
COMMENT ON COLUMN instance_info.enabled IS 'Whether instance is enabled';
COMMENT ON COLUMN instance_info.ephemeral IS 'Whether instance is ephemeral';
COMMENT ON COLUMN instance_info.metadata IS 'Instance metadata (JSON)';
COMMENT ON COLUMN instance_info.heartbeat_interval IS 'Heartbeat interval in ms';
COMMENT ON COLUMN instance_info.heartbeat_timeout IS 'Heartbeat timeout in ms';
COMMENT ON COLUMN instance_info.ip_delete_timeout IS 'IP delete timeout in ms';
COMMENT ON COLUMN instance_info.last_heartbeat IS 'Last heartbeat time';

-- Apply update trigger to service discovery tables
CREATE TRIGGER update_service_info_modify_time
    BEFORE UPDATE ON service_info
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_cluster_info_modify_time
    BEFORE UPDATE ON cluster_info
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_instance_info_modify_time
    BEFORE UPDATE ON instance_info
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

/******************************************/
/*   Operation Audit Log Table            */
/******************************************/
CREATE TABLE operation_log (
    id BIGSERIAL PRIMARY KEY,
    operation VARCHAR(32) NOT NULL,
    resource_type VARCHAR(32) NOT NULL,
    resource_id TEXT,
    tenant_id TEXT,
    operator VARCHAR(128) NOT NULL,
    source_ip TEXT,
    result VARCHAR(16) NOT NULL,
    error_message TEXT,
    details TEXT,
    gmt_create TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_oplog_operation ON operation_log(operation);
CREATE INDEX idx_oplog_resource_type ON operation_log(resource_type);
CREATE INDEX idx_oplog_operator ON operation_log(operator);
CREATE INDEX idx_oplog_result ON operation_log(result);
CREATE INDEX idx_oplog_gmt_create ON operation_log(gmt_create);

COMMENT ON TABLE operation_log IS 'Operation audit log table';
COMMENT ON COLUMN operation_log.operation IS 'Operation type (CREATE, UPDATE, DELETE, QUERY, LOGIN, etc.)';
COMMENT ON COLUMN operation_log.resource_type IS 'Resource type (CONFIG, SERVICE, INSTANCE, USER, etc.)';
COMMENT ON COLUMN operation_log.resource_id IS 'Resource identifier';
COMMENT ON COLUMN operation_log.tenant_id IS 'Tenant/Namespace ID';
COMMENT ON COLUMN operation_log.operator IS 'User who performed the operation';
COMMENT ON COLUMN operation_log.source_ip IS 'Source IP address';
COMMENT ON COLUMN operation_log.result IS 'Result (SUCCESS, FAILURE)';
COMMENT ON COLUMN operation_log.error_message IS 'Error message if operation failed';
COMMENT ON COLUMN operation_log.details IS 'Additional details in JSON format';
COMMENT ON COLUMN operation_log.gmt_create IS 'When the operation occurred';

/******************************************/
/*   Aggregate Config Table               */
/******************************************/
CREATE TABLE config_info_aggr (
    id BIGSERIAL PRIMARY KEY,
    data_id VARCHAR(255) NOT NULL,
    group_id VARCHAR(128) NOT NULL,
    datum_id VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    md5 VARCHAR(32) DEFAULT NULL,
    tenant_id VARCHAR(128) NOT NULL DEFAULT '',
    app_name VARCHAR(128) DEFAULT NULL,
    gmt_create TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    gmt_modified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_configinfoaggr_datagrouptenantdatum UNIQUE (data_id, group_id, tenant_id, datum_id)
);

CREATE INDEX idx_aggr_tenant_id ON config_info_aggr(tenant_id);

COMMENT ON TABLE config_info_aggr IS 'Aggregate configuration table';
COMMENT ON COLUMN config_info_aggr.data_id IS 'Parent configuration data ID';
COMMENT ON COLUMN config_info_aggr.group_id IS 'Configuration group';
COMMENT ON COLUMN config_info_aggr.datum_id IS 'Unique datum ID within the aggregate';
COMMENT ON COLUMN config_info_aggr.content IS 'Configuration content';
COMMENT ON COLUMN config_info_aggr.md5 IS 'MD5 hash of content';
COMMENT ON COLUMN config_info_aggr.tenant_id IS 'Tenant/Namespace ID';
COMMENT ON COLUMN config_info_aggr.app_name IS 'Application name';

-- Apply update trigger to config_info_aggr
CREATE TRIGGER update_config_info_aggr_modify_time
    BEFORE UPDATE ON config_info_aggr
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

/******************************************/
/*   Apollo Config Compatibility Tables   */
/******************************************/

/******************************************/
/*   Table: apollo_app                    */
/******************************************/
CREATE TABLE apollo_app (
    id BIGSERIAL PRIMARY KEY,
    app_id VARCHAR(64) NOT NULL,
    name VARCHAR(128) NOT NULL,
    org_id VARCHAR(32) DEFAULT '',
    org_name VARCHAR(64) DEFAULT '',
    owner_name VARCHAR(64) NOT NULL,
    owner_email VARCHAR(128) DEFAULT '',
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_apollo_app_app_id UNIQUE (app_id)
);

CREATE INDEX idx_apollo_app_name ON apollo_app(name);
CREATE INDEX idx_apollo_app_org_id ON apollo_app(org_id);
CREATE INDEX idx_apollo_app_owner_name ON apollo_app(owner_name);

COMMENT ON TABLE apollo_app IS 'Apollo application table';

/******************************************/
/*   Table: apollo_app_namespace          */
/******************************************/
CREATE TABLE apollo_app_namespace (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(128) NOT NULL,
    app_id VARCHAR(64) NOT NULL,
    format VARCHAR(32) DEFAULT 'properties',
    is_public BOOLEAN DEFAULT FALSE,
    comment VARCHAR(256) DEFAULT '',
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_apollo_app_namespace_app_id_name UNIQUE (app_id, name)
);

CREATE INDEX idx_apollo_app_namespace_app_id ON apollo_app_namespace(app_id);
CREATE INDEX idx_apollo_app_namespace_name ON apollo_app_namespace(name);

COMMENT ON TABLE apollo_app_namespace IS 'Apollo application namespace table';

/******************************************/
/*   Table: apollo_cluster                */
/******************************************/
CREATE TABLE apollo_cluster (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(32) NOT NULL,
    app_id VARCHAR(64) NOT NULL,
    parent_cluster_id BIGINT DEFAULT 0,
    comment VARCHAR(256) DEFAULT '',
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_apollo_cluster_app_id_name UNIQUE (app_id, name)
);

CREATE INDEX idx_apollo_cluster_app_id ON apollo_cluster(app_id);
CREATE INDEX idx_apollo_cluster_parent_cluster_id ON apollo_cluster(parent_cluster_id);

COMMENT ON TABLE apollo_cluster IS 'Apollo cluster table';

/******************************************/
/*   Table: apollo_namespace              */
/******************************************/
CREATE TABLE apollo_namespace (
    id BIGSERIAL PRIMARY KEY,
    app_id VARCHAR(64) NOT NULL,
    cluster_name VARCHAR(32) NOT NULL,
    namespace_name VARCHAR(128) NOT NULL,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_apollo_namespace_app_cluster_ns UNIQUE (app_id, cluster_name, namespace_name)
);

CREATE INDEX idx_apollo_namespace_app_id ON apollo_namespace(app_id);
CREATE INDEX idx_apollo_namespace_cluster_name ON apollo_namespace(cluster_name);
CREATE INDEX idx_apollo_namespace_namespace_name ON apollo_namespace(namespace_name);

COMMENT ON TABLE apollo_namespace IS 'Apollo namespace table';

/******************************************/
/*   Table: apollo_item                   */
/******************************************/
CREATE TABLE apollo_item (
    id BIGSERIAL PRIMARY KEY,
    namespace_id BIGINT NOT NULL,
    key VARCHAR(128) NOT NULL DEFAULT '',
    type SMALLINT DEFAULT 0,
    value TEXT,
    comment VARCHAR(256) DEFAULT '',
    line_num INTEGER DEFAULT 0,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_apollo_item_namespace_id ON apollo_item(namespace_id);
CREATE INDEX idx_apollo_item_key ON apollo_item(key);

COMMENT ON TABLE apollo_item IS 'Apollo configuration item table';

/******************************************/
/*   Table: apollo_release                */
/******************************************/
CREATE TABLE apollo_release (
    id BIGSERIAL PRIMARY KEY,
    release_key VARCHAR(64) NOT NULL,
    name VARCHAR(64) DEFAULT '',
    app_id VARCHAR(64) NOT NULL,
    cluster_name VARCHAR(32) NOT NULL,
    namespace_name VARCHAR(128) NOT NULL,
    configurations TEXT,
    comment VARCHAR(256) DEFAULT '',
    is_abandoned BOOLEAN DEFAULT FALSE,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_apollo_release_release_key UNIQUE (release_key)
);

CREATE INDEX idx_apollo_release_app_cluster_ns ON apollo_release(app_id, cluster_name, namespace_name);

COMMENT ON TABLE apollo_release IS 'Apollo configuration release table';

/******************************************/
/*   Table: apollo_release_history        */
/******************************************/
CREATE TABLE apollo_release_history (
    id BIGSERIAL PRIMARY KEY,
    app_id VARCHAR(64) NOT NULL,
    cluster_name VARCHAR(32) NOT NULL,
    namespace_name VARCHAR(128) NOT NULL,
    branch_name VARCHAR(32) DEFAULT 'default',
    release_id BIGINT DEFAULT 0,
    previous_release_id BIGINT DEFAULT 0,
    operation SMALLINT DEFAULT 0,
    operation_context TEXT,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_apollo_release_history_app_cluster_ns ON apollo_release_history(app_id, cluster_name, namespace_name);

COMMENT ON TABLE apollo_release_history IS 'Apollo release history table';

/******************************************/
/*   Table: apollo_release_message        */
/******************************************/
CREATE TABLE apollo_release_message (
    id BIGSERIAL PRIMARY KEY,
    message VARCHAR(1024) NOT NULL DEFAULT '',
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_apollo_release_message_message ON apollo_release_message(message);

COMMENT ON TABLE apollo_release_message IS 'Apollo release message table for notification';

/******************************************/
/*   Table: apollo_gray_release_rule      */
/******************************************/
CREATE TABLE apollo_gray_release_rule (
    id BIGSERIAL PRIMARY KEY,
    app_id VARCHAR(64) NOT NULL,
    cluster_name VARCHAR(32) NOT NULL,
    namespace_name VARCHAR(128) NOT NULL,
    branch_name VARCHAR(32) NOT NULL,
    rules TEXT,
    release_id BIGINT DEFAULT 0,
    branch_status SMALLINT DEFAULT 0,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_apollo_gray_release_rule_app_cluster_ns ON apollo_gray_release_rule(app_id, cluster_name, namespace_name);

COMMENT ON TABLE apollo_gray_release_rule IS 'Apollo gray release rule table';

/******************************************/
/*   Table: apollo_namespace_lock         */
/******************************************/
CREATE TABLE apollo_namespace_lock (
    id BIGSERIAL PRIMARY KEY,
    namespace_id BIGINT NOT NULL,
    locked_by VARCHAR(64) DEFAULT '',
    lock_time TIMESTAMP DEFAULT NULL,
    expire_time TIMESTAMP DEFAULT NULL,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uk_apollo_namespace_lock_namespace_id UNIQUE (namespace_id)
);

COMMENT ON TABLE apollo_namespace_lock IS 'Apollo namespace editing lock table';

/******************************************/
/*   Table: apollo_commit                 */
/******************************************/
CREATE TABLE apollo_commit (
    id BIGSERIAL PRIMARY KEY,
    change_sets TEXT,
    app_id VARCHAR(64) NOT NULL,
    cluster_name VARCHAR(32) NOT NULL,
    namespace_name VARCHAR(128) NOT NULL,
    comment VARCHAR(256) DEFAULT '',
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_apollo_commit_app_cluster_ns ON apollo_commit(app_id, cluster_name, namespace_name);

COMMENT ON TABLE apollo_commit IS 'Apollo configuration change commit table';

/******************************************/
/*   Table: apollo_access_key             */
/******************************************/
CREATE TABLE apollo_access_key (
    id BIGSERIAL PRIMARY KEY,
    app_id VARCHAR(64) NOT NULL,
    secret VARCHAR(128) NOT NULL,
    mode VARCHAR(16) DEFAULT 'STS',
    is_enabled BOOLEAN DEFAULT TRUE,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_apollo_access_key_app_id ON apollo_access_key(app_id);

COMMENT ON TABLE apollo_access_key IS 'Apollo access key table';

/******************************************/
/*   Table: apollo_instance               */
/******************************************/
CREATE TABLE apollo_instance (
    id BIGSERIAL PRIMARY KEY,
    app_id VARCHAR(64) NOT NULL,
    cluster_name VARCHAR(32) NOT NULL,
    data_center VARCHAR(64) DEFAULT '',
    ip VARCHAR(45) NOT NULL,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_apollo_instance_app_cluster ON apollo_instance(app_id, cluster_name);

COMMENT ON TABLE apollo_instance IS 'Apollo client instance table';

/******************************************/
/*   Table: apollo_instance_config        */
/******************************************/
CREATE TABLE apollo_instance_config (
    id BIGSERIAL PRIMARY KEY,
    instance_id BIGINT NOT NULL,
    config_app_id VARCHAR(64),
    config_cluster_name VARCHAR(32),
    config_namespace_name VARCHAR(128),
    release_key VARCHAR(64) DEFAULT '',
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_apollo_instance_config_instance_id ON apollo_instance_config(instance_id);

COMMENT ON TABLE apollo_instance_config IS 'Apollo instance configuration mapping table';

/******************************************/
/*   Table: apollo_audit                  */
/******************************************/
CREATE TABLE apollo_audit (
    id BIGSERIAL PRIMARY KEY,
    entity_name VARCHAR(64) NOT NULL,
    entity_id VARCHAR(64) DEFAULT '',
    op_name VARCHAR(64) NOT NULL,
    comment VARCHAR(256) DEFAULT '',
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP DEFAULT NULL,
    created_by VARCHAR(64) DEFAULT '',
    created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_by VARCHAR(64) DEFAULT '',
    last_modified_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_apollo_audit_entity_name ON apollo_audit(entity_name);
CREATE INDEX idx_apollo_audit_entity_id ON apollo_audit(entity_id);

COMMENT ON TABLE apollo_audit IS 'Apollo audit log table';

/******************************************/
/*   Apollo Update Triggers               */
/******************************************/
CREATE TRIGGER update_apollo_app_modify_time
    BEFORE UPDATE ON apollo_app
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_app_namespace_modify_time
    BEFORE UPDATE ON apollo_app_namespace
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_cluster_modify_time
    BEFORE UPDATE ON apollo_cluster
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_namespace_modify_time
    BEFORE UPDATE ON apollo_namespace
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_item_modify_time
    BEFORE UPDATE ON apollo_item
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_release_modify_time
    BEFORE UPDATE ON apollo_release
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_release_history_modify_time
    BEFORE UPDATE ON apollo_release_history
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_release_message_modify_time
    BEFORE UPDATE ON apollo_release_message
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_gray_release_rule_modify_time
    BEFORE UPDATE ON apollo_gray_release_rule
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_namespace_lock_modify_time
    BEFORE UPDATE ON apollo_namespace_lock
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_commit_modify_time
    BEFORE UPDATE ON apollo_commit
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_access_key_modify_time
    BEFORE UPDATE ON apollo_access_key
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_instance_modify_time
    BEFORE UPDATE ON apollo_instance
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_instance_config_modify_time
    BEFORE UPDATE ON apollo_instance_config
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();

CREATE TRIGGER update_apollo_audit_modify_time
    BEFORE UPDATE ON apollo_audit
    FOR EACH ROW EXECUTE FUNCTION update_modify_time();
