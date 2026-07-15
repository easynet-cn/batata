# Apollo插件完整实现计划

基于对原始Apollo项目（~/work/github/easynet-cn/apollo）的分析，本计划旨在完整实现Apollo 2.5.0的核心功能。

## 项目概述

Apollo是一个分布式配置中心，包含三个核心服务：
- **Config Service**：配置读取服务，提供客户端配置拉取、长轮询通知
- **Admin Service**：配置管理服务，提供配置管理接口
- **Portal Service**：Web管理界面（不在本插件范围内）

本插件作为batata的协议适配器，实现Config Service和Admin Service的核心API。

## 当前实现状态

### 已完成的Entity（35个表）
- ✅ apollo_app - 应用
- ✅ apollo_app_namespace - 应用命名空间模板
- ✅ apollo_cluster - 集群
- ✅ apollo_namespace - 命名空间
- ✅ apollo_item - 配置项
- ✅ apollo_release - 发布
- ✅ apollo_commit - 提交记录
- ✅ apollo_release_message - 发布消息
- ✅ apollo_gray_release_rule - 灰度发布规则
- ✅ apollo_instance - 客户端实例
- ✅ apollo_instance_config - 实例配置
- ✅ apollo_namespace_lock - 命名空间锁
- ✅ apollo_audit - 审计日志
- ✅ apollo_server_config - 服务配置
- ✅ apollo_access_key - 访问密钥
- ✅ apollo_release_history - 发布历史
- ✅ apollo_consumer - 开放API消费者
- ✅ apollo_consumer_audit - 消费者审计
- ✅ apollo_consumer_role - 消费者角色
- ✅ apollo_consumer_token - 消费者令牌
- ✅ apollo_favorite - 收藏
- ✅ apollo_permission - 权限
- ✅ apollo_role - 角色
- ✅ apollo_role_permission - 角色权限
- ✅ apollo_user_role - 用户角色
- ✅ apollo_users - 用户

### 已完成的Service层
- ✅ AppService - 应用CRUD
- ✅ ClusterService - 集群CRUD
- ✅ NamespaceService - 命名空间CRUD
- ✅ ItemService - 配置项CRUD
- ✅ ReleaseService - 发布管理（发布、回滚、历史查询、灰度合并到主分支）
- ✅ CommitService - 提交记录CRUD
- ✅ GrayReleaseRuleService - 灰度规则CRUD（含IP匹配：单IP、CIDR、IP范围）
- ✅ InstanceService - 实例注册、心跳、查询
- ✅ InstanceConfigService - 实例配置管理
- ✅ NamespaceLockService - 命名空间锁
- ✅ AccessKeyService - 访问密钥管理
- ✅ AppNamespaceService - 应用命名空间模板
- ✅ ServerConfigService - 服务配置管理
- ✅ ItemSetService - 批量配置项操作（创建/更新/删除，自动创建Commit）
- ✅ AuditService - 审计日志
- ✅ ConsumerService - 开放API消费者
- ✅ ConsumerTokenService - 消费者令牌
- ✅ PermissionService - 权限管理
- ✅ RoleService - 角色管理（角色CRUD、权限分配、用户角色分配）
- ✅ FavoriteService - 收藏管理
- ✅ SearchService - 配置搜索（按应用搜索、跨应用搜索）

### 已完成的路由层

