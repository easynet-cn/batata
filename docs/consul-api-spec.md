# Consul API 兼容规范

本文档定义了 Batata 需要实现的 Consul HTTP API 端点，以实现与 Consul 客户端的兼容。

## 实现优先级

| 优先级 | 模块 | 端点数 | 说明 |
|--------|------|--------|------|
| P0 | Agent Service | 5 | 核心服务注册/注销 |
| P0 | Health | 4 | 健康检查 |
| P1 | Catalog | 6 | 服务目录查询 |
| P1 | KV Store | 5 | 配置存储 |
| P2 | Agent Check | 4 | 检查管理 |
| P3 | Session | 4 | 分布式锁 |

---

## P0: Agent Service API (服务注册)

### 1. PUT /v1/agent/service/register
注册服务到本地 Agent

**请求体:**
```json
{
  "ID": "redis1",
  "Name": "redis",
  "Tags": ["primary", "v1"],
  "Address": "127.0.0.1",
  "Port": 8000,
  "Meta": {
    "redis_version": "4.0"
  },
  "EnableTagOverride": false,
  "Weights": {
    "Passing": 10,
    "Warning": 1
  },
  "Check": {
    "DeregisterCriticalServiceAfter": "90m",
    "HTTP": "http://localhost:8000/health",
    "Interval": "10s"
  }
}
```

**映射到 Nacos:**
- `ID` → `instanceId`
- `Name` → `serviceName`
- `Tags` → 存入 `metadata["consul_tags"]`
- `Address:Port` → `ip:port`
- `Meta` → `metadata`
- `Weights.Passing` → `weight`

### 2. PUT /v1/agent/service/deregister/:service_id
注销服务

**映射到 Nacos:** `DELETE /nacos/v1/ns/instance`

### 3. GET /v1/agent/services
获取本地 Agent 注册的所有服务

**响应:**
```json
{
  "redis1": {
    "ID": "redis1",
    "Service": "redis",
    "Tags": ["primary"],
    "Port": 8000,
    "Address": "192.168.1.100",
    "Weights": {
      "Passing": 10,
      "Warning": 1
    }
  }
}
```

### 4. GET /v1/agent/service/:service_id
获取单个服务详情

### 5. PUT /v1/agent/service/maintenance/:service_id
设置服务维护模式

**映射到 Nacos:** 设置 `enabled=false`

---

## P0: Health API (健康检查)

### 1. GET /v1/health/service/:service
获取服务健康状态（最常用）

**查询参数:**
- `passing` - 只返回健康实例
- `tag` - 按标签过滤
- `dc` - 数据中心

**响应:**
```json
[
  {
    "Node": {
      "ID": "node-uuid",
      "Node": "node1",
      "Address": "192.168.1.100",
      "Datacenter": "dc1"
    },
    "Service": {
      "ID": "redis1",
      "Service": "redis",
      "Tags": ["primary"],
      "Port": 8000,
      "Address": "192.168.1.100"
    },
    "Checks": [
      {
        "CheckID": "service:redis1",
        "Status": "passing",
        "Output": "HTTP GET http://localhost:8000/health: 200 OK"
      }
    ]
  }
]
```

**映射到 Nacos:** `GET /nacos/v1/ns/instance/list?healthyOnly=true`

### 2. GET /v1/health/checks/:service
获取服务的所有健康检查

### 3. GET /v1/health/state/:state
按状态获取检查 (passing/warning/critical)

### 4. GET /v1/health/node/:node
获取节点上的所有检查

---

## P1: Catalog API (服务目录)

### 1. GET /v1/catalog/services
列出所有服务

**响应:**
```json
{
  "redis": ["primary", "v1"],
  "postgresql": ["secondary"]
}
```

**映射到 Nacos:** `GET /nacos/v1/ns/service/list`

### 2. GET /v1/catalog/service/:service
获取服务的所有实例

**查询参数:**
- `tag` - 按标签过滤
- `dc` - 数据中心

### 3. GET /v1/catalog/nodes
列出所有节点

### 4. GET /v1/catalog/node/:node
获取节点详情

### 5. PUT /v1/catalog/register
直接注册到目录（绕过 Agent）

### 6. PUT /v1/catalog/deregister
直接从目录注销

---

## P1: KV Store API (配置存储)

### 1. GET /v1/kv/:key
获取配置值

**查询参数:**
- `raw` - 返回原始值（非 JSON）
- `keys` - 只返回键列表
- `recurse` - 递归获取前缀下所有键

**响应 (无 raw):**
```json
[
  {
    "Key": "config/database/host",
    "Value": "bXlzcWwuZXhhbXBsZS5jb20=",  // Base64
    "CreateIndex": 100,
    "ModifyIndex": 200,
    "LockIndex": 0,
    "Flags": 0
  }
]
```

