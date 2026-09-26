# API Reference

Batata exposes Nacos V2/V3, Consul, Apollo and console APIs. Protocol-specific
compatibility notes live under [`docs/compat/`](compat/).

## Nacos Configuration API (V2)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/nacos/v2/cs/config` | GET | Get configuration |
| `/nacos/v2/cs/config` | POST | Create/update configuration |
| `/nacos/v2/cs/config` | DELETE | Delete configuration |

## Nacos Configuration API (Console V3)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v3/console/cs/config` | GET | Get config detail (console) |
| `/v3/console/cs/config/list` | GET | List configs with pagination |
| `/v3/console/cs/config/export` | GET | Export configs (ZIP) |
| `/v3/console/cs/config/import` | POST | Import configs (ZIP) |

## Nacos Naming API (V2)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/nacos/v2/ns/instance` | POST | Register instance |
| `/nacos/v2/ns/instance` | PUT | Update instance |
| `/nacos/v2/ns/instance` | DELETE | Deregister instance |
| `/nacos/v2/ns/instance/list` | GET | List service instances |

## Consul API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/agent/service/register` | PUT | Register service |
| `/v1/agent/service/deregister/{id}` | PUT | Deregister service |
| `/v1/health/service/{service}` | GET | Get service health |
| `/v1/catalog/services` | GET | List all services |
| `/v1/kv/{key}` | GET | Get KV value |
| `/v1/kv/{key}` | PUT | Set KV value |
| `/v1/kv/{key}` | DELETE | Delete KV value |
| `/v1/kv/export` | GET | Export KV (JSON) |
| `/v1/kv/import` | PUT | Import KV (JSON) |

## Console API (V3)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v3/console/namespace/list` | GET | List namespaces |
| `/v3/console/namespace` | POST | Create namespace |
| `/v3/console/core/cluster/nodes` | GET | List cluster nodes |
| `/v3/console/core/cluster/health` | GET | Get cluster health |

## Configuration Import/Export

### Nacos Format (ZIP)

```bash
# Export configurations
curl -O "http://localhost:8081/v3/console/cs/config/export?namespaceId=public"

# Import with conflict policy (ABORT, SKIP, OVERWRITE)
curl -X POST "http://localhost:8081/v3/console/cs/config/import?namespaceId=public&policy=OVERWRITE" \
  -F "file=@export.zip"
```

### Consul Format (JSON)

```bash
# Export KV store
curl "http://localhost:8848/v1/kv/export?namespaceId=public" > configs.json

# Import KV store
curl -X PUT "http://localhost:8848/v1/kv/import?namespaceId=public" \
  -H "Content-Type: application/json" \
  -d @configs.json
```

## Client SDK Examples

Batata is compatible with existing Nacos, Consul and Apollo client SDKs.

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
// Apollo client reads config from the Apollo-compatible Config Service on port 8080
Config config = ConfigService.getConfig("application", "default", 5000);
String value = config.getProperty("key", "default");
```

```java
// Apollo OpenAPI (admin operations) — uses the OpenAPI access token
ApolloOpenApiClient client = ApolloOpenApiClient.newBuilder()
    .withPortalUrl("http://localhost:8080")
    .withToken("admin")
    .build();
OpenAppDTO app = client.createApp(new OpenAppDTO().setName("demo").setAppId("demo"));
```
