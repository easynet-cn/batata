/*
 * Consul ACL Compatibility Schema (PostgreSQL)
 *
 * This file contains the Consul ACL tables for Batata's Consul API compatibility layer.
 * Run this after the core Nacos schema (postgresql-schema.sql) if you need Consul compatibility.
 *
 * Usage:
 *   psql -U user -d batata -f conf/consul-postgresql-schema.sql
 */

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

-- Create update trigger function for modify_time (if not already created)
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
