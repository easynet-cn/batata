# Batata 部署与使用指南

Batata 是一个 Rust 实现的 Nacos 兼容服务，提供服务发现、配置管理和集群协调功能。同时兼容 Consul API。

## 目录

- [系统要求](#系统要求)
- [快速开始](#快速开始)
- [构建](#构建)
- [配置](#配置)
- [部署模式](#部署模式)
- [API 参考](#api-参考)
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
      - ./conf/mysql-schema.sql:/docker-entrypoint-initdb.d/init.sql
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
