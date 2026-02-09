# Batata 部署与使用指南

Batata 是一个 Rust 实现的 Nacos 兼容服务，提供服务发现、配置管理和集群协调功能。同时兼容 Consul API。

## 目录

- [系统要求](#系统要求)
- [快速开始](#快速开始)
- [构建](#构建)
- [配置](#配置)
- [部署模式](#部署模式)
- [API 参考](#api-参考)
  - [Nacos API](#nacos-api)
  - [Consul 兼容 API](#consul-兼容-api)
  - [配置导入导出](#配置导入导出)
- [OpenTelemetry 配置](#opentelemetry-配置)
- [多数据中心部署](#多数据中心部署)
- [监控与指标](#监控与指标)
- [客户端集成](#客户端集成)
- [故障排除](#故障排除)

---

## 系统要求

### 硬件要求

| 组件 | 最低配置 | 推荐配置 |
|------|----------|----------|
| CPU | 2 核 | 4 核+ |
| 内存 | 2 GB | 4 GB+ |
| 磁盘 | 10 GB SSD | 50 GB SSD |

### 软件要求

- **操作系统**: Linux (推荐 Ubuntu 20.04+), macOS, Windows
- **Rust**: 1.75+ (Edition 2024)
- **MySQL**: 5.7+ 或 8.0+
- **可选**: Docker, Kubernetes

---

## 快速开始

### 1. 克隆仓库

```bash
git clone https://github.com/easynet-cn/batata.git
cd batata
```

### 2. 初始化数据库

```bash
mysql -u root -p < conf/mysql-schema.sql
mysql -u root -p < conf/consul-mysql-schema.sql
mysql -u root -p < conf/apollo-mysql-schema.sql
```

### 3. 配置应用

```bash
cp conf/application.yml.example conf/application.yml
# 编辑配置文件，设置数据库连接等
```

### 4. 构建并运行

```bash
cargo build --release
./target/release/batata
```

服务将在以下端口启动：
- **8848**: 主 API 服务
- **8081**: 控制台服务
- **9848**: SDK gRPC 服务
- **9849**: 集群 gRPC 服务

---

## 构建

### 开发构建

```bash
cargo build
```

### 发布构建（优化）

```bash
cargo build --release
```

### 运行测试

```bash
cargo test
```

### 代码检查

```bash
cargo clippy
cargo fmt --check
```

### 生成文档

```bash
cargo doc --open
```

---

## 配置

### 配置文件位置

默认配置文件: `conf/application.yml`

### 核心配置项

```yaml
# conf/application.yml

# 服务器配置
nacos:
  server:
    # 主服务端口
    main:
      port: 8848
    # 上下文路径
    context-path: /nacos

  # 控制台配置
  console:
    port: 8081
    context-path: /nacos

  # SDK gRPC 端口
  sdk:
    port: 9848

  # 集群 gRPC 端口
  cluster:
    port: 9849

  # 认证配置
  core:
    auth:
      # 是否启用认证
      enabled: true
      # Token 过期时间（秒）
      token:
        expire:
          seconds: 18000
      plugin:
        nacos:
          token:
            # JWT 密钥（Base64 编码，至少 32 字节）
            secret:
              key: "YOUR_BASE64_ENCODED_SECRET_KEY_HERE"

# 数据库配置
db:
  url: "mysql://user:password@localhost:3306/batata"

# 部署类型: all, server, console
deployment:
  type: all

# 集群配置
cluster:
  # 成员发现方式: file, address-server, standalone
  member:
    lookup:
      type: standalone
  # 集群成员列表（file 模式）
  members:
    - "127.0.0.1:8848"
```

### 环境变量覆盖

所有配置项都可通过环境变量覆盖：

```bash
# 数据库连接
export BATATA_DB_URL="mysql://user:pass@localhost:3306/batata"

# JWT 密钥
export BATATA_AUTH_SECRET_KEY="base64_encoded_key"

# 主服务端口
export BATATA_SERVER_PORT=8848
```

### 日志配置

通过 `RUST_LOG` 环境变量控制日志级别：

```bash
# 设置全局日志级别
export RUST_LOG=info

# 设置特定模块日志级别
export RUST_LOG=batata=debug,sea_orm=warn
```

---

## 部署模式

### 单机模式（开发/测试）

```bash
# 使用默认配置启动
./target/release/batata

# 或指定配置文件
./target/release/batata --config /path/to/application.yml
```

### 集群模式（生产）

#### 1. 配置集群成员

编辑每个节点的 `conf/cluster.conf`：

```
# 节点 1
192.168.1.10:8848
192.168.1.11:8848
192.168.1.12:8848
```

#### 2. 配置成员发现

```yaml
# application.yml
cluster:
  member:
    lookup:
      type: file
```

#### 3. 启动所有节点

```bash
# 在每个节点上执行
./target/release/batata
```

### Docker 部署

#### Dockerfile

```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/batata /usr/local/bin/
COPY --from=builder /app/conf /etc/batata/conf

EXPOSE 8848 8081 9848 9849

CMD ["batata", "--config", "/etc/batata/conf/application.yml"]
```

#### docker-compose.yml

```yaml
version: '3.8'

services:
  mysql:
    image: mysql:8.0
    environment:
      MYSQL_ROOT_PASSWORD: root
      MYSQL_DATABASE: batata
    volumes:
      - mysql_data:/var/lib/mysql
      - ./conf/mysql-schema.sql:/docker-entrypoint-initdb.d/01-schema.sql
      - ./conf/consul-mysql-schema.sql:/docker-entrypoint-initdb.d/02-consul-schema.sql
      - ./conf/apollo-mysql-schema.sql:/docker-entrypoint-initdb.d/03-apollo-schema.sql
    ports:
      - "3306:3306"

  batata:
    build: .
    depends_on:
      - mysql
    environment:
      BATATA_DB_URL: "mysql://root:root@mysql:3306/batata"
      RUST_LOG: info
    ports:
      - "8848:8848"
      - "8081:8081"
      - "9848:9848"
      - "9849:9849"
    restart: unless-stopped

volumes:
  mysql_data:
```

#### 启动

```bash
docker-compose up -d
```

### Kubernetes 部署

#### ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: batata-config
data:
  application.yml: |
    nacos:
      server:
        main:
          port: 8848
    db:
      url: "mysql://user:pass@mysql-service:3306/batata"
    cluster:
      member:
        lookup:
          type: file
```

#### StatefulSet

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: batata
spec:
  serviceName: batata
  replicas: 3
  selector:
    matchLabels:
      app: batata
  template:
    metadata:
      labels:
        app: batata
    spec:
      containers:
      - name: batata
        image: batata:latest
        ports:
        - containerPort: 8848
          name: http
        - containerPort: 9848
          name: grpc
        - containerPort: 9849
          name: cluster
        volumeMounts:
        - name: config
          mountPath: /etc/batata
        - name: data
          mountPath: /var/lib/batata
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /nacos/v1/console/health/liveness
            port: 8848
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /nacos/v1/console/health/readiness
            port: 8848
          initialDelaySeconds: 10
          periodSeconds: 5
      volumes:
      - name: config
        configMap:
          name: batata-config
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 10Gi
```

#### Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: batata
spec:
  selector:
    app: batata
  ports:
  - port: 8848
    name: http
  - port: 9848
    name: grpc
  clusterIP: None  # Headless service for StatefulSet
```

---

## API 参考

### Nacos API

#### 配置管理

| 端点 | 方法 | 描述 |
|------|------|------|
| `/nacos/v1/cs/configs` | GET | 获取配置 |
| `/nacos/v1/cs/configs` | POST | 发布配置 |
| `/nacos/v1/cs/configs` | DELETE | 删除配置 |
| `/nacos/v1/cs/configs/listener` | POST | 监听配置变更 |
| `/nacos/v3/cs/config` | GET | 获取配置（v3） |
| `/nacos/v3/cs/config` | POST | 发布配置（v3） |
| `/nacos/v3/cs/config/export` | GET | 导出配置（ZIP 格式） |
| `/nacos/v3/cs/config/import` | POST | 导入配置（multipart/form-data） |

#### 服务发现

| 端点 | 方法 | 描述 |
|------|------|------|
| `/nacos/v1/ns/instance` | POST | 注册实例 |
| `/nacos/v1/ns/instance` | DELETE | 注销实例 |
| `/nacos/v1/ns/instance` | PUT | 更新实例 |
| `/nacos/v1/ns/instance/list` | GET | 获取实例列表 |
| `/nacos/v1/ns/service/list` | GET | 获取服务列表 |

#### 命名空间

| 端点 | 方法 | 描述 |
|------|------|------|
| `/nacos/v1/console/namespaces` | GET | 获取命名空间列表 |
| `/nacos/v1/console/namespaces` | POST | 创建命名空间 |
| `/nacos/v1/console/namespaces` | PUT | 更新命名空间 |
| `/nacos/v1/console/namespaces` | DELETE | 删除命名空间 |

#### 认证

| 端点 | 方法 | 描述 |
|------|------|------|
| `/nacos/v1/auth/login` | POST | 用户登录 |
| `/nacos/v1/auth/users` | GET | 获取用户列表 |
| `/nacos/v1/auth/users` | POST | 创建用户 |

### Consul 兼容 API

#### Agent API

| 端点 | 方法 | 描述 |
|------|------|------|
| `/v1/agent/services` | GET | 列出所有服务 |
| `/v1/agent/service/{id}` | GET | 获取服务详情 |
| `/v1/agent/service/register` | PUT | 注册服务 |
| `/v1/agent/service/deregister/{id}` | PUT | 注销服务 |

#### Health API

| 端点 | 方法 | 描述 |
|------|------|------|
| `/v1/health/service/{service}` | GET | 获取服务健康状态 |
| `/v1/health/checks/{service}` | GET | 获取服务检查 |
| `/v1/health/state/{state}` | GET | 按状态获取检查 |

#### Catalog API

| 端点 | 方法 | 描述 |
|------|------|------|
| `/v1/catalog/services` | GET | 列出所有服务 |
| `/v1/catalog/service/{service}` | GET | 获取服务实例 |
| `/v1/catalog/nodes` | GET | 列出所有节点 |
| `/v1/catalog/node/{node}` | GET | 获取节点详情 |
| `/v1/catalog/register` | PUT | 注册到目录 |
| `/v1/catalog/deregister` | PUT | 从目录注销 |

#### KV Store API

| 端点 | 方法 | 描述 |
|------|------|------|
| `/v1/kv/{key}` | GET | 获取键值 |
| `/v1/kv/{key}` | PUT | 设置键值 |
| `/v1/kv/{key}` | DELETE | 删除键值 |
| `/v1/kv/export` | GET | 导出 KV（JSON 格式） |
| `/v1/kv/import` | PUT | 导入 KV（JSON 格式） |
| `/v1/txn` | PUT | 事务操作 |

### gRPC API

gRPC 服务定义在 `proto/nacos_grpc_service.proto`：

```protobuf
service Request {
  rpc request(Payload) returns (Payload);
}

service BiRequestStream {
  rpc requestBiStream(stream Payload) returns (stream Payload);
}
```

### 配置导入导出

#### Nacos 格式导出

导出配置为 ZIP 文件，包含配置内容和元数据：

```bash
# 导出指定命名空间的所有配置
curl -O "http://localhost:8848/nacos/v3/console/cs/config/export?namespaceId=public"

# 导出指定 Group 的配置
curl -O "http://localhost:8848/nacos/v3/console/cs/config/export?namespaceId=public&group=DEFAULT_GROUP"

# 导出指定 dataIds 的配置（逗号分隔）
curl -O "http://localhost:8848/nacos/v3/console/cs/config/export?namespaceId=public&dataIds=app.yaml,db.properties"

# 按应用名称过滤
curl -O "http://localhost:8848/nacos/v3/console/cs/config/export?namespaceId=public&appName=my-app"
```

**ZIP 文件结构**：
```
export.zip
├── DEFAULT_GROUP/
│   ├── app.yaml           # 配置内容
│   ├── app.yaml.meta      # 元数据 (YAML)
│   ├── db.properties
│   └── db.properties.meta
└── MY_GROUP/
    ├── service.json
    └── service.json.meta
```

**元数据文件格式 (.meta)**：
```yaml
dataId: app.yaml
group: DEFAULT_GROUP
namespaceId: public
contentType: yaml
appName: my-app
desc: 应用配置
configTags: env:prod,team:backend
md5: abc123def456
createTime: 1704067200000
modifyTime: 1704067200000
```

#### Nacos 格式导入

导入 ZIP 格式的配置文件：

```bash
# 基本导入（遇到冲突跳过）
curl -X POST "http://localhost:8848/nacos/v3/console/cs/config/import?namespaceId=public" \
  -F "file=@export.zip"

# 覆盖已存在的配置
curl -X POST "http://localhost:8848/nacos/v3/console/cs/config/import?namespaceId=public&policy=OVERWRITE" \
  -F "file=@export.zip"

# 遇到冲突立即停止
curl -X POST "http://localhost:8848/nacos/v3/console/cs/config/import?namespaceId=public&policy=ABORT" \
  -F "file=@export.zip"
```

**冲突策略（policy 参数）**：

| 策略 | 描述 |
|------|------|
| `SKIP` | 跳过已存在的配置（默认） |
| `OVERWRITE` | 覆盖已存在的配置 |
| `ABORT` | 遇到冲突立即停止导入 |

**响应示例**：
```json
{
  "code": 0,
  "message": "success",
  "data": {
    "successCount": 10,
    "skipCount": 2,
    "failCount": 0,
    "failData": []
  }
}
```

#### Consul 格式导出

导出配置为 JSON 数组格式（值为 Base64 编码）：

```bash
# 导出所有配置
curl "http://localhost:8848/v1/kv/export"

# 按命名空间过滤
curl "http://localhost:8848/v1/kv/export?namespaceId=public"

# 按前缀过滤
curl "http://localhost:8848/v1/kv/export?prefix=public/DEFAULT_GROUP"
```

**响应格式**：
```json
[
  {
    "Key": "public/DEFAULT_GROUP/app.yaml",
    "Flags": 0,
    "Value": "c2VydmVyOgogIHBvcnQ6IDgwODA="
  },
  {
    "Key": "public/DEFAULT_GROUP/db.properties",
    "Flags": 0,
    "Value": "aG9zdD1sb2NhbGhvc3QKcG9ydD0zMzA2"
  }
]
```

**Key 路径映射**：
```
Key: namespace/group/dataId
示例: public/DEFAULT_GROUP/app.yaml
  → namespace_id: public
  → group: DEFAULT_GROUP
  → data_id: app.yaml
```

#### Consul 格式导入

导入 Consul 格式的 JSON 配置：

```bash
# 基本导入
curl -X PUT "http://localhost:8848/v1/kv/import" \
  -H "Content-Type: application/json" \
  -d '[
    {"Key": "public/DEFAULT_GROUP/app.yaml", "Flags": 0, "Value": "c2VydmVyOgogIHBvcnQ6IDgwODA="},
    {"Key": "public/DEFAULT_GROUP/db.properties", "Flags": 0, "Value": "aG9zdD1sb2NhbGhvc3Q="}
  ]'

# 指定目标命名空间（覆盖 Key 中的命名空间）
curl -X PUT "http://localhost:8848/v1/kv/import?namespaceId=dev" \
  -H "Content-Type: application/json" \
  -d @configs.json

# 使用冲突策略
curl -X PUT "http://localhost:8848/v1/kv/import?policy=OVERWRITE" \
  -H "Content-Type: application/json" \
  -d @configs.json
```

**响应示例**：
```json
{
  "code": 0,
  "message": "success",
  "data": {
    "successCount": 5,
    "skipCount": 1,
    "failCount": 0,
    "failData": []
  }
}
```

#### 导入导出使用场景

1. **环境迁移**：从开发环境导出配置，导入到测试/生产环境
2. **备份恢复**：定期导出配置作为备份，故障时快速恢复
3. **版本管理**：将导出的配置纳入 Git 版本控制
4. **批量更新**：修改导出文件后重新导入
5. **跨系统迁移**：Nacos 与 Consul 之间的配置迁移

---

## OpenTelemetry 配置

Batata 支持 OpenTelemetry 分布式追踪，可与 Jaeger、Zipkin、Grafana Tempo 等可观测性平台集成。

### 配置项

在 `application.yml` 中添加 OpenTelemetry 配置：

```yaml
# OpenTelemetry 配置
otel:
  # 是否启用 OpenTelemetry
  enabled: true
  # OTLP 导出端点 (gRPC)
  endpoint: "http://localhost:4317"
  # 服务名称
  service-name: "batata"
  # 采样率 (0.0-1.0, 1.0 = 100%)
  sampling-ratio: 1.0
  # 导出超时时间（秒）
  export-timeout-secs: 30
```

### 环境变量

```bash
# 启用 OpenTelemetry
export BATATA_OTEL_ENABLED=true

# OTLP 端点
export BATATA_OTEL_ENDPOINT="http://localhost:4317"

# 服务名称
export BATATA_OTEL_SERVICE_NAME="batata"

# 采样率
export BATATA_OTEL_SAMPLING_RATIO=1.0

# 导出超时
export BATATA_OTEL_EXPORT_TIMEOUT_SECS=30
```

### 与 Jaeger 集成

#### 启动 Jaeger

```bash
# 使用 Docker 启动 Jaeger All-in-One
docker run -d --name jaeger \
  -e COLLECTOR_OTLP_ENABLED=true \
  -p 16686:16686 \
  -p 4317:4317 \
  -p 4318:4318 \
  jaegertracing/all-in-one:latest
```

#### 配置 Batata

```yaml
otel:
  enabled: true
  endpoint: "http://localhost:4317"
  service-name: "batata"
  sampling-ratio: 1.0
```

#### 访问 Jaeger UI

打开 `http://localhost:16686` 查看分布式追踪数据。

### 与 Grafana Tempo 集成

#### docker-compose.yml 示例

```yaml
version: '3.8'

services:
  tempo:
    image: grafana/tempo:latest
    command: ["-config.file=/etc/tempo.yaml"]
    volumes:
      - ./tempo.yaml:/etc/tempo.yaml
    ports:
      - "4317:4317"   # OTLP gRPC
      - "4318:4318"   # OTLP HTTP
      - "3200:3200"   # Tempo API

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_AUTH_ANONYMOUS_ORG_ROLE=Admin

  batata:
    build: .
    environment:
      BATATA_OTEL_ENABLED: "true"
      BATATA_OTEL_ENDPOINT: "http://tempo:4317"
      BATATA_OTEL_SERVICE_NAME: "batata"
    depends_on:
      - tempo
```

### 追踪的操作

启用 OpenTelemetry 后，以下操作会被自动追踪：

| 操作类型 | 追踪内容 |
|----------|----------|
| HTTP 请求 | 请求路径、方法、状态码、延迟 |
| gRPC 调用 | 服务名、方法名、状态 |
| 数据库操作 | SQL 查询、执行时间 |
| 集群同步 | 节点间通信、同步延迟 |

### 生产环境建议

1. **采样率**：生产环境建议设置 0.1-0.5 的采样率以减少开销
2. **批量导出**：OTLP 导出器默认批量发送，减少网络开销
3. **超时设置**：根据网络状况调整导出超时时间
4. **资源标签**：通过环境变量添加额外的资源标签

```bash
export OTEL_RESOURCE_ATTRIBUTES="deployment.environment=production,service.version=1.0.0"
```

---

## 多数据中心部署

Batata 支持多数据中心部署，提供本地优先的数据访问和跨数据中心复制能力。

### 架构概述

```
                    ┌─────────────────────────────────────┐
                    │            全局集群                  │
                    └─────────────────────────────────────┘
                                     │
        ┌────────────────────────────┼────────────────────────────┐
        │                            │                            │
        ▼                            ▼                            ▼
┌───────────────┐          ┌───────────────┐          ┌───────────────┐
│  DC: 华东     │          │  DC: 华北     │          │  DC: 华南     │
│  Region: CN   │          │  Region: CN   │          │  Region: CN   │
├───────────────┤          ├───────────────┤          ├───────────────┤
│ Zone: zone-a  │          │ Zone: zone-a  │          │ Zone: zone-a  │
│  └─ 节点 1    │          │  └─ 节点 3    │          │  └─ 节点 5    │
│ Zone: zone-b  │          │ Zone: zone-b  │          │ Zone: zone-b  │
│  └─ 节点 2    │          │  └─ 节点 4    │          │  └─ 节点 6    │
└───────────────┘          └───────────────┘          └───────────────┘
```

### 配置项

#### application.yml

```yaml
# 多数据中心配置
datacenter:
  # 本地数据中心名称
  local: "cn-east-1"
  # 本地区域
  region: "cn-east"
  # 本地可用区
  zone: "zone-a"
  # 本地优先权重 (越高越优先)
  locality-weight: 1.0
  # 是否启用跨数据中心复制
  cross-dc-replication: true
  # 跨数据中心同步延迟（秒）
  cross-dc-sync-delay: 1
  # 每个数据中心的副本数
  replication-factor: 1
```

### 环境变量

```bash
# 数据中心名称
export BATATA_DATACENTER="cn-east-1"

# 区域
export BATATA_REGION="cn-east"

# 可用区
export BATATA_ZONE="zone-a"

# 本地优先权重
export BATATA_LOCALITY_WEIGHT=1.0

# 启用跨数据中心复制
export BATATA_CROSS_DC_REPLICATION=true

# 跨数据中心同步延迟
export BATATA_CROSS_DC_SYNC_DELAY_SECS=1

# 副本因子
export BATATA_REPLICATION_FACTOR=1
```

### 集群配置示例

#### 华东数据中心 (cn-east-1)

```yaml
# 节点 1: cn-east-1-zone-a
datacenter:
  local: "cn-east-1"
  region: "cn-east"
  zone: "zone-a"
  locality-weight: 1.0
  cross-dc-replication: true

cluster:
  member:
    lookup:
      type: file

# conf/cluster.conf
# 192.168.1.10:8848  # cn-east-1-zone-a
# 192.168.1.11:8848  # cn-east-1-zone-b
# 192.168.2.10:8848  # cn-north-1-zone-a
# 192.168.2.11:8848  # cn-north-1-zone-b
# 192.168.3.10:8848  # cn-south-1-zone-a
# 192.168.3.11:8848  # cn-south-1-zone-b
```

#### 华北数据中心 (cn-north-1)

```yaml
# 节点 3: cn-north-1-zone-a
datacenter:
  local: "cn-north-1"
  region: "cn-north"
  zone: "zone-a"
  locality-weight: 1.0
  cross-dc-replication: true
```

### Kubernetes 多数据中心部署

#### 使用 Pod 拓扑标签

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: batata
spec:
  replicas: 3
  template:
    spec:
      topologySpreadConstraints:
        - maxSkew: 1
          topologyKey: topology.kubernetes.io/zone
          whenUnsatisfiable: DoNotSchedule
          labelSelector:
            matchLabels:
              app: batata
      containers:
        - name: batata
          image: batata:latest
          env:
            - name: BATATA_DATACENTER
              valueFrom:
                fieldRef:
                  fieldPath: metadata.labels['topology.kubernetes.io/zone']
            - name: BATATA_REGION
              valueFrom:
                fieldRef:
                  fieldPath: metadata.labels['topology.kubernetes.io/region']
            - name: BATATA_ZONE
              valueFrom:
                fieldRef:
                  fieldPath: metadata.labels['topology.kubernetes.io/zone']
```

### 数据复制策略

| 策略 | 描述 | 适用场景 |
|------|------|----------|
| **本地优先** | 优先访问同数据中心节点 | 低延迟读取 |
| **跨 DC 复制** | 异步复制到远程数据中心 | 数据冗余、灾备 |
| **权重选择** | 按 locality_weight 选择节点 | 负载均衡 |

### 故障转移

当本地数据中心不可用时，请求会自动路由到远程数据中心：

```
1. 客户端请求 → 本地 DC (cn-east-1)
2. 本地 DC 不可用 → 检测故障
3. 自动切换 → 远程 DC (cn-north-1)
4. 本地 DC 恢复 → 自动切回
```

### 监控多数据中心状态

```bash
# 查看集群节点状态
curl localhost:8848/nacos/v1/core/cluster/nodes

# 响应示例
{
  "data": [
    {
      "address": "192.168.1.10:8848",
      "state": "UP",
      "extendInfo": {
        "datacenter": "cn-east-1",
        "region": "cn-east",
        "zone": "zone-a"
      }
    },
    {
      "address": "192.168.2.10:8848",
      "state": "UP",
      "extendInfo": {
        "datacenter": "cn-north-1",
        "region": "cn-north",
        "zone": "zone-a"
      }
    }
  ]
}
```

### 最佳实践

1. **网络延迟**：确保数据中心间网络延迟 < 100ms
2. **副本分布**：每个数据中心至少部署 2 个节点
3. **同步延迟**：根据业务需求调整 `cross-dc-sync-delay`
4. **监控告警**：监控跨数据中心复制延迟和失败率
5. **容灾测试**：定期进行数据中心故障切换演练

---

## 监控与指标

### Prometheus 指标端点

```
GET /nacos/metrics
```

### 可用指标

#### HTTP 指标
- `batata_http_requests_total` - HTTP 请求总数
- `batata_grpc_requests_total` - gRPC 请求总数

#### 业务指标
- `batata_config_publish_total` - 配置发布总数
- `batata_config_query_total` - 配置查询总数
- `batata_service_register_total` - 服务注册总数
- `batata_service_query_total` - 服务查询总数

#### 系统指标
- `batata_active_connections` - 活跃连接数
- `batata_uptime_seconds` - 运行时间（秒）
- `batata_cluster_size` - 集群节点数
- `batata_healthy` - 节点健康状态

### Prometheus 配置

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'batata'
    static_configs:
      - targets: ['localhost:8848']
    metrics_path: '/nacos/metrics'
```

### Grafana 仪表板

推荐监控面板：
- 请求速率和延迟
- 配置变更趋势
- 服务注册/注销事件
- 集群健康状态

---

## 客户端集成

### Java (Nacos SDK)

```xml
<dependency>
    <groupId>com.alibaba.nacos</groupId>
    <artifactId>nacos-client</artifactId>
    <version>2.3.0</version>
</dependency>
```

```java
// 配置管理
Properties properties = new Properties();
properties.put("serverAddr", "localhost:8848");
properties.put("namespace", "public");

ConfigService configService = NacosFactory.createConfigService(properties);
String config = configService.getConfig("dataId", "group", 5000);

// 服务发现
NamingService namingService = NacosFactory.createNamingService(properties);
namingService.registerInstance("serviceName", "127.0.0.1", 8080);
List<Instance> instances = namingService.getAllInstances("serviceName");
```

### Go (Consul SDK)

```go
import (
    "github.com/hashicorp/consul/api"
)

// 创建客户端
config := api.DefaultConfig()
config.Address = "localhost:8848"
client, _ := api.NewClient(config)

// 服务注册
registration := &api.AgentServiceRegistration{
    ID:      "my-service-1",
    Name:    "my-service",
    Port:    8080,
    Address: "127.0.0.1",
}
client.Agent().ServiceRegister(registration)

// 服务发现
services, _, _ := client.Health().Service("my-service", "", true, nil)
```

### Python

```python
import nacos

# Nacos 客户端
client = nacos.NacosClient(
    server_addresses="localhost:8848",
    namespace="public"
)

# 获取配置
config = client.get_config("dataId", "group")

# 注册服务
client.add_naming_instance("serviceName", "127.0.0.1", 8080)
```

### Rust

```rust
// 使用 reqwest 直接调用 HTTP API
use reqwest::Client;

let client = Client::new();

// 获取配置
let resp = client
    .get("http://localhost:8848/nacos/v1/cs/configs")
    .query(&[("dataId", "test"), ("group", "DEFAULT_GROUP")])
    .send()
    .await?;

// 注册服务
let resp = client
    .post("http://localhost:8848/nacos/v1/ns/instance")
    .form(&[
        ("serviceName", "my-service"),
        ("ip", "127.0.0.1"),
        ("port", "8080"),
    ])
    .send()
    .await?;
```

---

## 故障排除

### 常见问题

#### 1. 无法连接数据库

**症状**: 启动时报数据库连接错误

**解决方案**:
```bash
# 检查数据库连接
mysql -u user -p -h localhost batata

# 检查配置
cat conf/application.yml | grep -A3 "db:"

# 检查环境变量
echo $BATATA_DB_URL
```

#### 2. JWT 认证失败

**症状**: 401 Unauthorized 错误

**解决方案**:
```bash
# 确保密钥是 Base64 编码且至少 32 字节
echo -n "your-secret-key-at-least-32-bytes" | base64

# 更新配置
# nacos.core.auth.plugin.nacos.token.secret.key
```

#### 3. 集群节点无法同步

**症状**: 集群成员状态异常

**解决方案**:
```bash
# 检查网络连通性
ping other-node-ip
telnet other-node-ip 9849

# 检查集群配置
cat conf/cluster.conf

# 检查日志
tail -f logs/batata.log | grep -i cluster
```

#### 4. 内存使用过高

**症状**: OOM 或性能下降

**解决方案**:
```bash
# 检查 RocksDB 缓存设置
# 默认 256MB 块缓存，可根据内存调整

# 检查连接数
curl localhost:8848/nacos/metrics | grep connections

# 重启服务释放内存
systemctl restart batata
```

#### 5. 配置不生效

**症状**: 修改配置后客户端未更新

**解决方案**:
```bash
# 检查配置是否发布成功
curl "localhost:8848/nacos/v1/cs/configs?dataId=xxx&group=xxx"

# 检查客户端监听
# 确保客户端使用长轮询监听

# 检查 MD5 是否变化
curl "localhost:8848/nacos/v1/cs/configs?dataId=xxx&group=xxx&show=all"
```

### 日志分析

```bash
# 查看错误日志
tail -f logs/batata.log | grep -i error

# 查看特定模块日志
tail -f logs/batata.log | grep -i "cluster\|raft"

# 启用调试日志
export RUST_LOG=debug
./target/release/batata
```

### 健康检查

```bash
# Liveness 检查
curl localhost:8848/nacos/v1/console/health/liveness

# Readiness 检查
curl localhost:8848/nacos/v1/console/health/readiness

# 集群状态
curl localhost:8848/nacos/v1/core/cluster/nodes
```

---

## 性能调优

### JVM vs Rust 对比

| 指标 | Nacos (Java) | Batata (Rust) |
|------|--------------|---------------|
| 启动时间 | 30-60s | 1-2s |
| 内存占用 | 1-2GB | 100-200MB |
| CPU 使用 | 较高 | 较低 |
| 延迟 | 10-50ms | 1-5ms |

### 调优建议

1. **RocksDB 缓存**: 根据可用内存调整块缓存大小
2. **连接池**: 数据库连接池大小根据并发量调整
3. **Token 缓存**: 默认 10,000 条，可根据用户数调整
4. **限流配置**: 根据业务需求调整速率限制

---

## 安全建议

1. **生产环境必须修改默认 JWT 密钥**
2. **启用 HTTPS/TLS**
3. **配置防火墙，只开放必要端口**
4. **定期轮换凭据**
5. **启用审计日志**
6. **使用专用数据库账户，最小权限原则**

---

## 许可证

MIT License
