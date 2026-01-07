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
