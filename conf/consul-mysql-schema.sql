/*
 * Consul ACL Compatibility Schema (MySQL)
 *
 * This file contains the Consul ACL tables for Batata's Consul API compatibility layer.
 * Run this after the core Nacos schema (mysql-schema.sql) if you need Consul compatibility.
 *
 * Usage:
 *   mysql -u user -p batata < conf/consul-mysql-schema.sql
 */

/******************************************/
/*   Consul ACL Tables                     */
/******************************************/

-- Consul ACL Tokens
CREATE TABLE `consul_acl_tokens` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'id',
    `accessor_id` varchar(36) NOT NULL COMMENT 'Accessor ID (UUID)',
    `secret_id` varchar(36) NOT NULL COMMENT 'Secret ID (the actual token)',
    `description` varchar(256) DEFAULT '' COMMENT 'Token description',
    `policies` text COMMENT 'JSON array of policy IDs',
    `roles` text COMMENT 'JSON array of role names',
    `local` boolean NOT NULL DEFAULT FALSE COMMENT 'Whether token is local to datacenter',
    `expiration_time` datetime DEFAULT NULL COMMENT 'Token expiration time',
    `create_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `modify_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Modification time',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_accessor_id` (`accessor_id`),
    UNIQUE KEY `uk_secret_id` (`secret_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8 COLLATE=utf8_bin COMMENT='Consul ACL Tokens';

-- Consul ACL Policies
CREATE TABLE `consul_acl_policies` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'id',
    `policy_id` varchar(36) NOT NULL COMMENT 'Policy ID (UUID)',
    `name` varchar(128) NOT NULL COMMENT 'Policy name',
    `description` varchar(256) DEFAULT '' COMMENT 'Policy description',
    `rules` text NOT NULL COMMENT 'HCL/JSON policy rules',
    `datacenters` text COMMENT 'JSON array of datacenter names',
    `create_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `modify_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Modification time',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_policy_id` (`policy_id`),
    UNIQUE KEY `uk_name` (`name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8 COLLATE=utf8_bin COMMENT='Consul ACL Policies';

-- Bootstrap token (management token)
INSERT INTO `consul_acl_tokens` (`accessor_id`, `secret_id`, `description`, `policies`, `roles`, `local`)
VALUES ('00000000-0000-0000-0000-000000000001', 'root', 'Bootstrap Token (Management)', '["global-management"]', '[]', FALSE);

-- Global management policy
INSERT INTO `consul_acl_policies` (`policy_id`, `name`, `description`, `rules`)
VALUES ('00000000-0000-0000-0000-000000000001', 'global-management', 'Builtin global management policy',
'agent_prefix "" { policy = "write" }
key_prefix "" { policy = "write" }
node_prefix "" { policy = "write" }
service_prefix "" { policy = "write" }
session_prefix "" { policy = "write" }
query_prefix "" { policy = "write" }');
