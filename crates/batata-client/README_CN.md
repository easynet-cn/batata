# Batata Client

[Batata](https://github.com/easynet-cn/batata)（兼容 Nacos 的服务发现与配置管理服务器）的 Rust 客户端库。

[![Crates.io](https://img.shields.io/crates/v/batata-client.svg)](https://crates.io/crates/batata-client)
[![Documentation](https://docs.rs/batata-client/badge.svg)](https://docs.rs/batata-client)
[![License](https://img.shields.io/crates/l/batata-client.svg)](LICENSE)

[English](README.md) | 中文

## 功能特性

- **配置管理**
  - 通过 gRPC 获取、发布、删除配置
  - 支持配置类型与 CAS（MD5 比较交换）发布
  - 回调监听配置变更，或接收字段级变更差异
  - 本地缓存与 MD5 校验、配置过滤器与加密钩子
- **服务发现**
  - 注册、注销、批量注册服务实例
  - 查询全部实例、健康实例，或按权重选取单个健康实例
  - 订阅 / 取消订阅、模糊监听、服务列表分页
  - 临时实例自动心跳，断线重连后自动 redo
- **认证支持**
  - 用户名 / 密码 JWT 认证，自动刷新令牌
  - AccessKey / SecretKey（HMAC）签名认证
  - 运维接口的 Server Identity 请求头（通过 `batata-maintainer-client`）
- **高可用与传输**
  - gRPC 双向流 + 类型化的 HTTP 运维接口
  - 多服务器地址故障转移、重试、地址服务器动态刷新
  - 代理模式（关闭 / 系统 / 自定义）、超时与客户端标签
  - gRPC 通道 TLS / mTLS（自定义 CA 或系统根证书、SNI 覆盖、客户端证书），
    HTTPS 运维接口支持私有 CA
  - 本地文件缓存容灾、空推送保护
  - 协议模型由 [`batata-api`](https://crates.io/crates/batata-api) 提供

## 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
batata-client = "1.0.0"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

以下示例使用 `anyhow::Result` 处理错误；若项目尚未依赖，请添加 `anyhow = "1"`。
`Instance` 已在本 crate 重新导出，只有使用其他协议模型时才需要直接依赖
`batata-api`。

## 快速开始

### 基本用法

```rust,no_run
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 服务器地址使用 "host:port"（也接受 "http://" 前缀）。
    let config = ClientConfig::new("127.0.0.1:8848")
        .with_auth("nacos", "nacos")
        .namespace("public");

    // 连接是懒加载的：首次获取某个 service 时才建立。
    let mut client = BatataClient::new(config).await?;

    let config_svc = client.config_service().await?;
    config_svc
        .publish_config("my-config", "DEFAULT_GROUP", "", "key=value")
        .await?;
    let content = config_svc
        .get_config("my-config", "DEFAULT_GROUP", "")
        .await?;
    println!("配置内容: {content}");

    let naming_svc = client.naming_service().await?;
    naming_svc
        .register_instance_simple("my-service", "127.0.0.1", 8080)
        .await?;
    let instances = naming_svc
        .select_instances("", "DEFAULT_GROUP", "my-service", true)
        .await?;
    println!("健康实例数: {}", instances.len());

    client.shutdown();
    Ok(())
}
```

`tenant` / `namespace` 传空字符串表示 public 命名空间；
`ClientConfig::namespace(...)` 用于 gRPC 连接初始化。

### 使用认证

```rust,no_run
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 用户名 / 密码（JWT，自动刷新）。
    let config = ClientConfig::new("127.0.0.1:8848").with_auth("nacos", "nacos");
    let jwt_client = BatataClient::new(config).await?;

    // AccessKey / SecretKey（HMAC 签名）。
    let config =
        ClientConfig::new("127.0.0.1:8848").with_access_key("your-ak", "your-sk");
    let aksk_client = BatataClient::new(config).await?;

    drop((jwt_client, aksk_client));
    Ok(())
}
```

### 监听配置变更

```rust,no_run
use std::sync::Arc;
use batata_client::config::listener::FnConfigChangeListener;
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client =
        BatataClient::new(ClientConfig::new("127.0.0.1:8848")).await?;
    let config_svc = client.config_service().await?;

    let listener = Arc::new(FnConfigChangeListener::new(|info| {
        println!("{} 已变更: {}", info.data_id, info.content);
    }));
    config_svc
        .add_listener("my-config", "DEFAULT_GROUP", "", listener)
        .await?;

    // 回调在 Tokio runtime 线程上同步执行，切勿在其中阻塞。
    // add_change_event_listener 还可提供解析后的字段级差异。
    Ok(())
}
```

### 订阅服务变更

```rust,no_run
use std::sync::Arc;
use batata_client::naming::listener::FnEventListener;
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client =
        BatataClient::new(ClientConfig::new("127.0.0.1:8848")).await?;
    let naming_svc = client.naming_service().await?;

    let listener = Arc::new(FnEventListener::new(|event| {
        println!("{} 当前有 {} 个实例", event.service_name, event.instances.len());
    }));
    let initial = naming_svc
        .subscribe("", "DEFAULT_GROUP", "my-service", "", listener)
        .await?;
    println!("初始实例数: {}", initial.len());

    // unsubscribe 使用同样的 namespace/group/service/clusters 四元组。
    Ok(())
}
```

### 搜索配置

配置搜索属于 HTTP 运维接口，不是 gRPC SDK 操作，需使用类型化的 API 客户端：

```rust,no_run
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client = BatataClient::new(
        ClientConfig::new("127.0.0.1:8848").with_auth("nacos", "nacos"),
    )
    .await?;
    let api = client.api_client().await?;
    let page = api
        .config_list(1, 10, "", "", "DEFAULT_GROUP", "", "", "", "")
        .await?;

    println!("共找到 {} 个配置", page.total_count);
    for item in &page.page_items {
        println!("- {}: {}", item.data_id, item.group_name);
    }
    Ok(())
}
```

### 自定义服务实例

1.0.0 没有单独的「更新实例」方法：构造 `Instance` 后，用相同的
`ip` / `port` / `cluster_name` 重新注册即可。

```rust,no_run
use batata_client::{BatataClient, ClientConfig, Instance};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client =
        BatataClient::new(ClientConfig::new("127.0.0.1:8848")).await?;
    let naming_svc = client.naming_service().await?;

    let mut instance = Instance::new("127.0.0.1".to_string(), 8080);
    instance.weight = 2.0;
    instance.enabled = false;
    instance
        .metadata
        .insert("version".to_string(), "2.0.0".to_string());

    naming_svc
        .register_instance("", "DEFAULT_GROUP", "my-service", instance)
        .await?;
    Ok(())
}
```

### 本地缓存与故障转移

```rust,no_run
use batata_client::{BatataClient, ClientConfig, ProxyConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ClientConfig::new("127.0.0.1:8848")
        // 服务发现容灾磁盘缓存。
        .naming_cache_dir("/tmp/batata-cache")
        .naming_load_cache_at_start(true)
        .naming_push_empty_protection(true)
        // 系统代理与地址服务器动态刷新。
        .proxy(ProxyConfig::SystemProxy)
        .address_server("http://127.0.0.1:8080/nacos/serverlist");
    let client = BatataClient::new(config).await?;
    drop(client);
    Ok(())
}
```

## 配置选项

| 方法 / 字段 | 说明 | 默认值 |
|-------------|------|--------|
| `ClientConfig::new(addr)` | 单个服务器 `host:port` | `127.0.0.1:8848` |
| `with_server_addrs(vec)` | 多服务器地址，故障转移 | - |
| `context_path(path)` | HTTP 接口上下文路径 | `/nacos` |
| `auth_endpoint(path)` | 登录接口 | `/v3/auth/user/login` |
| `with_auth(u, p)` | 用户名 / 密码 JWT 认证 | 关闭 |
| `with_access_key(ak, sk)` | HMAC AK/SK 认证 | 关闭 |
| `proxy(mode)` | `NoProxy` / `SystemProxy` / `Custom(url)` | `NoProxy` |
| `connect_timeout(d)` | 连接超时 | 5s |
| `request_timeout(d)` | 请求超时 | 30s |
| `grpc_port_offset(n)` | gRPC 端口 = HTTP 端口 + n | 1000 |
| `namespace(ns)` | 命名空间 / 租户 ID（空 = public） | 空 |
| `label(k, v)` | 连接建立时上报的客户端标签 | - |
| `address_server(url)` | 动态服务器列表接口 | 关闭 |
| `server_list_refresh_interval(d)` | 刷新周期 | 30s |
| `server_max_failures(n)` | 判定节点不健康的连续失败次数 | 3 |
| `max_retries(n)` / `retry_initial_delay(d)` | 重试策略 | 3 / 500ms |
| `redo_delay(d)` | 重连后的重放延迟 | 3s |
| `config_long_poll_timeout(d)` | 配置监听超时 | 30s |
| `naming_cache_dir(path)` / `naming_load_cache_at_start(b)` | 磁盘容灾缓存 | 临时目录 / false |
| `naming_push_empty_protection(b)` | 拒绝空实例推送 | false |
| `with_tls(ca_path)` | 启用 gRPC TLS；传 `None` 使用系统根证书 | 关闭 |
| `with_mtls(ca, cert, key)` | 启用 gRPC TLS 并携带客户端证书 | 关闭 |
| `tls_domain(domain)` | gRPC SNI / 证书名称覆盖 | 取自地址 |

## TLS 与 mTLS

先在服务端 `conf/application.yml` 打开 SDK gRPC 端口的 TLS：

```yaml
batata.remote.server.grpc.sdk.tls.enabled: true
batata.remote.server.grpc.tls.cert.path: /path/to/server.crt
batata.remote.server.grpc.tls.key.path: /path/to/server.key
# 双向认证时再打开：
batata.remote.server.grpc.tls.mtls.enabled: true
batata.remote.server.grpc.tls.ca.cert.path: /path/to/ca.crt
```

客户端使用同一个 CA。gRPC 端口仍由 HTTP 端口推导（默认 `+1000`），改变的
只是通道协议从 `http://` 变为 `https://`。

```rust,no_run
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 使用私有 CA 校验服务端；传 None 则使用系统根证书。
    let config = ClientConfig::new("127.0.0.1:8848")
        .with_tls(Some("/path/to/ca.crt"))
        // 用 IP 连接但证书签给域名时，需要指定 SNI / 校验名称。
        .tls_domain("batata.example.com")
        .with_auth("nacos", "nacos");

    let mut client = BatataClient::new(config).await?;
    let config_svc = client.config_service().await?;
    let _ = config_svc.get_config("app.yaml", "DEFAULT_GROUP", "").await?;
    Ok(())
}
```

双向认证时补充客户端证书与私钥：

```rust,no_run
use batata_client::ClientConfig;

let config = ClientConfig::new("127.0.0.1:8848")
    .with_mtls(
        Some("/path/to/ca.crt"),
        "/path/to/client.crt",
        "/path/to/client.key",
    )
    .tls_domain("batata.example.com");
# let _ = config;
```

同一个 CA 与客户端证书同样会用于 JWT 登录和类型化 HTTP 运维请求。HTTP URL
使用 `server_addrs` 中的协议前缀，因此 HTTP 侧由代理终结 TLS 时，请显式传入
`https://host:port`。直接构造 `GrpcClient` 时可使用
`GrpcClientConfig::with_tls` / `with_mtls` / `with_tls_domain`。PEM 文件缺失或
格式错误会在配置阶段直接报错，不会静默回退到明文。

## 运行示例

示例需要已启动并使用 `nacos` / `nacos` 凭证的 Batata 服务器：

```bash
cargo run -p batata-client --example config_example
cargo run -p batata-client --example naming_example
```

完整的 SDK 兼容性测试位于
[`sdk-tests/batata-client-tests`](https://github.com/easynet-cn/batata/tree/main/sdk-tests/batata-client-tests)。

## 从 0.0.x 迁移

`1.0.0` 是**完全重写**，不是可以直接升级的小版本。`0.0.1` 与 `0.0.2`
来自独立仓库 `easynet-cn/batata-client`，现已被 `1.0.0` 取代。自 `1.0.0`
起，crate 在
[`easynet-cn/batata`](https://github.com/easynet-cn/batata) workspace 中构建与发布。

主要差异：

- `BatataClient::builder()` 改为
  `BatataClient::new(ClientConfig::new(...).with_auth(...).namespace(...))`。
- `config_service()` / `naming_service()` 现在是 `async` 且返回 `Result`；
  `shutdown()` 是同步方法。
- 配置接口增加了显式的 `tenant` 参数，返回 `Result<String>` / `Result<bool>`；
  `search_config` 移至类型化 HTTP API 客户端。
- `AuthManager`、`Credentials`、`FileCache`、`TlsConfig`、
  `common::constants::*` 等导出不再存在；认证、代理、缓存统一由
  `ClientConfig` 配置。TLS 改为 `ClientConfig::with_tls` / `with_mtls` /
  `tls_domain`，且已真正作用于 gRPC 通道（见「TLS 与 mTLS」）。
- 协议模型拆到独立的 `batata-api` crate（`Instance` 为方便起见在此重新导出）。
- gRPC 生成代码已签入 `batata-api`，编译客户端不再需要 `protoc` 或 `build.rs`。

旧版本仍会被已有 `Cargo.lock` 锁定，请按 API 迁移来规划升级，而不是只改
版本号。详见 [CHANGELOG.md](CHANGELOG.md)。

## 环境要求

Rust 1.89 及以上（edition 2024），需要 Tokio runtime。下限为 1.89 是因为运行时依赖
`uuid` 1.29 要求 Rust 1.89；SDK 使用的 let chains 则在 Rust 1.88 才于 2024 edition
稳定。gRPC 与 HTTP 的 TLS 均使用 rustls 后端，无需系统 OpenSSL 开发包。

## 已知限制

- TLS 保护的是 gRPC 通道；HTTP 登录 / 运维 URL 沿用 `server_addrs` 中的协议前缀。
  HTTP 侧走 TLS 时请显式传入 `https://host:port`。
- 配置搜索、历史、克隆属于运维 / console 接口，应使用
  `BatataClient::api_client()` 或 `batata-maintainer-client`，而非 gRPC 配置服务。
- 没有 `update_instance` 方法；请以相同实例标识重新注册（见上文「自定义服务实例」）。

## 许可证

Apache-2.0，参见 [LICENSE](LICENSE)。
