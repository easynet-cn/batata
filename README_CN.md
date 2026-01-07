# Batata

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/easynet-cn/batata/ci.yml?branch=main)](https://github.com/easynet-cn/batata/actions)

**Batata** 是一个基于 Rust 实现的高性能动态服务发现、配置管理和服务管理平台。完全兼容 [Nacos](https://nacos.io/) 和 [Consul](https://www.consul.io/) API，可作为云原生应用的理想替代方案。

[English Documentation](README.md)

## 特性

### 核心能力

- **配置管理** - 集中式动态配置，支持实时更新
- **服务发现** - 服务注册、发现和健康检查
- **命名空间隔离** - 基于命名空间的多租户资源隔离
- **集群模式** - 基于 Raft 共识协议的高可用方案

### API 兼容性

- **Nacos API** - 完全兼容 Nacos v1 和 v3 API
- **Consul API** - 兼容 Consul Agent、Health、Catalog、KV 和 ACL API
- **gRPC 支持** - 高性能双向流式通信，支持 SDK 客户端

### 高级特性

- **配置导入/导出** - 支持 Nacos ZIP 和 Consul JSON 格式的批量配置迁移
- **灰度发布** - 支持配置的渐进式发布
- **认证与授权** - 基于 JWT 的认证和 RBAC 权限模型
- **限流** - 可配置的 API 限流保护
- **熔断器** - 故障容错的弹性模式
- **监控与可观测性** - 兼容 Prometheus 的指标端点

### 性能优势

- **快速启动** - 约 1-2 秒（对比 Java 版 Nacos 的 30-60 秒）
- **低内存占用** - 约 50-100MB（对比 Java 版 Nacos 的 1-2GB）
- **高吞吐量** - 基于 Tokio 运行时的异步 I/O 优化
- **高效存储** - 使用 RocksDB 持久化存储，配合优化的缓存策略

## 项目结构

Batata 采用多 crate 工作空间架构，提高模块化和可维护性：

```
batata/
├── src/                          # 主应用程序
│   ├── api/                      # API 处理器 (HTTP, gRPC, Consul)
│   ├── auth/                     # 认证与授权
│   ├── config/                   # 配置模型
│   ├── console/                  # Web 控制台 API
│   ├── core/                     # 核心业务逻辑 & Raft
│   ├── entity/                   # 数据库实体（重导出）
│   ├── middleware/               # HTTP 中间件
│   ├── model/                    # 数据模型
│   └── service/                  # 业务服务
├── crates/
│   ├── batata-api/               # API 定义 & gRPC proto
│   ├── batata-auth/              # 认证模块
│   ├── batata-client/            # 客户端 SDK
│   ├── batata-common/            # 公共工具
│   ├── batata-config/            # 配置服务
│   ├── batata-console/           # 控制台服务
│   ├── batata-consistency/       # 一致性协议
│   ├── batata-core/              # 核心抽象 & 集群
│   ├── batata-naming/            # 服务发现
│   ├── batata-persistence/       # 数据库实体 (SeaORM)
│   ├── batata-plugin/            # 插件接口
│   ├── batata-plugin-consul/     # Consul 兼容插件
│   └── batata-server/            # 服务器二进制
├── conf/                         # 配置文件
├── proto/                        # Protocol buffer 定义
├── tests/                        # 集成测试
└── benches/                      # 性能基准测试
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
cargo build --release
./target/release/batata
```

### 默认端口

| 端口 | 服务 | 描述 |
|------|------|------|
| 8848 | 主 HTTP API | Nacos 兼容 API |
| 8081 | 控制台 HTTP API | Web 管理控制台 |
| 9848 | SDK gRPC | 客户端 SDK 通信 |
| 9849 | 集群 gRPC | 节点间通信 |

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
cargo run --release

# 或使用环境变量
NACOS_STANDALONE=true ./target/release/batata
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
# 开发构建
cargo build

# 发布构建（优化）
cargo build --release

# 运行测试
cargo test

# 运行指定测试
cargo test test_name

# 运行性能基准测试
cargo bench

# 格式化代码
cargo fmt

# Clippy 检查
cargo clippy

# 生成文档
cargo doc --open
```

### 生成数据库实体

```bash
sea-orm-cli generate entity \
  -u "mysql://user:pass@localhost:3306/batata" \
  -o ./crates/batata-persistence/src/entity \
  --with-serde both
```

### 项目统计

- **~47,500+ 行** Rust 代码
- **13 个内部 crate** 工作空间
- **178 个单元测试**，全面覆盖
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

- [ ] Kubernetes Operator
- [ ] Web UI 控制台
- [ ] OpenTelemetry 集成
- [ ] 配置静态加密
- [ ] 多数据中心支持

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
