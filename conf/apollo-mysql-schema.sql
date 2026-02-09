/*
 * Copyright 2024 Batata Authors.
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

/******************************************/
/*   Apollo Config Compatibility Tables   */
/******************************************/

/******************************************/
/*   Table: apollo_app                    */
/******************************************/
CREATE TABLE `apollo_app` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `app_id` varchar(64) NOT NULL COMMENT 'Application ID',
    `name` varchar(128) NOT NULL COMMENT 'Application name',
    `org_id` varchar(32) DEFAULT '' COMMENT 'Organization ID',
    `org_name` varchar(64) DEFAULT '' COMMENT 'Organization name',
    `owner_name` varchar(64) NOT NULL COMMENT 'Owner name',
    `owner_email` varchar(128) DEFAULT '' COMMENT 'Owner email address',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_app_id` (`app_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo application table';

/******************************************/
/*   Table: apollo_app_namespace          */
/******************************************/
CREATE TABLE `apollo_app_namespace` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `name` varchar(128) NOT NULL COMMENT 'Namespace name',
    `app_id` varchar(64) NOT NULL COMMENT 'Application ID',
    `format` varchar(32) DEFAULT 'properties' COMMENT 'Configuration format (properties, xml, json, yml, yaml, txt)',
    `is_public` boolean DEFAULT FALSE COMMENT 'Whether namespace is public',
    `comment` varchar(256) DEFAULT '' COMMENT 'Comment',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_app_namespace` (`app_id`, `name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo application namespace definition table';

/******************************************/
/*   Table: apollo_cluster                */
/******************************************/
CREATE TABLE `apollo_cluster` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `name` varchar(32) NOT NULL COMMENT 'Cluster name',
    `app_id` varchar(64) NOT NULL COMMENT 'Application ID',
    `parent_cluster_id` bigint(20) DEFAULT 0 COMMENT 'Parent cluster ID',
    `comment` varchar(256) DEFAULT '' COMMENT 'Comment',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_app_cluster` (`app_id`, `name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo cluster table';

/******************************************/
/*   Table: apollo_namespace              */
/******************************************/
CREATE TABLE `apollo_namespace` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `app_id` varchar(64) NOT NULL COMMENT 'Application ID',
    `cluster_name` varchar(32) NOT NULL COMMENT 'Cluster name',
    `namespace_name` varchar(128) NOT NULL COMMENT 'Namespace name',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_app_cluster_namespace` (`app_id`, `cluster_name`, `namespace_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo namespace instance table';

