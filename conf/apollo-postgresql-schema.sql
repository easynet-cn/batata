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

-- PostgreSQL Schema for Batata Apollo Config Compatibility Plugin
-- This file contains the PostgreSQL-compatible schema definitions for Apollo tables
-- Assumes the update_modify_time() trigger function already exists from postgresql-schema.sql

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
COMMENT ON COLUMN apollo_app.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_app.app_id IS 'Application ID (unique identifier)';
COMMENT ON COLUMN apollo_app.name IS 'Application name';
COMMENT ON COLUMN apollo_app.org_id IS 'Organization ID';
COMMENT ON COLUMN apollo_app.org_name IS 'Organization name';
COMMENT ON COLUMN apollo_app.owner_name IS 'Application owner name';
COMMENT ON COLUMN apollo_app.owner_email IS 'Application owner email';
COMMENT ON COLUMN apollo_app.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_app.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_app.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_app.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_app.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_app.last_modified_time IS 'Last modification timestamp';

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
COMMENT ON COLUMN apollo_app_namespace.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_app_namespace.name IS 'Namespace name';
COMMENT ON COLUMN apollo_app_namespace.app_id IS 'Application ID';
COMMENT ON COLUMN apollo_app_namespace.format IS 'Configuration format (properties, xml, json, yml, yaml, txt)';
COMMENT ON COLUMN apollo_app_namespace.is_public IS 'Whether namespace is public';
COMMENT ON COLUMN apollo_app_namespace.comment IS 'Namespace description';
COMMENT ON COLUMN apollo_app_namespace.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_app_namespace.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_app_namespace.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_app_namespace.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_app_namespace.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_app_namespace.last_modified_time IS 'Last modification timestamp';

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
COMMENT ON COLUMN apollo_cluster.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_cluster.name IS 'Cluster name';
COMMENT ON COLUMN apollo_cluster.app_id IS 'Application ID';
COMMENT ON COLUMN apollo_cluster.parent_cluster_id IS 'Parent cluster ID';
COMMENT ON COLUMN apollo_cluster.comment IS 'Cluster description';
COMMENT ON COLUMN apollo_cluster.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_cluster.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_cluster.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_cluster.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_cluster.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_cluster.last_modified_time IS 'Last modification timestamp';

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
COMMENT ON COLUMN apollo_namespace.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_namespace.app_id IS 'Application ID';
COMMENT ON COLUMN apollo_namespace.cluster_name IS 'Cluster name';
COMMENT ON COLUMN apollo_namespace.namespace_name IS 'Namespace name';
COMMENT ON COLUMN apollo_namespace.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_namespace.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_namespace.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_namespace.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_namespace.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_namespace.last_modified_time IS 'Last modification timestamp';

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
COMMENT ON COLUMN apollo_item.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_item.namespace_id IS 'Namespace ID (FK to apollo_namespace)';
COMMENT ON COLUMN apollo_item.key IS 'Configuration key';
COMMENT ON COLUMN apollo_item.type IS 'Item type (0: String, 1: Number, 2: Boolean, 3: JSON)';
COMMENT ON COLUMN apollo_item.value IS 'Configuration value';
COMMENT ON COLUMN apollo_item.comment IS 'Item description';
COMMENT ON COLUMN apollo_item.line_num IS 'Line number for ordering';
COMMENT ON COLUMN apollo_item.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_item.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_item.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_item.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_item.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_item.last_modified_time IS 'Last modification timestamp';

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

CREATE INDEX idx_apollo_release_app_id ON apollo_release(app_id);
CREATE INDEX idx_apollo_release_cluster_name ON apollo_release(cluster_name);
CREATE INDEX idx_apollo_release_namespace_name ON apollo_release(namespace_name);
CREATE INDEX idx_apollo_release_app_cluster_ns ON apollo_release(app_id, cluster_name, namespace_name);

COMMENT ON TABLE apollo_release IS 'Apollo configuration release table';
COMMENT ON COLUMN apollo_release.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_release.release_key IS 'Release key (unique identifier)';
COMMENT ON COLUMN apollo_release.name IS 'Release name';
COMMENT ON COLUMN apollo_release.app_id IS 'Application ID';
COMMENT ON COLUMN apollo_release.cluster_name IS 'Cluster name';
COMMENT ON COLUMN apollo_release.namespace_name IS 'Namespace name';
COMMENT ON COLUMN apollo_release.configurations IS 'Released configuration content (JSON)';
COMMENT ON COLUMN apollo_release.comment IS 'Release description';
COMMENT ON COLUMN apollo_release.is_abandoned IS 'Whether this release has been rolled back';
COMMENT ON COLUMN apollo_release.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_release.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_release.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_release.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_release.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_release.last_modified_time IS 'Last modification timestamp';

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

