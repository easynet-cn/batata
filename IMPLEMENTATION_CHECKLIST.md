# Batata gRPC 功能实现清单

基于 Nacos 3.x 协议分析，以下是需要实现的 gRPC Request/Response Handler 清单。

## 实现状态说明
- [x] 已完成
- [ ] 未实现
- [~] 部分实现

---

## 一、Internal 模块 (api/remote/model.rs)

### 已定义模型，已实现 Handler
| Request | Response | Handler | 状态 |
|---------|----------|---------|------|
| HealthCheckRequest | HealthCheckResponse | HealthCheckHandler | [x] |
| ServerCheckRequest | ServerCheckResponse | ServerCheckHandler | [x] |
| ConnectionSetupRequest | - | ConnectionSetupHandler | [x] |
| ConnectResetRequest | ConnectResetResponse | ConnectResetHandler | [x] |
| ServerLoaderInfoRequest | ServerLoaderInfoResponse | ServerLoaderInfoHandler | [x] |
| ServerReloadRequest | ServerReloadResponse | ServerReloadHandler | [x] |
| ClientDetectionRequest | ClientDetectionResponse | ClientDetectionHandler | [x] |

### 需新增模型和 Handler
| Request | Response | Handler | 状态 |
|---------|----------|---------|------|
| PushAckRequest | - | PushAckHandler | [x] |
| SetupAckRequest | SetupAckResponse | SetupAckHandler | [x] |

---

## 二、Config 模块 (api/config/model.rs)

### 已定义模型，已实现 Handler
| Request | Response | Handler | 状态 | 优先级 |
|---------|----------|---------|------|--------|
| ConfigQueryRequest | ConfigQueryResponse | ConfigQueryHandler | [x] | P0 |
| ConfigPublishRequest | ConfigPublishResponse | ConfigPublishHandler | [x] | P0 |
| ConfigRemoveRequest | ConfigRemoveResponse | ConfigRemoveHandler | [x] | P0 |
| ConfigBatchListenRequest | ConfigChangeBatchListenResponse | ConfigBatchListenHandler | [x] | P0 |

### 已定义模型，已实现 Handler (P1/P2)
| Request | Response | Handler | 状态 | 优先级 |
|---------|----------|---------|------|--------|
| ConfigChangeNotifyRequest | ConfigChangeNotifyResponse | ConfigChangeNotifyHandler | [x] | P1 |
| ConfigFuzzyWatchRequest | ConfigFuzzyWatchResponse | ConfigFuzzyWatchHandler | [x] | P2 |
| ConfigFuzzyWatchChangeNotifyRequest | ConfigFuzzyWatchChangeNotifyResponse | ConfigFuzzyWatchChangeNotifyHandler | [x] | P2 |
| ConfigFuzzyWatchSyncRequest | ConfigFuzzyWatchSyncResponse | ConfigFuzzyWatchSyncHandler | [x] | P2 |
| ClientConfigMetricRequest | ClientConfigMetricResponse | ClientConfigMetricHandler | [x] | P2 |
| ConfigChangeClusterSyncRequest | ConfigChangeClusterSyncResponse | ConfigChangeClusterSyncHandler | [x] | P1 |

---

## 三、Naming 模块 (api/naming/model.rs)

### 已定义模型，已实现 Handler
| Request | Response | Handler | 状态 | 优先级 |
|---------|----------|---------|------|--------|
| InstanceRequest | InstanceResponse | InstanceRequestHandler | [x] | P0 |
| BatchInstanceRequest | BatchInstanceResponse | BatchInstanceRequestHandler | [x] | P1 |
| ServiceListRequest | ServiceListResponse | ServiceListRequestHandler | [x] | P0 |
| ServiceQueryRequest | QueryServiceResponse | ServiceQueryRequestHandler | [x] | P0 |
| SubscribeServiceRequest | SubscribeServiceResponse | SubscribeServiceRequestHandler | [x] | P0 |