/******************************************/
/*   Table: apollo_item                   */
/******************************************/
CREATE TABLE `apollo_item` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `namespace_id` bigint(20) NOT NULL COMMENT 'Namespace ID (FK to apollo_namespace)',
    `key` varchar(128) NOT NULL DEFAULT '' COMMENT 'Configuration key',
    `type` tinyint(4) DEFAULT 0 COMMENT 'Configuration value type',
    `value` longtext COMMENT 'Configuration value',
    `comment` varchar(256) DEFAULT '' COMMENT 'Comment',
    `line_num` int(11) DEFAULT 0 COMMENT 'Line number for display ordering',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    KEY `idx_namespace_id` (`namespace_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo configuration item table';

/******************************************/
/*   Table: apollo_release                */
/******************************************/
CREATE TABLE `apollo_release` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `release_key` varchar(64) NOT NULL COMMENT 'Release key (unique identifier)',
    `name` varchar(64) DEFAULT '' COMMENT 'Release name',
    `app_id` varchar(64) NOT NULL COMMENT 'Application ID',
    `cluster_name` varchar(32) NOT NULL COMMENT 'Cluster name',
    `namespace_name` varchar(128) NOT NULL COMMENT 'Namespace name',
    `configurations` longtext COMMENT 'Published configuration content (JSON)',
    `comment` varchar(256) DEFAULT '' COMMENT 'Release comment',
    `is_abandoned` boolean DEFAULT FALSE COMMENT 'Whether release is abandoned (rolled back)',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_release_key` (`release_key`),
    KEY `idx_app_cluster_namespace` (`app_id`, `cluster_name`, `namespace_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo configuration release table';

/******************************************/
/*   Table: apollo_release_history        */
/******************************************/
CREATE TABLE `apollo_release_history` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `app_id` varchar(64) NOT NULL COMMENT 'Application ID',
    `cluster_name` varchar(32) NOT NULL COMMENT 'Cluster name',
    `namespace_name` varchar(128) NOT NULL COMMENT 'Namespace name',
    `branch_name` varchar(32) DEFAULT 'default' COMMENT 'Branch name',
    `release_id` bigint(20) DEFAULT 0 COMMENT 'Release ID',
    `previous_release_id` bigint(20) DEFAULT 0 COMMENT 'Previous release ID',
    `operation` tinyint(4) DEFAULT 0 COMMENT 'Operation type (0: normal publish, 1: rollback, etc.)',
    `operation_context` longtext COMMENT 'Operation context (JSON)',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    KEY `idx_app_cluster_namespace` (`app_id`, `cluster_name`, `namespace_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo release history table';

/******************************************/
/*   Table: apollo_release_message        */
/******************************************/
CREATE TABLE `apollo_release_message` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `message` varchar(1024) NOT NULL DEFAULT '' COMMENT 'Release message content (appId+cluster+namespace)',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    KEY `idx_message` (`message`(191))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo release message notification table';

/******************************************/
/*   Table: apollo_gray_release_rule      */
/******************************************/
CREATE TABLE `apollo_gray_release_rule` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `app_id` varchar(64) NOT NULL COMMENT 'Application ID',
    `cluster_name` varchar(32) NOT NULL COMMENT 'Cluster name',
    `namespace_name` varchar(128) NOT NULL COMMENT 'Namespace name',
    `branch_name` varchar(32) NOT NULL COMMENT 'Gray release branch name',
    `rules` text COMMENT 'Gray release rules (JSON)',
    `release_id` bigint(20) DEFAULT 0 COMMENT 'Associated release ID',
    `branch_status` tinyint(4) DEFAULT 0 COMMENT 'Branch status (0: active, 1: merged, 2: abandoned)',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    KEY `idx_app_cluster_namespace` (`app_id`, `cluster_name`, `namespace_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo gray release rule table';

/******************************************/
/*   Table: apollo_namespace_lock         */
/******************************************/
CREATE TABLE `apollo_namespace_lock` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `namespace_id` bigint(20) NOT NULL COMMENT 'Namespace ID (FK to apollo_namespace)',
    `locked_by` varchar(64) DEFAULT '' COMMENT 'User who holds the lock',
    `lock_time` datetime DEFAULT NULL COMMENT 'Lock acquisition time',
    `expire_time` datetime DEFAULT NULL COMMENT 'Lock expiration time',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_namespace_id` (`namespace_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo namespace edit lock table';

/******************************************/
/*   Table: apollo_commit                 */
/******************************************/
CREATE TABLE `apollo_commit` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `change_sets` longtext COMMENT 'Change sets (JSON with create, update, delete items)',
    `app_id` varchar(64) NOT NULL COMMENT 'Application ID',
    `cluster_name` varchar(32) NOT NULL COMMENT 'Cluster name',
    `namespace_name` varchar(128) NOT NULL COMMENT 'Namespace name',
    `comment` varchar(256) DEFAULT '' COMMENT 'Commit comment',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    KEY `idx_app_cluster_namespace` (`app_id`, `cluster_name`, `namespace_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo configuration commit history table';

/******************************************/
/*   Table: apollo_access_key             */
/******************************************/
CREATE TABLE `apollo_access_key` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `app_id` varchar(64) NOT NULL COMMENT 'Application ID',
    `secret` varchar(128) NOT NULL COMMENT 'Secret key',
    `mode` varchar(16) DEFAULT 'STS' COMMENT 'Access mode (STS, etc.)',
    `is_enabled` boolean DEFAULT TRUE COMMENT 'Whether access key is enabled',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    KEY `idx_app_id` (`app_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo access key table';

/******************************************/
/*   Table: apollo_instance               */
/******************************************/
CREATE TABLE `apollo_instance` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `app_id` varchar(64) NOT NULL COMMENT 'Application ID',
    `cluster_name` varchar(32) NOT NULL COMMENT 'Cluster name',
    `data_center` varchar(64) DEFAULT '' COMMENT 'Data center name',
    `ip` varchar(45) NOT NULL COMMENT 'Instance IP address',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    KEY `idx_app_cluster` (`app_id`, `cluster_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo client instance table';

/******************************************/
/*   Table: apollo_instance_config        */
/******************************************/
CREATE TABLE `apollo_instance_config` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `instance_id` bigint(20) NOT NULL COMMENT 'Instance ID (FK to apollo_instance)',
    `config_app_id` varchar(64) DEFAULT NULL COMMENT 'Configuration application ID',
    `config_cluster_name` varchar(32) DEFAULT NULL COMMENT 'Configuration cluster name',
    `config_namespace_name` varchar(128) DEFAULT NULL COMMENT 'Configuration namespace name',
    `release_key` varchar(64) DEFAULT '' COMMENT 'Current release key',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    KEY `idx_instance_id` (`instance_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo instance configuration usage table';

/******************************************/
/*   Table: apollo_audit                  */
/******************************************/
CREATE TABLE `apollo_audit` (
    `id` bigint(20) NOT NULL AUTO_INCREMENT COMMENT 'Primary key ID',
    `entity_name` varchar(64) NOT NULL COMMENT 'Audited entity name (e.g., App, Cluster, Namespace)',
    `entity_id` varchar(64) DEFAULT NULL COMMENT 'Audited entity ID',
    `op_name` varchar(64) NOT NULL COMMENT 'Operation name (e.g., INSERT, UPDATE, DELETE)',
    `comment` varchar(256) DEFAULT '' COMMENT 'Audit comment',
    `is_deleted` boolean NOT NULL DEFAULT FALSE COMMENT 'Soft delete flag',
    `deleted_at` datetime DEFAULT NULL COMMENT 'Deletion timestamp',
    `created_by` varchar(64) DEFAULT '' COMMENT 'Created by user',
    `created_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT 'Creation time',
    `last_modified_by` varchar(64) DEFAULT '' COMMENT 'Last modified by user',
    `last_modified_time` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last modification time',
    PRIMARY KEY (`id`),
    KEY `idx_entity` (`entity_name`, `entity_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin COMMENT='Apollo audit log table';