CREATE INDEX idx_apollo_release_history_app_id ON apollo_release_history(app_id);
CREATE INDEX idx_apollo_release_history_cluster_name ON apollo_release_history(cluster_name);
CREATE INDEX idx_apollo_release_history_namespace_name ON apollo_release_history(namespace_name);
CREATE INDEX idx_apollo_release_history_release_id ON apollo_release_history(release_id);
CREATE INDEX idx_apollo_release_history_previous_release_id ON apollo_release_history(previous_release_id);
CREATE INDEX idx_apollo_release_history_app_cluster_ns ON apollo_release_history(app_id, cluster_name, namespace_name);

COMMENT ON TABLE apollo_release_history IS 'Apollo release history table';
COMMENT ON COLUMN apollo_release_history.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_release_history.app_id IS 'Application ID';
COMMENT ON COLUMN apollo_release_history.cluster_name IS 'Cluster name';
COMMENT ON COLUMN apollo_release_history.namespace_name IS 'Namespace name';
COMMENT ON COLUMN apollo_release_history.branch_name IS 'Branch name (default or gray release branch)';
COMMENT ON COLUMN apollo_release_history.release_id IS 'Release ID';
COMMENT ON COLUMN apollo_release_history.previous_release_id IS 'Previous release ID';
COMMENT ON COLUMN apollo_release_history.operation IS 'Operation type (0: normal publish, 1: rollback, etc.)';
COMMENT ON COLUMN apollo_release_history.operation_context IS 'Operation context (JSON)';
COMMENT ON COLUMN apollo_release_history.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_release_history.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_release_history.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_release_history.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_release_history.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_release_history.last_modified_time IS 'Last modification timestamp';

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
COMMENT ON COLUMN apollo_release_message.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_release_message.message IS 'Release message content (format: appId+cluster+namespace)';
COMMENT ON COLUMN apollo_release_message.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_release_message.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_release_message.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_release_message.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_release_message.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_release_message.last_modified_time IS 'Last modification timestamp';

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

CREATE INDEX idx_apollo_gray_release_rule_app_id ON apollo_gray_release_rule(app_id);
CREATE INDEX idx_apollo_gray_release_rule_app_cluster_ns ON apollo_gray_release_rule(app_id, cluster_name, namespace_name);
CREATE INDEX idx_apollo_gray_release_rule_branch_name ON apollo_gray_release_rule(branch_name);

COMMENT ON TABLE apollo_gray_release_rule IS 'Apollo gray release rule table';
COMMENT ON COLUMN apollo_gray_release_rule.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_gray_release_rule.app_id IS 'Application ID';
COMMENT ON COLUMN apollo_gray_release_rule.cluster_name IS 'Cluster name';
COMMENT ON COLUMN apollo_gray_release_rule.namespace_name IS 'Namespace name';
COMMENT ON COLUMN apollo_gray_release_rule.branch_name IS 'Gray release branch name';
COMMENT ON COLUMN apollo_gray_release_rule.rules IS 'Gray release rules (JSON)';
COMMENT ON COLUMN apollo_gray_release_rule.release_id IS 'Associated release ID';
COMMENT ON COLUMN apollo_gray_release_rule.branch_status IS 'Branch status (0: inactive, 1: active)';
COMMENT ON COLUMN apollo_gray_release_rule.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_gray_release_rule.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_gray_release_rule.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_gray_release_rule.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_gray_release_rule.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_gray_release_rule.last_modified_time IS 'Last modification timestamp';

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
COMMENT ON COLUMN apollo_namespace_lock.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_namespace_lock.namespace_id IS 'Namespace ID (FK to apollo_namespace)';
COMMENT ON COLUMN apollo_namespace_lock.locked_by IS 'User who holds the lock';
COMMENT ON COLUMN apollo_namespace_lock.lock_time IS 'Lock acquisition time';
COMMENT ON COLUMN apollo_namespace_lock.expire_time IS 'Lock expiration time';
COMMENT ON COLUMN apollo_namespace_lock.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_namespace_lock.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_namespace_lock.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_namespace_lock.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_namespace_lock.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_namespace_lock.last_modified_time IS 'Last modification timestamp';

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

CREATE INDEX idx_apollo_commit_app_id ON apollo_commit(app_id);
CREATE INDEX idx_apollo_commit_cluster_name ON apollo_commit(cluster_name);
CREATE INDEX idx_apollo_commit_namespace_name ON apollo_commit(namespace_name);
CREATE INDEX idx_apollo_commit_app_cluster_ns ON apollo_commit(app_id, cluster_name, namespace_name);
CREATE INDEX idx_apollo_commit_created_time ON apollo_commit(created_time);

COMMENT ON TABLE apollo_commit IS 'Apollo configuration change commit table';
COMMENT ON COLUMN apollo_commit.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_commit.change_sets IS 'Change sets content (JSON)';
COMMENT ON COLUMN apollo_commit.app_id IS 'Application ID';
COMMENT ON COLUMN apollo_commit.cluster_name IS 'Cluster name';
COMMENT ON COLUMN apollo_commit.namespace_name IS 'Namespace name';
COMMENT ON COLUMN apollo_commit.comment IS 'Commit description';
COMMENT ON COLUMN apollo_commit.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_commit.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_commit.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_commit.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_commit.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_commit.last_modified_time IS 'Last modification timestamp';

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
CREATE INDEX idx_apollo_access_key_secret ON apollo_access_key(secret);