### 已定义模型，已实现 Handler (P1/P2)
| Request | Response | Handler | 状态 | 优先级 |
|---------|----------|---------|------|--------|
| PersistentInstanceRequest | InstanceResponse | PersistentInstanceRequestHandler | [x] | P1 |
| NotifySubscriberRequest | NotifySubscriberResponse | NotifySubscriberHandler | [x] | P1 |
| NamingFuzzyWatchRequest | NamingFuzzyWatchResponse | NamingFuzzyWatchHandler | [x] | P2 |
| NamingFuzzyWatchChangeNotifyRequest | NamingFuzzyWatchChangeNotifyResponse | NamingFuzzyWatchChangeNotifyHandler | [x] | P2 |
| NamingFuzzyWatchSyncRequest | NamingFuzzyWatchSyncResponse | NamingFuzzyWatchSyncHandler | [x] | P2 |

---

## 四、实现进度总结

### ✅ 全部完成

**Internal 模块 (9/9):**
- [x] HealthCheckHandler - 健康检查
- [x] ServerCheckHandler - 服务器检查
- [x] ConnectionSetupHandler - 连接建立
- [x] ClientDetectionHandler - 客户端检测
- [x] ServerLoaderInfoHandler - 服务器负载信息
- [x] ServerReloadHandler - 服务器配置重载
- [x] ConnectResetHandler - 连接重置
- [x] PushAckHandler - 推送确认
- [x] SetupAckHandler - 连接确认

**Config 模块 (10/10):**
- [x] ConfigQueryHandler - 查询配置
- [x] ConfigPublishHandler - 发布配置
- [x] ConfigRemoveHandler - 删除配置
- [x] ConfigBatchListenHandler - 批量监听配置变更
- [x] ConfigChangeNotifyHandler - 配置变更通知
- [x] ConfigChangeClusterSyncHandler - 配置变更集群同步
- [x] ConfigFuzzyWatchHandler - 模糊监听
- [x] ConfigFuzzyWatchChangeNotifyHandler - 模糊监听变更通知
- [x] ConfigFuzzyWatchSyncHandler - 模糊监听同步
- [x] ClientConfigMetricHandler - 配置指标

**Naming 模块 (10/10):**
- [x] InstanceRequestHandler - 服务实例注册/注销
- [x] BatchInstanceRequestHandler - 批量实例操作
- [x] ServiceListRequestHandler - 服务列表查询
- [x] ServiceQueryRequestHandler - 服务详情查询
- [x] SubscribeServiceRequestHandler - 服务订阅
- [x] PersistentInstanceRequestHandler - 持久化实例
- [x] NotifySubscriberHandler - 订阅者通知
- [x] NamingFuzzyWatchHandler - 命名模糊监听
- [x] NamingFuzzyWatchChangeNotifyHandler - 命名模糊监听变更通知
- [x] NamingFuzzyWatchSyncHandler - 命名模糊监听同步

### 总计: 29/29 Handlers 已实现

---

## 五、新增文件列表

```
src/
├── api/
│   └── naming/
│       └── model.rs          # [NEW] Naming 模块请求/响应定义
├── service/
│   ├── config_handler.rs     # [NEW] Config gRPC handlers
│   ├── naming_handler.rs     # [NEW] Naming gRPC handlers
│   └── naming.rs             # [NEW] Naming 业务服务层 (内存服务注册表)
```

---

## 六、架构说明

### Handler 注册流程 (main.rs)
1. 创建 `HandlerRegistry`
2. 创建各模块 Handler 实例（携带 `Arc<AppState>` 或 `Arc<NamingService>`）
3. 注册 Handler 到 Registry
4. 创建 `GrpcRequestService` 和 `GrpcBiRequestStreamService`
5. 启动 gRPC Server

### Naming 服务实现
当前采用内存实现 (`NamingService`)：
- 使用 `DashMap` 存储服务实例
- 支持服务注册/注销/查询/订阅
- 订阅者管理按 connection_id 追踪

### Config 服务实现
使用数据库存储 (`service::config`)：
- 通过 SeaORM 访问 MySQL
- 支持配置的 CRUD 操作
- 配置变更历史记录

---

## 参考资料

- [Nacos GitHub - Naming Request](https://github.com/alibaba/nacos/tree/develop/api/src/main/java/com/alibaba/nacos/api/naming/remote/request)
- [Nacos GitHub - Config Request](https://github.com/alibaba/nacos/tree/develop/api/src/main/java/com/alibaba/nacos/api/config/remote/request)
- [Nacos 2.0 gRPC 架构](https://www.alibabacloud.com/blog/an-in-depth-insight-into-nacos-2-0-architecture-and-new-model-with-a-supported-grpc-persistent-connection_597804)