#### Admin Service端点（/admin）
- ✅ POST /apps - 创建应用
- ✅ GET /apps - 列出所有应用
- ✅ GET /apps/{appId} - 获取应用详情
- ✅ PUT /apps/{appId} - 更新应用
- ✅ DELETE /apps/{appId} - 删除应用
- ✅ GET /apps/{appId}/clusters - 列出集群
- ✅ POST /apps/{appId}/clusters - 创建集群
- ✅ GET /apps/{appId}/clusters/{clusterName} - 获取集群
- ✅ DELETE /apps/{appId}/clusters/{clusterName} - 删除集群
- ✅ POST /apps/{appId}/clusters/{clusterName}/namespaces - 创建命名空间
- ✅ GET /apps/{appId}/clusters/{clusterName}/namespaces - 列出命名空间
- ✅ GET /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName} - 获取命名空间
- ✅ PUT /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName} - 更新命名空间
- ✅ DELETE /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName} - 删除命名空间
- ✅ POST /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items - 创建配置项
- ✅ GET /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items - 列出配置项
- ✅ PUT /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items/{itemId} - 更新配置项
- ✅ DELETE /items/{itemId} - 删除配置项
- ✅ POST /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/releases - 发布配置
- ✅ GET /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/commits - 列出提交
- ✅ POST /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/commits - 创建提交
- ✅ GET /commits/{id} - 获取提交详情
- ✅ GET /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/gray-release-rules - 列出灰度规则
- ✅ POST /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/gray-release-rules - 创建灰度规则
- ✅ GET/PUT/DELETE gray-release-rules/{branchName} - 灰度规则管理
- ✅ GET/POST/DELETE /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/lock - 命名空间锁
- ✅ GET/POST/PUT/DELETE /serverconfigs - 服务配置管理
- ✅ GET/POST /apps/{appId}/appnamespaces - 应用命名空间模板管理
- ✅ POST /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/itemset - 批量配置项操作
- ✅ GET /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/releases - 分页查询发布列表
- ✅ POST /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/releases/{releaseId}/rollback - 发布回滚
- ✅ POST /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/updateAndPublish - 灰度发布合并到主分支
- ✅ GET /instance-configs - 查询实例配置
- ✅ GET /appnamespaces - 公共命名空间查询
- ✅ GET /audit - 审计日志查询
- ✅ GET /audit/by-entity - 按实体查询审计日志
- ✅ GET /permissions - 权限查询
- ✅ POST/DELETE /roles - 角色管理
- ✅ GET /users/{userId}/roles - 用户角色查询
- ✅ POST/DELETE /users/{userId}/roles/{roleId} - 用户角色分配
- ✅ POST /consumers - 创建消费者
- ✅ GET /consumers/{appId} - 查询消费者
- ✅ POST/GET /consumers/{consumerId}/tokens - 消费者令牌管理
- ✅ DELETE /consumers/{consumerId}/tokens/{tokenId} - 删除消费者令牌
- ✅ GET /configs/{appId}/{clusterName}/{namespaceName}/export - 导出配置
- ✅ POST /configs/import - 导入配置
- ✅ GET/POST /favorites - 收藏管理
- ✅ DELETE /favorites/{id} - 删除收藏
- ✅ GET /search - 全局搜索

#### Config Service端点（/config）
- ✅ GET /configs/{appId}/{clusterName}/{namespace} - 获取配置（JSON）
- ✅ GET /configfiles/{appId}/{clusterName}/{namespace} - 获取配置文件
- ✅ GET /notifications/v2 - 长轮询通知（V2）
- ✅ GET /notifications - 长轮询通知（V1）
- ✅ POST /instances - 注册实例
- ✅ PUT /instances - 实例心跳

#### OpenAPI端点（/openapi/v1）
- ✅ POST/GET /apps - 应用管理
- ✅ GET /apps/{appId} - 获取应用
- ✅ GET /apps/{appId}/envclusters - 获取环境集群
- ✅ POST/GET /apps/{appId}/appnamespaces - 应用命名空间
- ✅ GET /envs - 列出环境
- ✅ POST /envs/{env}/apps/{appId}/clusters - 创建集群
- ✅ POST/GET /envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces - 命名空间管理
- ✅ GET/POST/PUT/DELETE items端点 - 配置项管理
- ✅ POST/GET releases端点 - 发布管理
- ✅ POST releases/{releaseId}/rollback - 发布回滚
- ✅ GET releases/history - 发布历史
- ✅ GET instances端点 - 实例查询
- ✅ POST/GET/DELETE accesskeys端点 - 访问密钥管理
- ✅ GET/POST/PUT/DELETE branches端点 - 灰度分支管理
- ✅ POST branches/{branchName}/merge - 灰度发布合并到主分支
- ✅ GET/POST commits端点 - 提交管理