COMMENT ON TABLE apollo_access_key IS 'Apollo access key table for application authentication';
COMMENT ON COLUMN apollo_access_key.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_access_key.app_id IS 'Application ID';
COMMENT ON COLUMN apollo_access_key.secret IS 'Access key secret';
COMMENT ON COLUMN apollo_access_key.mode IS 'Key mode (STS or other)';
COMMENT ON COLUMN apollo_access_key.is_enabled IS 'Whether the access key is enabled';
COMMENT ON COLUMN apollo_access_key.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_access_key.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_access_key.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_access_key.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_access_key.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_access_key.last_modified_time IS 'Last modification timestamp';

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

CREATE INDEX idx_apollo_instance_app_id ON apollo_instance(app_id);
CREATE INDEX idx_apollo_instance_cluster_name ON apollo_instance(cluster_name);
CREATE INDEX idx_apollo_instance_ip ON apollo_instance(ip);
CREATE INDEX idx_apollo_instance_data_center ON apollo_instance(data_center);
CREATE INDEX idx_apollo_instance_app_cluster_ip_dc ON apollo_instance(app_id, cluster_name, ip, data_center);

COMMENT ON TABLE apollo_instance IS 'Apollo client instance table';
COMMENT ON COLUMN apollo_instance.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_instance.app_id IS 'Application ID';
COMMENT ON COLUMN apollo_instance.cluster_name IS 'Cluster name';
COMMENT ON COLUMN apollo_instance.data_center IS 'Data center name';
COMMENT ON COLUMN apollo_instance.ip IS 'Client instance IP address';
COMMENT ON COLUMN apollo_instance.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_instance.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_instance.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_instance.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_instance.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_instance.last_modified_time IS 'Last modification timestamp';

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
CREATE INDEX idx_apollo_instance_config_config_app_id ON apollo_instance_config(config_app_id);
CREATE INDEX idx_apollo_instance_config_release_key ON apollo_instance_config(release_key);
CREATE INDEX idx_apollo_instance_config_instance_config ON apollo_instance_config(instance_id, config_app_id, config_namespace_name);

COMMENT ON TABLE apollo_instance_config IS 'Apollo instance configuration mapping table';
COMMENT ON COLUMN apollo_instance_config.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_instance_config.instance_id IS 'Instance ID (FK to apollo_instance)';
COMMENT ON COLUMN apollo_instance_config.config_app_id IS 'Configuration application ID';
COMMENT ON COLUMN apollo_instance_config.config_cluster_name IS 'Configuration cluster name';
COMMENT ON COLUMN apollo_instance_config.config_namespace_name IS 'Configuration namespace name';
COMMENT ON COLUMN apollo_instance_config.release_key IS 'Current release key the instance is using';
COMMENT ON COLUMN apollo_instance_config.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_instance_config.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_instance_config.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_instance_config.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_instance_config.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_instance_config.last_modified_time IS 'Last modification timestamp';

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
CREATE INDEX idx_apollo_audit_op_name ON apollo_audit(op_name);
CREATE INDEX idx_apollo_audit_created_time ON apollo_audit(created_time);
CREATE INDEX idx_apollo_audit_created_by ON apollo_audit(created_by);

COMMENT ON TABLE apollo_audit IS 'Apollo audit log table';
COMMENT ON COLUMN apollo_audit.id IS 'Primary key ID';
COMMENT ON COLUMN apollo_audit.entity_name IS 'Audited entity name (e.g., App, Cluster, Namespace, Item, Release)';
COMMENT ON COLUMN apollo_audit.entity_id IS 'Audited entity ID';
COMMENT ON COLUMN apollo_audit.op_name IS 'Operation name (e.g., INSERT, UPDATE, DELETE)';
COMMENT ON COLUMN apollo_audit.comment IS 'Audit description';
COMMENT ON COLUMN apollo_audit.is_deleted IS 'Soft delete flag';
COMMENT ON COLUMN apollo_audit.deleted_at IS 'Deletion timestamp';
COMMENT ON COLUMN apollo_audit.created_by IS 'Created by user';
COMMENT ON COLUMN apollo_audit.created_time IS 'Creation timestamp';
COMMENT ON COLUMN apollo_audit.last_modified_by IS 'Last modified by user';
COMMENT ON COLUMN apollo_audit.last_modified_time IS 'Last modification timestamp';

/******************************************/
/*   Update Triggers                      */
/******************************************/
-- Apply update_modify_time() trigger to all Apollo tables
-- The trigger function is assumed to already exist from postgresql-schema.sql
-- It updates last_modified_time on row update

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
