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
- **Apollo API** - 完全兼容 Apollo Config 客户端 SDK 和 Open API
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
| Apollo API 兼容 | ❌ | ✅ | **额外特性** |
| **可观测性** | | | |
| Prometheus 指标 | ✅ | ✅ | 完整 |
| 健康检查端点 | ✅ | ✅ | 完整 |
| OpenTelemetry | ✅ | ✅ | 完整 |
| 操作审计日志 | ✅ | ✅ | 完整 |

### Batata 独有特性

| 特性 | 说明 |
|-----|------|
| **Consul API 兼容** | 完整支持 Agent、Health、Catalog、KV、ACL API |
| **Apollo API 兼容** | 完整支持 Apollo 客户端 SDK 和 Open API |
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

Batata 采用多 crate 工作空间架构，提高模块化和可维护性：

```
batata/
├── crates/
│   ├── batata-common/            # 公共工具和类型
│   ├── batata-api/               # API 定义 & gRPC proto
│   ├── batata-persistence/       # 数据库实体 (SeaORM)
│   ├── batata-auth/              # 认证与授权
│   ├── batata-consistency/       # Raft 共识协议
│   ├── batata-core/              # 核心抽象 & 集群
│   ├── batata-config/            # 配置服务
│   ├── batata-naming/            # 服务发现
│   ├── batata-plugin/            # 插件接口
│   ├── batata-plugin-consul/     # Consul 兼容插件
│   ├── batata-plugin-apollo/     # Apollo 兼容插件
│   ├── batata-console/           # 控制台后端服务
│   ├── batata-client/            # 客户端 SDK
│   ├── batata-mesh/              # 服务网格 (xDS, Istio MCP)
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
├── batata-plugin-consul (Consul API)
├── batata-plugin-apollo (Apollo API)
└── batata-persistence (数据库)
```

## 快速开始

### 前置条件

- Rust 1.85+（Edition 2024）
- MySQL 5.7+ 或 8.0+

### 安装

```bash
# 克隆仓库
git clone https://github.com/easynet-cn/batata.git
cd batata

# 初始化数据库
mysql -u root -p < conf/mysql-schema.sql

# 配置应用
cp conf/application.yml.example conf/application.yml
# 编辑 conf/application.yml 配置数据库连接信息

# 构建并运行
cargo build --release -p batata-server
./target/release/batata-server
```

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

```yaml
nacos:
  server:
    main:
      port: 8848
    context-path: /nacos
  console:
    port: 8081
  core:
    auth:
      enabled: true
      plugin:
        nacos:
          token:
            secret:
              key: "your-base64-encoded-secret-key"
      token:
        expire:
          seconds: 18000
  standalone: true  # 集群模式设为 false

db:
  url: "mysql://user:password@localhost:3306/batata"

cluster:
  member:
    lookup:
      type: standalone  # standalone, file, 或 address-server
```

### 环境变量

```bash
export NACOS_DB_URL="mysql://user:pass@localhost:3306/batata"
export NACOS_SERVER_MAIN_PORT=8848
export NACOS_CORE_AUTH_ENABLED=true
export RUST_LOG=info
```

### OpenTelemetry 配置

启用 OpenTelemetry OTLP 导出的分布式追踪：

**application.yml:**
```yaml
nacos:
  otel:
    enabled: true
    endpoint: "http://localhost:4317"  # OTLP gRPC 端点
    service_name: "batata"
    sampling_ratio: 1.0  # 采样率 0.0 到 1.0
    export_timeout_secs: 10
```

**环境变量（优先级高于配置文件）：**
```bash
export OTEL_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4317"
export OTEL_SERVICE_NAME="batata"
export OTEL_SAMPLING_RATIO=1.0
export OTEL_EXPORT_TIMEOUT_SECS=10
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
xds:
  enabled: true
  port: 15010                    # xDS gRPC 服务器端口
  server_id: "batata-xds-server"
  sync_interval_ms: 5000         # 服务同步间隔
  generate_listeners: true       # 生成 LDS 资源
  generate_routes: true          # 生成 RDS 资源
  default_listener_port: 15001   # 生成资源的默认监听端口
```

**支持的 xDS 资源：**
- **EDS** (端点发现服务) - 服务端点
- **CDS** (集群发现服务) - 上游集群
- **LDS** (监听器发现服务) - 监听器配置
- **RDS** (路由发现服务) - 路由配置
- **ADS** (聚合发现服务) - 通过单一流获取所有资源

**兼容：** Envoy Proxy、Istio 及任何 xDS 兼容的服务网格。