**映射到 Nacos:** `GET /nacos/v1/cs/configs`
- Key 映射: `namespace/group/dataId` → `config/database/host`

### 2. PUT /v1/kv/:key
设置配置值

**查询参数:**
- `cas` - Check-And-Set（ModifyIndex）
- `flags` - 自定义标志

**请求体:** 配置值（原始文本）

**映射到 Nacos:** `POST /nacos/v1/cs/configs`

### 3. DELETE /v1/kv/:key
删除配置

**查询参数:**
- `recurse` - 递归删除

### 4. GET /v1/kv/:key?keys
获取前缀下的所有键（不含值）

### 5. PUT /v1/txn
事务操作（原子性多键操作）

**请求体:**
```json
[
  {
    "KV": {
      "Verb": "set",
      "Key": "config/key1",
      "Value": "dmFsdWUx"
    }
  },
  {
    "KV": {
      "Verb": "set",
      "Key": "config/key2",
      "Value": "dmFsdWUy"
    }
  }
]
```

---

## P2: Agent Check API (检查管理)

### 1. PUT /v1/agent/check/register
注册健康检查

```json
{
  "Name": "Memory utilization",
  "CheckID": "mem",
  "TTL": "30s",
  "Notes": "Check memory usage"
}
```

### 2. PUT /v1/agent/check/deregister/:check_id
注销检查

### 3. PUT /v1/agent/check/pass/:check_id
标记 TTL 检查为通过

**映射到 Nacos:** 心跳接口

### 4. PUT /v1/agent/check/fail/:check_id
标记 TTL 检查为失败

---

## P3: Session API (分布式锁)

### 1. PUT /v1/session/create
创建会话

### 2. PUT /v1/session/destroy/:session_id
销毁会话

### 3. GET /v1/session/info/:session_id
获取会话信息

### 4. PUT /v1/kv/:key?acquire=:session_id
获取锁

---

## 数据模型映射

### Namespace/Datacenter 映射

| Consul | Nacos | 说明 |
|--------|-------|------|
| `dc` (Datacenter) | `namespace` | 数据隔离边界 |
| - | `group` | Consul 无对应，存入 Meta |
| `Node` | 集群成员 | 节点概念 |

### Service 映射

| Consul 字段 | Nacos 字段 | 转换逻辑 |
|-------------|-----------|----------|
| `ID` | `instanceId` | 直接映射 |
| `Name` | `serviceName` | 直接映射 |
| `Address` | `ip` | 直接映射 |
| `Port` | `port` | 直接映射 |
| `Tags` | `metadata["consul_tags"]` | JSON 数组字符串 |
| `Meta` | `metadata` | 直接合并 |
| `Weights.Passing` | `weight` | 直接映射 |
| `EnableTagOverride` | `metadata["enableTagOverride"]` | 存入 metadata |

### KV 映射

| Consul KV | Nacos Config | 转换逻辑 |
|-----------|--------------|----------|
| `Key` | `namespace/group/dataId` | 路径解析 |
| `Value` | `content` | Base64 解码 |
| `ModifyIndex` | 版本号 | 直接映射 |
| `Flags` | `metadata["consul_flags"]` | 存入 metadata |

---

## 实现路径

### Phase 1: 核心服务发现 (2-3 周)

```
src/
├── api/
│   └── consul/
│       ├── mod.rs
│       ├── agent.rs      # Agent Service/Check API
│       ├── health.rs     # Health API
│       └── model.rs      # Consul 数据模型
```

**端点:**
- `PUT /v1/agent/service/register`
- `PUT /v1/agent/service/deregister/:id`
- `GET /v1/agent/services`
- `GET /v1/health/service/:service`

### Phase 2: 目录和 KV (2 周)

**端点:**
- `GET /v1/catalog/services`
- `GET /v1/catalog/service/:service`
- `GET /v1/kv/:key`
- `PUT /v1/kv/:key`
- `DELETE /v1/kv/:key`

### Phase 3: 健康检查完善 (1 周)

**端点:**
- `PUT /v1/agent/check/register`
- `PUT /v1/agent/check/pass/:id`
- `PUT /v1/agent/check/fail/:id`

### Phase 4: 高级功能 (可选)

- Session/Lock API
- ACL API
- Connect/Service Mesh

---

## 配置示例

```yaml
# application.yml
server:
  consul:
    enabled: true
    port: 8500
    # 数据中心映射到命名空间
    datacenter_mapping:
      dc1: public
      dc2: dev
    # 默认组
    default_group: DEFAULT_GROUP
```

---

## 客户端兼容性

已测试/目标客户端：
- [x] Consul CLI
- [ ] consul-template
- [ ] Spring Cloud Consul
- [ ] go-micro/consul
- [ ] Fabio
