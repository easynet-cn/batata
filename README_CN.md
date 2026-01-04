# Batata

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Batata** 是一个高性能的 Rust 实现的动态服务发现、配置管理和服务管理平台。完全兼容 [Nacos](https://nacos.io/) 和 [Consul](https://www.consul.io/) API，可作为云原生应用的即插即用替代方案。

[English](README.md)

## 功能特性

### 核心能力

- **配置管理** - 集中式、动态配置，支持实时更新推送
- **服务发现** - 服务注册、发现和健康检查
- **命名空间隔离** - 多租户支持，基于命名空间的资源隔离
- **集群模式** - 基于 Raft 共识协议的高可用集群

### API 兼容性

- **Nacos API** - 完全兼容 Nacos v1 和 v3 API
- **Consul API** - 兼容 Consul Agent、Health、Catalog、KV 和 ACL API
- **gRPC 支持** - 高性能双向流，用于 SDK 客户端通信

### 高级特性

- **配置导入导出** - 支持 Nacos ZIP 和 Consul JSON 格式的批量配置迁移
- **认证授权** - 基于 JWT 的认证，RBAC 权限模型
- **限流保护** - 可配置的 API 限流
- **熔断器** - 容错弹性设计模式
- **可观测性** - Prometheus 兼容的指标端点

### 性能优势

- **快速启动** - 约 1-2 秒，对比 Java 版 Nacos 的 30-60 秒
- **低内存占用** - 约 100-200MB，对比 Java 版 Nacos 的 1-2GB
- **高吞吐量** - 基于 Tokio 运行时的优化异步 I/O
- **高效存储** - RocksDB 持久化存储，优化的缓存配置

## 快速开始

### 环境要求

- Rust 1.75+ (Edition 2024)
- MySQL 5.7+ 或 8.0+

### 安装部署

```bash
# 克隆仓库
git clone https://github.com/easynet-cn/batata.git
cd batata

# 初始化数据库
mysql -u root -p < conf/mysql-schema.sql

# 配置应用
cp conf/application.yml.example conf/application.yml
# 编辑 conf/application.yml，配置数据库连接信息

# 构建并运行
cargo build --release
./target/release/batata
```

### 默认端口

| 端口 | 服务 |
|------|------|
| 8848 | 主 HTTP API |
| 8081 | 控制台 HTTP API |
| 9848 | SDK gRPC |
| 9849 | 集群 gRPC |

## 配置说明

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
      token:
        expire:
          seconds: 18000

db:
  url: "mysql://user:password@localhost:3306/batata"

cluster:
  member:
    lookup:
      type: standalone  # standalone, file 或 address-server
```

环境变量覆盖：
```bash
export BATATA_DB_URL="mysql://user:pass@localhost:3306/batata"
export BATATA_SERVER_PORT=8848
export RUST_LOG=info
```

## API 参考

### Nacos API

| 端点 | 方法 | 描述 |
|------|------|------|
| `/nacos/v1/cs/configs` | GET/POST/DELETE | 配置 CRUD |
| `/nacos/v1/ns/instance` | POST/PUT/DELETE | 实例注册 |
| `/nacos/v1/ns/instance/list` | GET | 获取服务实例列表 |
| `/nacos/v3/cs/config/export` | GET | 导出配置（ZIP 格式） |
| `/nacos/v3/cs/config/import` | POST | 导入配置（ZIP 格式） |

### Consul API

| 端点 | 方法 | 描述 |
|------|------|------|
| `/v1/agent/service/register` | PUT | 注册服务 |
| `/v1/health/service/{service}` | GET | 获取服务健康状态 |
| `/v1/kv/{key}` | GET/PUT/DELETE | KV 存储操作 |
| `/v1/kv/export` | GET | 导出 KV（JSON 格式） |
| `/v1/kv/import` | PUT | 导入 KV（JSON 格式） |

### 配置导入导出

**Nacos 格式（ZIP）**
```bash
# 导出
curl -O "http://localhost:8848/nacos/v3/console/cs/config/export?namespaceId=public"

# 导入（指定冲突策略）
curl -X POST "http://localhost:8848/nacos/v3/console/cs/config/import?namespaceId=public&policy=OVERWRITE" \
  -F "file=@export.zip"
```

**Consul 格式（JSON）**
```bash
# 导出
curl "http://localhost:8848/v1/kv/export?namespaceId=public"

# 导入
curl -X PUT "http://localhost:8848/v1/kv/import" \
  -H "Content-Type: application/json" \
  -d @configs.json
```

**冲突策略说明**

| 策略 | 描述 |
|------|------|
| `SKIP` | 跳过已存在的配置（默认） |
| `OVERWRITE` | 覆盖已存在的配置 |
| `ABORT` | 遇到冲突立即停止导入 |

## 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                       Batata 服务器                          │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  HTTP API   │  │  gRPC API   │  │   控制台 API        │  │
│  │  (Actix)    │  │  (Tonic)    │  │   (Actix)           │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         │                │                     │             │
│  ┌──────▼─────────────────▼─────────────────────▼──────────┐ │
│  │                     服务层                               │ │
│  │  ┌─────────┐  ┌──────────┐  ┌────────┐  ┌───────────┐   │ │
│  │  │  配置   │  │  命名    │  │  认证  │  │  命名空间 │   │ │
│  │  └─────────┘  └──────────┘  └────────┘  └───────────┘   │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                     存储层                               │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │ │
│  │  │   MySQL     │  │  RocksDB    │  │   Moka 缓存     │  │ │
│  │  │  (SeaORM)   │  │ (Raft 日志) │  │   (内存)        │  │ │
│  │  └─────────────┘  └─────────────┘  └─────────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                     集群层                               │ │
│  │  ┌─────────────────────────────────────────────────────┐│ │
│  │  │              Raft 共识 (OpenRaft)                   ││ │
│  │  └─────────────────────────────────────────────────────┘│ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## 部署方式

### Docker 部署

```bash
docker-compose up -d
```

### Kubernetes 部署

详见 [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) 了解完整的 Kubernetes 部署配置。

### 集群模式

1. 在 `conf/cluster.conf` 中配置集群成员地址
2. 设置 `cluster.member.lookup.type: file`
3. 启动所有节点

## 开发指南

```bash
# 构建
cargo build

# 运行测试
cargo test

# 运行基准测试
cargo bench

# 代码格式化
cargo fmt

# 代码检查
cargo clippy
```

### 生成数据库实体

```bash
sea-orm-cli generate entity \
  -u "mysql://user:pass@localhost:3306/batata" \
  -o ./src/entity \
  --with-serde both
```

## 客户端 SDK

Batata 兼容现有的 Nacos 和 Consul 客户端 SDK：

**Java (Nacos SDK)**
```java
ConfigService configService = NacosFactory.createConfigService("localhost:8848");
String config = configService.getConfig("dataId", "group", 5000);
```

**Go (Consul SDK)**
```go
client, _ := api.NewClient(api.DefaultConfig())
kv, _, _ := client.KV().Get("key", nil)
```

**Python (Nacos SDK)**
```python
client = nacos.NacosClient(server_addresses="localhost:8848")
config = client.get_config("dataId", "group")
```

## 监控指标

Prometheus 指标端点：`/nacos/metrics`

- `batata_http_requests_total` - HTTP 请求总数
- `batata_config_publish_total` - 配置发布总数
- `batata_service_register_total` - 服务注册总数
- `batata_active_connections` - 活跃连接数

## 文档

- [部署指南](docs/DEPLOYMENT.md) - 详细的部署文档
- [Nacos 文档](https://nacos.io/docs/) - Nacos API 参考
- [Consul 文档](https://developer.hashicorp.com/consul/docs) - Consul API 参考

## 贡献

欢迎贡献代码！请随时提交 Issue 和 Pull Request。

## 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。

## 致谢

- [Nacos](https://nacos.io/) - 提供原始设计和 API 规范
- [Consul](https://www.consul.io/) - 提供 KV 存储和服务发现 API 设计
- [OpenRaft](https://github.com/datafuselabs/openraft) - 提供 Raft 共识实现