## API 参考

### Nacos 配置 API

| 端点 | 方法 | 描述 |
|------|------|------|
| `/nacos/v1/cs/configs` | GET | 获取配置 |
| `/nacos/v1/cs/configs` | POST | 创建/更新配置 |
| `/nacos/v1/cs/configs` | DELETE | 删除配置 |
| `/nacos/v3/console/cs/config` | GET | 获取配置详情（控制台） |
| `/nacos/v3/console/cs/config/list` | GET | 分页获取配置列表 |
| `/nacos/v3/console/cs/config/export` | GET | 导出配置（ZIP） |
| `/nacos/v3/console/cs/config/import` | POST | 导入配置（ZIP） |

### Nacos 服务发现 API

| 端点 | 方法 | 描述 |
|------|------|------|
| `/nacos/v1/ns/instance` | POST | 注册实例 |
| `/nacos/v1/ns/instance` | PUT | 更新实例 |
| `/nacos/v1/ns/instance` | DELETE | 注销实例 |
| `/nacos/v1/ns/instance/list` | GET | 获取服务实例列表 |
| `/nacos/v1/ns/instance/beat` | PUT | 发送心跳 |

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

### Apollo API

| 端点 | 方法 | 描述 |
|------|------|------|
| `/configs/{appId}/{cluster}/{namespace}` | GET | 获取配置 |
| `/configfiles/{appId}/{cluster}/{namespace}` | GET | 获取配置文本 |
| `/configfiles/json/{appId}/{cluster}/{namespace}` | GET | 获取 JSON 格式配置 |
| `/notifications/v2` | GET | 配置变更长轮询 |
| `/openapi/v1/apps` | GET | 列出所有应用 |
| `/openapi/v1/apps/{appId}/envclusters` | GET | 获取环境集群 |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces` | GET | 列出命名空间 |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items` | GET | 列出配置项 |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items` | POST | 创建配置项 |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/releases` | POST | 发布配置 |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/lock` | POST | 获取命名空间锁 |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/gray` | POST | 创建灰度发布 |
| `/openapi/v1/apps/{appId}/accesskeys` | POST | 创建访问密钥 |
| `/openapi/v1/metrics/clients` | GET | 获取客户端指标 |

### 控制台 API (v3)

| 端点 | 方法 | 描述 |
|------|------|------|
| `/nacos/v3/console/namespace/list` | GET | 列出命名空间 |
| `/nacos/v3/console/namespace` | POST | 创建命名空间 |
| `/nacos/v3/console/core/cluster/nodes` | GET | 列出集群节点 |
| `/nacos/v3/console/core/cluster/health` | GET | 获取集群健康状态 |

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
│  │  │    MySQL    │  │   RocksDB   │  │    Moka 缓存        │  │ │
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

### 单机模式

```bash
# 直接运行
cargo run --release -p batata-server

# 或使用环境变量
NACOS_STANDALONE=true ./target/release/batata-server
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
   nacos:
     standalone: false
   cluster:
     member:
       lookup:
         type: file
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
sea-orm-cli generate entity \
  -u "mysql://user:pass@localhost:3306/batata" \
  -o ./crates/batata-persistence/src/entity \
  --with-serde both
```

### 项目统计

- **~50,000+ 行** Rust 代码
- **14 个内部 crate** 工作空间
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

### Java (Apollo SDK)

```java
Config config = ConfigService.getAppConfig();
String value = config.getProperty("key", "defaultValue");

// 使用自定义命名空间
Config applicationConfig = ConfigService.getConfig("application");
```

### Go (Apollo SDK)

```go
client, _ := agollo.Start(&config.AppConfig{
    AppID:         "your-app-id",
    Cluster:       "default",
    NamespaceName: "application",
    IP:            "localhost:8848",
})
value := client.GetValue("key")
```

## 监控

Prometheus 指标端点：`/nacos/actuator/prometheus`

```
# HELP batata_http_requests_total HTTP 请求总数
# TYPE batata_http_requests_total counter
batata_http_requests_total{method="GET",path="/nacos/v1/cs/configs"} 1234

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
- [x] Apollo Config API 兼容
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
- [Apollo](https://www.apolloconfig.com/) - 配置管理 API 设计
- [OpenRaft](https://github.com/datafuselabs/openraft) - Raft 共识实现
- [SeaORM](https://www.sea-ql.org/SeaORM/) - 异步 ORM 框架
- [Actix-web](https://actix.rs/) - 高性能 Web 框架
- [Tonic](https://github.com/hyperium/tonic) - gRPC 实现
