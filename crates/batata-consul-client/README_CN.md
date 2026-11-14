# batata-consul-client

[English](README.md) | 中文

用于 Consul HTTP API 的 Rust 客户端，为 [Batata](https://github.com/easynet-cn/batata)
构建，同样可以直接连接 HashiCorp Consul。它与官方 Go SDK
（`github.com/hashicorp/consul/api`）保持方法级对应：一个 Rust 模块对应一个
`api/*.go` 文件。

该 crate **不依赖 workspace 内部的其他 crate**，仅依赖 `reqwest`、`serde`、
`tokio`、`tracing` 及少量工具库，可以单独使用。

## 功能

- **完整的 HTTP API 面**：ACL、agent、catalog、config entry、connect、
  coordinate、debug、discovery chain、event、exported/imported services、
  health、KV、lock/semaphore、namespace、operator、partition、peering、
  prepared query、raw、session、snapshot、status、txn、watch。
- **扁平化类型方法**：所有调用都是 `ConsulClient` 上的方法，例如 `kv_get`、
  `catalog_services`、`agent_service_register`，配合强类型模型与
  `QueryMeta` / `WriteMeta` 返回值。
- **阻塞查询**：`QueryOptions::wait_index` / `wait_time` 与
  `QueryMeta::last_index` 对应 Consul 长轮询契约，`X-Consul-Index`、
  `X-Consul-KnownLeader`、`X-Consul-LastContact` 及缓存响应头都会被解析。
- **鉴权、TLS 与故障转移**：ACL token（`X-Consul-Token`，静态或每次请求重读
  `token_file`）、自定义 CA / mTLS / 跳过校验、多地址轮转、429/5xx 指数退避重试。
- **可观测性**：通过 `ConsulClient::metrics` 暴露客户端侧指标。

`KVPair::value` 遵循 Consul 的 wire 格式，保存的是 **base64 编码**的字节；
读取用 `value_bytes()` / `value_str()` 解码，写入时先自行 base64 编码
（`kv_put` 会解码后发送原始字节）。

## 安装

```toml
[dependencies]
batata-consul-client = "1.0.0"
tokio = { version = "1", features = ["rt", "macros"] }
```

## 快速开始

```rust,no_run
use batata_consul_client::{
    ConsulClient, ConsulClientConfig, KVPair, QueryOptions, WriteOptions,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Batata 在 SDK HTTP 端口 8848 上提供 Consul API。
    let config = ConsulClientConfig::new("http://127.0.0.1:8848");
    let client = ConsulClient::new(config)?;

    let (leader, _) = client.status_leader(&QueryOptions::default()).await?;
    println!("leader: {leader}");

    // "key=value" 的 base64，符合 Consul JSON wire 格式。
    let pair = KVPair {
        key: "example/config".to_string(),
        value: Some("a2V5PXZhbHVl".to_string()),
        ..Default::default()
    };
    client.kv_put(&pair, &WriteOptions::default()).await?;

    let (stored, meta) = client
        .kv_get("example/config", &QueryOptions::default())
        .await?;
    println!(
        "value: {:?}, index: {}",
        stored.and_then(|p| p.value_str()),
        meta.last_index
    );

    client
        .kv_delete("example/config", &WriteOptions::default())
        .await?;
    Ok(())
}
```

`QueryOptions::default()` 使用客户端配置的 datacenter、token 与 namespace。
读操作通常返回 `(T, QueryMeta)`，写操作通常返回 `(bool, WriteMeta)`；KV
键不存在时是 `Ok((None, _))`，不是错误。

## ACL Token 与 TLS

```rust,no_run
use batata_consul_client::{ConsulClient, ConsulClientConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ConsulClientConfig::new("https://consul.example.com:8501")
        .with_token("acl-token")
        // mTLS：CA、客户端证书、客户端私钥（均为 PEM）。
        .with_mtls(
            "/path/to/ca.pem",
            "/path/to/client.pem",
            "/path/to/client-key.pem",
        );
    let client = ConsulClient::new(config)?;
    drop(client);
    Ok(())
}
```

`with_tls(ca)` 只安装私有 CA、不携带客户端身份。`token_file` 会在每次请求时
重读，便于轮换短期 token。`tls_insecure_skip_verify` 仅用于开发环境。
PEM 文件缺失或格式错误会在 `ConsulClient::new` 时报错，不会静默退回明文。

## 配置项

| 构造器 / 字段 | 说明 | 默认值 |
|---------------|------|--------|
| `ConsulClientConfig::new(addr)` | 主 agent 的 HTTP(S) 地址 | `http://127.0.0.1:8500` |
| `with_addresses(vec)` | 多地址故障转移轮转 | 单地址 |
| `with_token(token)` / `with_token_file(path)` | ACL token | 空 / 关闭 |
| `with_datacenter(dc)` | datacenter 覆盖 | agent 默认 |
| `with_wait_time(d)` | 阻塞查询默认等待 | 关闭 |
| `with_timeouts(connect, read)` | 连接 / 读取超时 | 5s / 30s |
| `with_namespace(ns)` / `with_partition(p)` | Consul Enterprise 作用域 | 空 |
| `with_tls(ca)` / `with_mtls(ca, cert, key)` | TLS / 双向 TLS | 关闭 |
| `with_retry(max, base_ms)` | 重试次数与退避基数 | 2 / 100ms |

单次请求的覆盖项在 `QueryOptions` / `WriteOptions` 上；token 优先级为
「请求级覆盖 > `token_file` > 客户端默认值」。

## 示例

```bash
cargo run -p batata-consul-client --example kv_example
```

示例默认连接 `127.0.0.1:8500`；连接 Batata 时把地址改为
`http://127.0.0.1:8848`。兼容性测试套件位于
[`sdk-tests/batata-consul-client-tests`](https://github.com/easynet-cn/batata/tree/main/sdk-tests/batata-consul-client-tests)。

## 从 0.0.x 迁移

`1.0.0` 是对独立仓库 `easynet-cn/batata-consul-client` 原型的**完全重写**，
与 `0.0.1` / `0.0.2` 不兼容。自 `1.0.0` 起，crate 由
[`easynet-cn/batata`](https://github.com/easynet-cn/batata) workspace 构建并发布。

主要差异：

- `Client` / `Config` 改为 `ConsulClient` / `ConsulClientConfig`。
- 子 API 访问器（`client.kv()`、`client.agent()`、`client.health()` 等）改为
  `ConsulClient` 上的扁平方法（`kv_get`、`kv_put`、
  `agent_service_register`、`health_service` 等）。
- `api::*` 模块改为顶层模块 `kv` / `agent` / `health` / `catalog` /
  `session` 等；类型由 `model` 在 crate 根重导出。
- `KVPair::value_string()` 改为 `value_str()`；`kv.put_string(...)` 改为构造
  带 base64 `value` 的 `KVPair` 后调用 `kv_put`。
- `AgentServiceRegistration` 构造器及基于 `urlencoding` 的辅助方法已移除，
  改用 `model` 中的强类型结构体（`Default` + 字段赋值）。
- 整个 workspace 升级到 edition 2024 / 最低 Rust 1.89。

已有 `Cargo.lock` 仍会锁定旧版本，请按真实的 API 迁移来规划升级，而不是只改
版本号。详见 [CHANGELOG.md](CHANGELOG.md)。

## 相关 crate

- [`batata-client`](https://crates.io/crates/batata-client)：基于 gRPC 的
  Nacos 兼容配置与服务发现 SDK。
- [`batata-maintainer-client`](https://crates.io/crates/batata-maintainer-client)：
  类型化运维 API 客户端。
- [`batata-api`](https://crates.io/crates/batata-api)：Nacos 兼容面的共享 wire
  模型；Consul SDK 不依赖它。

## 环境要求

Rust 1.89 或更高（edition 2024），需要 Tokio runtime。该 crate 自身依赖闭包的
下限是 Rust 1.88，但为与其余三个 Batata SDK crate 保持统一，声明为 1.89。
TLS 使用 rustls 后端，无需系统 OpenSSL 开发包。

## 许可证

Apache-2.0，见 [LICENSE](LICENSE)。