## 待完成功能

### Phase 5: 增强功能（优先级：高） - ✅ 已完成

#### 5.1 ItemSet批量操作
- [x] POST /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/itemset - 批量创建/更新/删除配置项
- [x] 自动创建Commit记录
- [x] 参考：`ItemSetController.java`

#### 5.2 Release增强功能
- [x] POST /releases/{releaseId}/rollback - 按releaseId回滚（Admin Service）
- [x] GET /releases - 全局发布查询（分页）
- [x] POST /apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/updateAndPublish - 灰度发布合并到主分支
- [x] POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}/merge - OpenAPI灰度合并
- [x] ReleaseHistory记录（operation=4 GRAY_RELEASE_MERGE_TO_MASTER）

#### 5.3 InstanceConfig管理
- [x] GET /instance-configs - 查询实例配置
- [x] InstanceConfigService实现（create_or_update、get_by_instance、list_by_app_cluster、delete_by_instance）

#### 5.4 AppNamespace增强
- [x] GET /appnamespaces - 公共命名空间查询

### Phase 6: 审计与权限系统（优先级：中） - ✅ 已完成

#### 6.1 Audit审计日志
- [x] GET /audit - 查询审计日志（分页）
- [x] GET /audit/by-entity - 按实体查询审计日志
- [x] AuditService实现（create、list、list_by_entity）

#### 6.2 权限管理
- [x] PermissionService实现（create、list_by_target、list_by_type）
- [x] RoleService实现（角色CRUD、权限分配、用户角色分配）
- [x] 端点：
  - GET /permissions - 查询权限
  - POST/DELETE /roles - 角色管理
  - GET /users/{userId}/roles - 用户角色查询
  - POST/DELETE /users/{userId}/roles/{roleId} - 用户角色分配

#### 6.3 Consumer开放API消费者
- [x] ConsumerService实现（create、get、get_by_app、list）
- [x] ConsumerTokenService实现（create、list_by_consumer、delete、get_by_token）
- [x] 端点：
  - POST /consumers - 创建消费者
  - GET /consumers/{appId} - 查询消费者
  - POST/GET /consumers/{consumerId}/tokens - 令牌管理
  - DELETE /consumers/{consumerId}/tokens/{tokenId} - 删除令牌

### Phase 7: Portal Service端点（优先级：中） - ✅ 已完成

Portal是Web管理界面后端，聚合了Admin Service和配置导出导入功能。

#### 7.1 配置导入导出
- [x] GET /configs/{appId}/{clusterName}/{namespaceName}/export - 导出配置
- [x] POST /configs/import - 导入配置

#### 7.2 收藏功能
- [x] FavoriteService实现（create、list_by_user、delete）
- [x] 端点：
  - GET /favorites - 查询收藏
  - POST /favorites - 添加收藏
  - DELETE /favorites/{id} - 删除收藏

#### 7.3 搜索功能
- [x] GET /search - 全局搜索（按key/value搜索，支持按应用和跨应用）
- [x] SearchService实现（search_items、search_across_apps）

### Phase 8: 高级功能（优先级：低）

#### 8.1 长轮询优化
- [ ] ReleaseMessageService - 发布消息服务增强
- [ ] 支持消息广播
- [ ] 支持消息持久化

#### 8.2 灰度发布增强
- [ ] 支持标签（Label）路由
- [ ] 支持更多规则类型（IP、标签、Header等）
- [ ] 灰度规则优先级管理

#### 8.3 配置加密
- [ ] 配置项加密存储
- [ ] 客户端解密支持

#### 8.4 配置同步
- [ ] GET /configs/sync - 跨环境配置同步
- [ ] POST /configs/sync - 执行同步

## 数据库迁移状态

所有迁移文件使用 `m20260710_` 前缀，共35个迁移文件。

