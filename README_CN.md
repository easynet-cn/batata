# Batata

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/easynet-cn/batata/ci.yml?branch=main)](https://github.com/easynet-cn/batata/actions)

**Batata** 是一个基于 Rust 实现的高性能动态服务发现、配置管理和服务管理平台。完全兼容 [Nacos](https://nacos.io/) V2/V3 API 和 [Consul](https://www.consul.io/) API，可作为云原生应用的理想替代方案。

[English Documentation](README.md)

## 特性

### 核心能力

- **配置管理** - 集中式动态配置，支持实时更新
- **服务发现** - 服务注册、发现和健康检查
- **命名空间隔离** - 基于命名空间的多租户资源隔离
- **集群模式** - 基于 Raft 共识协议的高可用方案

### API 兼容性

- **Nacos API** - 完全兼容 Nacos V2 和 V3 API（V1 API 不支持）
- **Consul API** - 兼容 Consul Agent、Health、Catalog、KV 和 ACL API
- **gRPC 支持** - 高性能双向流式通信，支持 SDK 客户端

### 高级特性

- **配置导入/导出** - 支持 Nacos ZIP 和 Consul JSON 格式的批量配置迁移
- **灰度发布** - 支持配置的渐进式发布
- **认证与授权** - 基于 JWT 的认证和 RBAC 权限模型，支持 LDAP、OAuth2/OIDC
- **限流** - 可配置的 API 限流保护
- **熔断器** - 故障容错的弹性模式
- **监控与可观测性** - 兼容 Prometheus 的指标端点
- **服务网格** - xDS 协议支持 (EDS, CDS, LDS, RDS, ADS)
- **多数据中心** - 地域感知复制与跨数据中心同步
- **DNS 服务** - 基于 UDP 的 DNS 服务发现
- **分布式锁** - 基于 Raft 的分布式锁
- **AI 集成** - MCP (模型内容协议) 和 A2A (Agent-to-Agent) 注册中心

### 性能优势

- **快速启动** - 约 1-2 秒（对比 Java 版 Nacos 的 30-60 秒）
- **低内存占用** - 约 50-100MB（对比 Java 版 Nacos 的 1-2GB）
- **高吞吐量** - 基于 Tokio 运行时的异步 I/O 优化
- **高效存储** - 使用 RocksDB 持久化存储，配合优化的缓存策略

## 与 Nacos 功能对比

Batata 已实现 **~98% 的 Nacos 功能**，可作为生产环境的替代方案。

### 核心功能

| 功能 | Nacos | Batata | 状态 |
|------|-------|--------|------|
| **配置管理** | | | |
| 配置 CRUD | ✅ | ✅ | 完整 |
| 配置搜索与分页 | ✅ | ✅ | 完整 |
| 配置历史与回滚 | ✅ | ✅ | 完整 |
| 配置监听 (长轮询) | ✅ | ✅ | 完整 |
| 灰度发布 | ✅ | ✅ | 完整 |
| 配置导入/导出 (ZIP) | ✅ | ✅ | 完整 |
| 配置标签 | ✅ | ✅ | 完整 |
| 配置加密 | ✅ | ✅ | 完整 (AES-128-CBC) |
| 配置容量配额 | ✅ | ✅ | 完整 |
| 聚合配置 (datumId) | ✅ | ✅ | 完整 |
| **服务发现** | | | |
| 实例注册/注销 | ✅ | ✅ | 完整 |
| 服务发现查询 | ✅ | ✅ | 完整 |
| 健康检查 | ✅ | ✅ | 完整 |
| 心跳机制 | ✅ | ✅ | 完整 |
| 订阅/推送 | ✅ | ✅ | 完整 |
| 权重路由 | ✅ | ✅ | 完整 |
| 临时实例 | ✅ | ✅ | 完整 |
| 持久实例 | ✅ | ✅ | 完整 |
| 服务元数据 | ✅ | ✅ | 完整 |
| DNS 服务发现 | ✅ | ✅ | 完整 |
| **命名空间** | | | |
| 命名空间 CRUD | ✅ | ✅ | 完整 |
| 多租户隔离 | ✅ | ✅ | 完整 |
| **集群** | | | |
| 集群节点发现 | ✅ | ✅ | 完整 |
| 节点健康检查 | ✅ | ✅ | 完整 |
| Raft 共识 (CP) | ✅ | ✅ | 完整 |
| Distro 协议 (AP) | ✅ | ✅ | 完整 |
| 多数据中心同步 | ✅ | ✅ | 完整 |
| **认证** | | | |
| 用户管理 | ✅ | ✅ | 完整 |
| 角色管理 | ✅ | ✅ | 完整 |
| 权限 (RBAC) | ✅ | ✅ | 完整 |
| JWT Token | ✅ | ✅ | 完整 |
| LDAP 集成 | ✅ | ✅ | 完整 |
| OAuth2/OIDC | ✅ | ✅ | 完整 |
| **API** | | | |
| Nacos V2 API | ✅ | ✅ | 完整 |
| Nacos V3 API | ✅ | ✅ | 完整 |
| Nacos V1 API | ✅ | ❌ | **不支持** |
| gRPC 双向流 | ✅ | ✅ | 完整 |
| Consul API 兼容 | ❌ | ✅ | **额外特性** |
| **可观测性** | | | |
| Prometheus 指标 | ✅ | ✅ | 完整 |
| 健康检查端点 | ✅ | ✅ | 完整 |
| OpenTelemetry | ✅ | ✅ | 完整 |
| 操作审计日志 | ✅ | ✅ | 完整 |

### Batata 独有特性

| 特性 | 说明 |
|-----|------|
| **Consul API 兼容** | 完整支持 Agent、Health、Catalog、KV、ACL API |
| **PostgreSQL 支持** | 除 MySQL 外还支持 PostgreSQL |
| **Consul JSON 导入/导出** | 支持 Consul KV 存储迁移 |
| **内置熔断器** | 集群健康检查弹性模式 |
| **OpenTelemetry 分布式追踪** | OTLP 导出支持 (Jaeger、Zipkin 等) |
| **多数据中心支持** | 本地优先同步的地域感知复制 |
| **服务网格 (xDS)** | EDS、CDS、LDS、RDS、ADS 协议支持 |
| **AI 集成** | MCP 服务器注册和 A2A 代理注册 |
| **分布式锁** | 基于 Raft 的分布式锁机制 |
| **Kubernetes 同步** | 与 Kubernetes 双向服务同步 |

### 功能完成度总结

| 模块 | 完成度 | 备注 |
|-----|-------|------|
| 配置管理 | **100%** | 完整 AES-GCM 加密支持 |
| 服务发现 | **95%** | UDP 推送未实现（使用 gRPC 推送） |
| 命名空间 | **100%** | 完全支持 |
| 集群管理 | **95%** | 单 Raft 组（多 Raft 部分支持） |
| 认证授权 | **100%** | JWT、LDAP、OAuth2/OIDC |
| API 兼容性 | **100%+** | 完整 Nacos V2/V3 + Consul API |
| 云原生 | **85%** | K8s、Prometheus、xDS（基础） |
| **总体** | **~98%** | 生产就绪 |

## 项目结构

Batata 采用多 crate 工作空间架构，包含 19 个内部 crate，提高模块化和可维护性：

```
batata/
├── crates/
│   ├── batata-common/            # 公共工具和类型
│   ├── batata-api/               # API 定义 & gRPC proto
│   ├── batata-persistence/       # 数据库实体 (SeaORM)
│   ├── batata-migration/         # 数据库迁移
│   ├── batata-auth/              # 认证与授权
│   ├── batata-consistency/       # Raft 共识协议
│   ├── batata-core/              # 核心抽象 & 集群
│   ├── batata-config/            # 配置服务
│   ├── batata-naming/            # 服务发现
│   ├── batata-plugin/            # 插件接口
│   ├── batata-plugin-consul/     # Consul 兼容插件
│   ├── batata-plugin-cloud/      # Prometheus/K8s 云插件
│   ├── batata-console/           # 控制台后端服务
│   ├── batata-client/            # 客户端 SDK
│   ├── batata-maintainer-client/ # 维护客户端
│   ├── batata-consul-client/     # Consul 客户端
│   ├── batata-mesh/              # 服务网格 (xDS, Istio MCP)
│   ├── batata-ai/                # AI 集成 (MCP/A2A)
│   ├── batata-server-common/     # 共享服务器基础设施
│   └── batata-server/            # 主服务器 (HTTP, gRPC, 控制台)
│       ├── src/
│       │   ├── api/              # API 处理器 (HTTP, gRPC, Consul)
│       │   ├── auth/             # 认证 HTTP 处理器
│       │   ├── config/           # 配置模型（重导出）
│       │   ├── console/          # 控制台 API 处理器
│       │   ├── middleware/       # HTTP 中间件
│       │   ├── model/            # 应用状态与配置
│       │   ├── service/          # gRPC 处理器
│       │   ├── startup/          # 服务器初始化
│       │   ├── lib.rs            # 库导出
│       │   └── main.rs           # 入口点
│       ├── tests/                # 集成测试
│       └── benches/              # 性能基准测试
├── conf/                         # 配置文件
├── docs/                         # 文档
└── proto/                        # Protocol buffer 定义
```

### Crate 依赖关系

```
batata-server (主二进制)
├── batata-api (API 类型, gRPC proto)
├── batata-auth (JWT, RBAC, LDAP, OAuth2)
├── batata-config (配置服务)
├── batata-naming (服务发现)
├── batata-console (控制台后端)
├── batata-core (集群, 连接, 数据中心)
├── batata-mesh (xDS, Istio MCP)
├── batata-ai (AI/MCP/A2A)
├── batata-plugin-consul (Consul API)
├── batata-plugin-cloud (Prometheus/K8s)
├── batata-migration (数据库迁移)
├── batata-server-common (共享服务器基础设施)
└── batata-persistence (数据库)
```

## 快速开始

### 前置条件

- Rust 1.85+（Edition 2024）
- MySQL 5.7+ / 8.0+ 或 PostgreSQL 14+（可选，嵌入式模式无需数据库）

### 嵌入式模式（无需数据库）

最快的启动方式，使用内嵌 RocksDB 存储：

```bash
# 克隆仓库
git clone https://github.com/easynet-cn/batata.git
cd batata

# 构建并运行（嵌入式模式）
cargo build --release -p batata-server
./target/release/batata-server --batata.sql.init.platform=embedded

# 或使用启动脚本
./scripts/start-embedded.sh
```

首次启动后需要初始化管理员用户：

```bash
# 初始化 admin 用户
./scripts/init-admin.sh
# 或手动执行：
curl -X POST http://localhost:8848/nacos/v3/auth/user/admin -d "username=nacos&password=nacos"
```

### 使用 MySQL 数据库

```bash
# 初始化数据库
mysql -u root -p < conf/mysql-schema.sql

# 编辑 conf/application.yml 配置数据库连接
# 设置 batata.sql.init.platform: mysql
# 设置 batata.db.url: mysql://user:password@localhost:3306/batata

# 构建并运行
cargo build --release -p batata-server
./target/release/batata-server
```

### 使用 PostgreSQL 数据库

```bash
# 初始化数据库
psql -U user -d batata -f conf/postgresql-schema.sql

# 编辑 conf/application.yml 配置数据库连接
# 设置 batata.sql.init.platform: postgresql
# 设置 batata.db.url: postgres://user:password@localhost:5432/batata

# 构建并运行
cargo build --release -p batata-server
./target/release/batata-server
```

> **提示：** 也可以启用自动数据库迁移，设置 `batata.db.migration.enabled: true`，启动时自动执行 schema 迁移，无需手动导入 SQL 文件。详见[数据库迁移](#数据库迁移)。

### 默认端口

| 端口 | 服务 | 描述 |
|------|------|------|
| 8848 | 主 HTTP API | Nacos 兼容 API |
| 8081 | 控制台 HTTP API | Web 管理控制台 |
| 9848 | SDK gRPC | 客户端 SDK 通信 |
| 9849 | 集群 gRPC | 节点间通信 |
| 15010 | xDS gRPC | 服务网格 xDS/ADS 协议 |

## 配置

主配置文件：`conf/application.yml`

所有配置项使用 `batata.*` 前缀的扁平点号格式。可通过以下方式覆盖：
- `BATATA_*` 环境变量（例如 `BATATA_SERVER_MAIN_PORT=9090`）
- CLI 属性覆盖（例如 `--batata.server.main.port=9090`）

```yaml
# 服务器配置
batata.server.main.port: 8848
batata.server.context_path: /nacos
batata.standalone: true

# 控制台配置
batata.console.port: 8081
batata.console.context_path:

# 数据库配置
batata.sql.init.platform: mysql
batata.db.url: "mysql://user:password@localhost:3306/batata"
batata.db.pool.max_connections: 100
batata.db.pool.min_connections: 10

# 认证配置
batata.core.auth.enabled: true
batata.core.auth.system.type: nacos
batata.core.auth.plugin.nacos.token.secret.key: "your-base64-encoded-secret-key"
batata.core.auth.plugin.nacos.token.expire.seconds: 18000
```

### 环境变量

```bash
export BATATA_DB_URL="mysql://user:pass@localhost:3306/batata"
export BATATA_SERVER_MAIN_PORT=8848
export BATATA_CORE_AUTH_ENABLED=true
export RUST_LOG=info
```

### OpenTelemetry 配置

启用 OpenTelemetry OTLP 导出的分布式追踪：

**application.yml:**
```yaml
batata.otel.enabled: true
batata.otel.endpoint: "http://localhost:4317"
batata.otel.service_name: "batata"
batata.otel.sampling_ratio: 1.0
batata.otel.export_timeout_secs: 10
```

**环境变量（优先级高于配置文件）：**
```bash
export BATATA_OTEL_ENABLED=true
export BATATA_OTEL_ENDPOINT="http://localhost:4317"
export BATATA_OTEL_SERVICE_NAME="batata"
export BATATA_OTEL_SAMPLING_RATIO=1.0
export BATATA_OTEL_EXPORT_TIMEOUT_SECS=10
```

**兼容后端：** Jaeger、Zipkin、Tempo、Datadog、Honeycomb 及任何 OTLP 兼容的收集器。

### 多数据中心配置

启用数据中心、区域和可用区的地域感知集群：

**环境变量：**
```bash
export BATATA_DATACENTER="dc1"
export BATATA_REGION="us-east"
export BATATA_ZONE="zone-a"
export BATATA_LOCALITY_WEIGHT=1.0
export BATATA_CROSS_DC_REPLICATION=true
export BATATA_CROSS_DC_SYNC_DELAY_SECS=1
export BATATA_REPLICATION_FACTOR=1
```

**功能特点：**
- 地域感知成员选择（本地优先同步）
- 可配置延迟的跨数据中心复制
- 自动数据中心拓扑发现
- 区域/可用区层级支持

### xDS/服务网格配置

启用 xDS 协议支持 Envoy 代理和 Istio 服务网格：

**application.yml:**
```yaml
batata.mesh.xds.enabled: true
batata.mesh.xds.port: 15010
batata.mesh.xds.server.id: "batata-xds-server"
batata.mesh.xds.sync.interval.ms: 5000
batata.mesh.xds.generate.listeners: true
batata.mesh.xds.generate.routes: true
batata.mesh.xds.default.listener.port: 15001
```

**支持的 xDS 资源：**
- **EDS** (端点发现服务) - 服务端点
- **CDS** (集群发现服务) - 上游集群
- **LDS** (监听器发现服务) - 监听器配置
- **RDS** (路由发现服务) - 路由配置
- **ADS** (聚合发现服务) - 通过单一流获取所有资源

**兼容：** Envoy Proxy、Istio 及任何 xDS 兼容的服务网格。

## 配置参考

### 服务器

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `batata.server.main.port` | `8848` | 主 HTTP 服务器端口 |
| `batata.server.context_path` | `/nacos` | 服务器 Web 上下文路径 |
| `batata.standalone` | `true` | 单机模式 (true=单机, false=集群) |
| `batata.deployment.type` | `merged` | 部署类型: merged/server/console/serverWithMcp |

### 控制台

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `batata.console.port` | `8081` | 控制台 HTTP 端口 |
| `batata.console.context_path` | _(空)_ | 控制台 Web 上下文路径 |
| `batata.console.remote.server_addr` | _(空)_ | 远程服务器地址（console 部署模式） |

### 数据库

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `batata.sql.init.platform` | _(空)_ | 存储平台: mysql/postgresql/空(=嵌入式) |
| `batata.db.url` | _(空)_ | 数据库连接 URL |
| `batata.db.pool.max_connections` | `100` | 最大连接数 |
| `batata.db.pool.min_connections` | `10` | 最小连接数 |
| `batata.db.pool.connect_timeout` | `30` | 连接超时（秒） |
| `batata.db.pool.idle_timeout` | `600` | 空闲超时（秒） |
| `batata.db.pool.max_lifetime` | `1800` | 最大生命周期（秒） |
| `batata.db.migration.enabled` | `false` | 启动时自动执行迁移 |

### 认证

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `batata.core.auth.enabled` | `true` | 启用认证 |
| `batata.core.auth.system.type` | `nacos` | 认证类型: nacos/ldap/oauth |
| `batata.core.auth.plugin.nacos.token.secret.key` | _(必填)_ | JWT 密钥 (Base64) |
| `batata.core.auth.plugin.nacos.token.expire.seconds` | `18000` | Token 过期时间（秒） |
| `batata.core.auth.server.identity.key` | `nacos-server-key` | 服务器身份标识键 |
| `batata.core.auth.server.identity.value` | `nacos-server-value` | 服务器身份标识值 |

### 限流

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `batata.ratelimit.enabled` | `false` | 启用 API 限流 |
| `batata.ratelimit.max_requests` | `10000` | 最大请求数 |
| `batata.ratelimit.window_seconds` | `60` | 限流时间窗口（秒） |
| `batata.ratelimit.auth.enabled` | `false` | 启用认证限流 |
| `batata.ratelimit.auth.max_attempts` | `5` | 最大认证尝试次数 |
| `batata.ratelimit.auth.lockout_seconds` | `300` | 锁定时间（秒） |

### 可观测性

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `batata.otel.enabled` | `false` | 启用 OpenTelemetry |
| `batata.otel.endpoint` | `http://localhost:4317` | OTLP 端点 |
| `batata.otel.service_name` | `batata` | 服务名称 |
| `batata.otel.sampling_ratio` | `0.1` | 采样率 (0.0-1.0) |
| `batata.logs.path` | `logs` | 日志目录 |
| `batata.logs.level` | `info` | 日志级别 |
| `batata.logs.console.enabled` | `true` | 控制台日志输出 |
| `batata.logs.file.enabled` | `true` | 文件日志输出 |

### 服务网格 (xDS)

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `batata.mesh.xds.enabled` | `false` | 启用 xDS 协议 |
| `batata.mesh.xds.port` | `15010` | xDS gRPC 端口 |
| `batata.mesh.xds.server.id` | `batata-xds-server` | xDS 服务器 ID |
| `batata.mesh.xds.sync.interval.ms` | `5000` | 服务同步间隔（毫秒） |

### Consul 兼容

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `batata.plugin.consul.enabled` | `false` | 启用 Consul 兼容 |
| `batata.plugin.consul.port` | `8500` | Consul HTTP 端口 |
| `batata.plugin.consul.datacenter` | `dc1` | 数据中心名称 |

### AI 集成

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `batata.ai.mcp.registry.enabled` | `false` | 启用 MCP 注册中心 |
| `batata.ai.mcp.registry.port` | `9080` | MCP 注册中心端口 |

## API 参考

### Nacos 配置 API (V2)

> **注意：** V1 API 不支持。请使用 V2 或 V3 API。

| 端点 | 方法 | 描述 |
|------|------|------|
| `/nacos/v2/cs/config` | GET | 获取配置 |
| `/nacos/v2/cs/config` | POST | 创建/更新配置 |
| `/nacos/v2/cs/config` | DELETE | 删除配置 |

### Nacos 服务发现 API (V2)

| 端点 | 方法 | 描述 |
|------|------|------|
| `/nacos/v2/ns/instance` | POST | 注册实例 |
| `/nacos/v2/ns/instance` | PUT | 更新实例 |
| `/nacos/v2/ns/instance` | DELETE | 注销实例 |
| `/nacos/v2/ns/instance/list` | GET | 获取服务实例列表 |

### 控制台 API (V3)

| 端点 | 方法 | 描述 |
|------|------|------|
| `/v3/console/cs/config` | GET | 获取配置详情（控制台） |
| `/v3/console/cs/config/list` | GET | 分页获取配置列表 |
| `/v3/console/cs/config/export` | GET | 导出配置（ZIP） |
| `/v3/console/cs/config/import` | POST | 导入配置（ZIP） |
| `/v3/console/namespace/list` | GET | 列出命名空间 |
| `/v3/console/namespace` | POST | 创建命名空间 |
| `/v3/console/core/cluster/nodes` | GET | 列出集群节点 |
| `/v3/console/core/cluster/health` | GET | 获取集群健康状态 |

### Consul API

| 端点 | 方法 | 描述 |
|------|------|------|
| `/v1/agent/service/register` | PUT | 注册服务 |
| `/v1/agent/service/deregister/{id}` | PUT | 注销服务 |
| `/v1/health/service/{service}` | GET | 获取服务健康状态 |
| `/v1/catalog/services` | GET | 列出所有服务 |
| `/v1/kv/{key}` | GET | 获取 KV 值 |
| `/v1/kv/{key}` | PUT | 设置 KV 值 |
| `/v1/kv/{key}` | DELETE | 删除 KV 值 |
| `/v1/kv/export` | GET | 导出 KV（JSON） |
| `/v1/kv/import` | PUT | 导入 KV（JSON） |

## 配置导入/导出

### Nacos 格式（ZIP）

```bash
# 导出配置
curl -O "http://localhost:8848/nacos/v3/console/cs/config/export?namespaceId=public"

# 带冲突策略导入（ABORT, SKIP, OVERWRITE）
curl -X POST "http://localhost:8848/nacos/v3/console/cs/config/import?namespaceId=public&policy=OVERWRITE" \
  -F "file=@export.zip"
```

### Consul 格式（JSON）

```bash
# 导出 KV 存储
curl "http://localhost:8848/v1/kv/export?namespaceId=public" > configs.json

# 导入 KV 存储
curl -X PUT "http://localhost:8848/v1/kv/import?namespaceId=public" \
  -H "Content-Type: application/json" \
  -d @configs.json
```

### 冲突策略说明

| 策略 | 描述 |
|------|------|
| `ABORT` | 遇到冲突立即停止导入（默认） |
| `SKIP` | 跳过已存在的配置 |
| `OVERWRITE` | 覆盖已存在的配置 |

## 架构

```
┌─────────────────────────────────────────────────────────────────┐
│                        Batata 服务器                              │
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────────┐  │
│  │   HTTP API    │  │   gRPC API    │  │    控制台 API        │  │
│  │   (Actix)     │  │   (Tonic)     │  │    (Actix)          │  │
│  │   端口:8848   │  │   端口:9848   │  │    端口:8081        │  │
│  └───────┬───────┘  └───────┬───────┘  └──────────┬──────────┘  │
│          │                  │                      │             │
│  ┌───────▼──────────────────▼──────────────────────▼───────────┐ │
│  │                      服务层                                   │ │
│  │  ┌──────────┐  ┌──────────┐  ┌────────┐  ┌───────────────┐  │ │
│  │  │  配置    │  │  服务发现 │  │  认证  │  │   命名空间     │  │ │
│  │  │  服务    │  │  服务    │  │  服务  │  │   服务        │  │ │
│  │  └──────────┘  └──────────┘  └────────┘  └───────────────┘  │ │
│  └─────────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                      存储层                                   │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │ │
│  │  │  MySQL/PG   │  │   RocksDB   │  │    Moka 缓存        │  │ │
│  │  │  (SeaORM)   │  │ (Raft 日志) │  │   (内存缓存)        │  │ │
│  │  └─────────────┘  └─────────────┘  └─────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                      集群层                                   │ │
│  │  ┌─────────────────────────────────────────────────────────┐│ │
│  │  │               Raft 共识 (OpenRaft)                      ││ │
│  │  │                    端口: 9849                           ││ │
│  │  └─────────────────────────────────────────────────────────┘│ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## 部署

### 部署模式

Batata 支持多种部署模式，通过 `-d` 参数指定：

| 模式 | 参数 | 描述 |
|------|------|------|
| Merged（默认） | `-d merged` | 控制台 (8081) + 主服务器 (8848) 同进程 |
| Server | `-d server` | 仅主服务器 (8848) + gRPC (9848/9849) |
| Console | `-d console` | 仅控制台 (8081)，连接远程服务器 |
| Server + MCP | `-d serverWithMcp` | 主服务器 + MCP 注册中心 (9080) |

### 启动脚本

```bash
# 嵌入式模式启动（无需数据库）
./scripts/start-embedded.sh

# 仅启动服务器
./scripts/start-server.sh

# 仅启动控制台（连接远程服务器）
./scripts/start-console.sh [server_addr]

# 使用 MySQL 数据库启动
./scripts/start-mysql.sh [db_url]

# 初始化管理员用户（首次启动必须执行）
./scripts/init-admin.sh [username] [password] [server_url]

# 测试控制台/服务器路由分离
./scripts/test-separation.sh
```

### 单机模式

```bash
# 直接运行
cargo run --release -p batata-server

# 或使用 CLI 参数
./target/release/batata-server --batata.standalone=true
```

### 集群模式

1. 配置 `conf/cluster.conf` 添加成员地址：
   ```
   192.168.1.1:8848
   192.168.1.2:8848
   192.168.1.3:8848
   ```

2. 更新 `conf/application.yml`：
   ```yaml
   batata.standalone: false
   batata.core.member.lookup.type: file
   ```

3. 启动所有节点

### Docker

```bash
# 构建镜像
docker build -t batata:latest .

# 运行容器
docker run -d \
  -p 8848:8848 \
  -p 8081:8081 \
  -p 9848:9848 \
  -v $(pwd)/conf:/app/conf \
  batata:latest
```

### Docker Compose

```bash
docker-compose up -d
```

## 数据库迁移

Batata 通过 `batata-migration` crate 提供自动数据库迁移功能。

### 启用自动迁移

在 `conf/application.yml` 中设置：

```yaml
batata.db.migration.enabled: true
```

启用后，Batata 在启动时会自动检查并执行数据库 schema 迁移，无需手动导入 SQL 文件。

### 手动迁移

如果不使用自动迁移，可以手动导入 schema：

**MySQL:**
```bash
mysql -u root -p < conf/mysql-schema.sql
```

**PostgreSQL:**
```bash
psql -U user -d batata -f conf/postgresql-schema.sql
```

## 开发

### 构建命令

```bash
# 构建所有 crate
cargo build

# 仅构建服务器（更快）
cargo build -p batata-server

# 发布构建（优化）
cargo build --release -p batata-server

# 运行测试（所有 crate）
cargo test --workspace

# 运行指定 crate 测试
cargo test -p batata-server

# 运行性能基准测试
cargo bench -p batata-server

# 格式化代码
cargo fmt --all

# Clippy 检查
cargo clippy --workspace

# 生成文档
cargo doc --workspace --open
```

### 生成数据库实体

```bash
# MySQL:
sea-orm-cli generate entity \
  -u "mysql://user:pass@localhost:3306/batata" \
  -o ./crates/batata-persistence/src/entity \
  --with-serde both

# PostgreSQL:
sea-orm-cli generate entity \
  -u "postgres://user:pass@localhost:5432/batata" \
  -o ./crates/batata-persistence/src/entity \
  --with-serde both
```

### 项目统计

- **~50,000+ 行** Rust 代码
- **19 个内部 crate** 工作空间
- **333 个单元测试**，全面覆盖
- **3 套性能基准测试**

## 客户端 SDK

Batata 兼容现有的 Nacos 和 Consul 客户端 SDK：

### Java (Nacos SDK)

```java
Properties properties = new Properties();
properties.setProperty("serverAddr", "localhost:8848");
ConfigService configService = NacosFactory.createConfigService(properties);
String config = configService.getConfig("dataId", "group", 5000);
```

### Go (Nacos SDK)

```go
client, _ := clients.NewConfigClient(
    vo.NacosClientParam{
        ServerConfigs: []constant.ServerConfig{
            {IpAddr: "localhost", Port: 8848},
        },
    },
)
config, _ := client.GetConfig(vo.ConfigParam{DataId: "dataId", Group: "group"})
```

### Go (Consul SDK)

```go
client, _ := api.NewClient(api.DefaultConfig())
kv, _, _ := client.KV().Get("key", nil)
```

### Python (Nacos SDK)

```python
import nacos
client = nacos.NacosClient(server_addresses="localhost:8848")
config = client.get_config("dataId", "group")
```

## 监控

Prometheus 指标端点：`/nacos/actuator/prometheus`

```
# HELP batata_http_requests_total HTTP 请求总数
# TYPE batata_http_requests_total counter
batata_http_requests_total{method="GET",path="/nacos/v2/cs/config"} 1234

# HELP batata_config_count 当前配置数量
# TYPE batata_config_count gauge
batata_config_count{namespace="public"} 100

# HELP batata_service_count 当前服务数量
# TYPE batata_service_count gauge
batata_service_count{namespace="public"} 50

# HELP batata_cluster_member_count 集群成员数量
# TYPE batata_cluster_member_count gauge
batata_cluster_member_count 3
```

## 路线图

- [x] 持久服务实例（数据库存储）
- [x] Distro 协议完善（AP 模式）
- [x] 配置静态加密（AES-128-CBC）
- [x] OpenTelemetry 集成（OTLP 导出）
- [x] 多数据中心支持（地域感知同步）
- [x] 服务网格 xDS 协议（EDS、CDS、LDS、RDS、ADS）
- [x] AI 集成（MCP 服务器注册、A2A 代理注册）
- [x] Kubernetes 同步（双向服务同步）
- [x] LDAP 认证
- [x] OAuth2/OIDC 认证（Google、GitHub、Microsoft）
- [x] 分布式锁（基于 Raft）
- [x] DNS 服务发现
- [x] 灰度发布 API
- [x] 操作审计日志
- [ ] Kubernetes Operator
- [ ] Web UI（仅后端 API，可使用 Nacos UI 或自定义前端）

## 贡献

欢迎贡献代码！请随时提交 Issue 和 Pull Request。

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 许可证

本项目采用 Apache-2.0 许可证 - 详见 [LICENSE](LICENSE) 文件。

## 致谢

- [Nacos](https://nacos.io/) - 原始设计和 API 规范
- [Consul](https://www.consul.io/) - KV 存储和服务发现 API 设计
- [OpenRaft](https://github.com/datafuselabs/openraft) - Raft 共识实现
- [SeaORM](https://www.sea-ql.org/SeaORM/) - 异步 ORM 框架
- [Actix-web](https://actix.rs/) - 高性能 Web 框架
- [Tonic](https://github.com/hyperium/tonic) - gRPC 实现