### 迁移文件清单
1. ✅ m20260710_000001_create_apollo_app.rs
2. ✅ m20260710_000002_create_apollo_app_namespace.rs
3. ✅ m20260710_000003_create_apollo_cluster.rs
4. ✅ m20260710_000004_create_apollo_namespace.rs
5. ✅ m20260710_000005_create_apollo_item.rs
6. ✅ m20260710_000006_create_apollo_release.rs
7. ✅ m20260710_000007_create_apollo_commit.rs
8. ✅ m20260710_000008_create_apollo_release_message.rs
9. ✅ m20260710_000009_create_apollo_gray_release_rule.rs
10. ✅ m20260710_000010_create_apollo_instance.rs
11. ✅ m20260710_000011_create_apollo_instance_config.rs
12. ✅ m20260710_000012_create_apollo_namespace_lock.rs
13. ✅ m20260710_000013_create_apollo_audit.rs
14. ✅ m20260710_000014_create_apollo_server_config.rs
15. ✅ m20260710_000015_create_apollo_consumer.rs
16. ✅ m20260710_000016_create_apollo_consumer_audit.rs
17. ✅ m20260710_000017_create_apollo_consumer_role.rs
18. ✅ m20260710_000018_create_apollo_consumer_token.rs
19. ✅ m20260710_000019_create_apollo_favorite.rs
20. ✅ m20260710_000020_create_apollo_permission.rs
21. ✅ m20260710_000021_create_apollo_role.rs
22. ✅ m20260710_000022_create_apollo_role_permission.rs
23. ✅ m20260710_000023_create_apollo_user_role.rs
24. ✅ m20260710_000024_create_apollo_users.rs
25. ✅ m20260710_000025_create_apollo_release_history.rs
26. ✅ m20260710_000026_create_apollo_access_key.rs
27. ✅ m20260710_000027_create_apollo_service_registry.rs
28. ✅ m20260710_000028_create_apollo_audit_log.rs
29. ✅ m20260710_000029_create_apollo_audit_log_data_influence.rs
30. ✅ m20260710_000030_create_apollo_user_token.rs
31. ✅ m20260710_000031_create_apollo_user_token_audit.rs
32. ✅ m20260710_000032_create_apollo_authorities.rs
33. ✅ m20260710_000033_seed_initial_data.rs
34. ✅ m20260710_000034_fix_release_id_type.rs
35. ✅ m20260710_000035_add_namespace_fields.rs

## 实施建议

### 优先级排序
1. **P0（核心功能）**：Phase 5.1 ItemSet批量操作、Phase 5.2 Release增强
2. **P1（重要功能）**：Phase 5.3 InstanceConfig、Phase 5.4 AppNamespace增强
3. **P2（管理功能）**：Phase 6 审计与权限、Phase 7 Portal端点
4. **P3（增强功能）**：Phase 8 高级功能

### 测试策略
1. **单元测试**：每个Service方法需有单元测试
2. **集成测试**：每个端点需有集成测试
3. **SDK兼容性测试**：使用Apollo Java SDK进行端到端测试
4. **性能测试**：长轮询、配置读取性能基准

### 兼容性保证
- OpenAPI 2.5.0兼容性：所有OpenAPI端点需与Apollo官方文档一致
- 客户端SDK兼容：支持Apollo Java SDK 2.x版本
- 数据库兼容：支持MySQL 8.x和PostgreSQL 14+

## 下一步行动

1. **Phase 8.1**: 长轮询优化 - ReleaseMessageService增强、消息广播、消息持久化
2. **Phase 8.2**: 灰度发布增强 - 标签路由、更多规则类型、规则优先级管理
3. **Phase 8.3**: 配置加密 - 配置项加密存储、客户端解密支持
4. **Phase 8.4**: 配置同步 - 跨环境配置同步
5. **集成测试**: 补充灰度发布合并到主分支的端到端测试用例

---

**文档版本**: 1.1
**最后更新**: 2026-07-13
**负责人**: Apollo Plugin Team