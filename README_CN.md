# Batata

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/easynet-cn/batata/ci.yml?branch=main)](https://github.com/easynet-cn/batata/actions)

**Batata** 是一个基于 Rust 实现的高性能服务发现、配置管理和服务管理平台。
它是 [Nacos](https://nacos.io/)（V2/V3）的替代方案，并额外兼容
[Consul](https://www.consul.io/) 与 [Apollo](https://github.com/apolloconfig/apollo)。

[English Documentation](README.md)

## 为什么选择 Batata

- **Rust 原生实现** —— 启动约 1-2 秒、内存约 50-100MB（Java 版 Nacos 为 30-60 秒、1-2GB）
- **开箱兼容** —— 兼容 Nacos V2/V3 API，并提供 Consul、Apollo 兼容层
- **零依赖启动** —— 嵌入式 RocksDB 模式无需外部数据库
- **生产可用** —— 实现 Nacos 约 98% 的功能；支持 Raft 集群、RBAC、限流

## 特性

**核心能力**

- 配置管理：实时更新、历史与回滚、灰度发布
- 服务发现：注册、健康检查、订阅推送
- 命名空间隔离：多租户资源隔离
- 集群模式：Raft 共识（CP）与 Distro 协议（AP）

**协议兼容**

- **Nacos** —— 完全兼容 V2、V3 API（V1 有意不支持）
- **Consul** —— 兼容 Agent、Health、Catalog、KV、ACL API
- **Apollo** —— 兼容 Config Service（`/configs`、`/configfiles`、`/notifications/v2`、`/releaseMessage`）、Admin Service 与 OpenAPI；现有 Apollo SDK 可直接使用，集群模式下写操作通过 Raft 复制。详见 [`docs/compat/apollo/`](docs/compat/apollo/)
- **gRPC** —— 面向 SDK 客户端的高性能双向流式通信

**运维能力**

- JWT 认证 + RBAC，支持 LDAP、OAuth2/OIDC
- 限流、熔断、审计日志、分布式锁
- 配置导入/导出（Nacos ZIP、Consul JSON）
- 服务网格 xDS（EDS/CDS/LDS/RDS/ADS）、多数据中心复制、DNS 服务发现
- Prometheus 指标与 OpenTelemetry 链路追踪

完整功能对比见 [`docs/compat/nacos/feature-comparison.md`](docs/compat/nacos/feature-comparison.md)

## Web 控制台（batata-ui）

Web 控制台是独立的前端项目：
[**easynet-cn/batata-ui**](https://github.com/easynet-cn/batata-ui)

- 构建前端并将产物放入 `console-ui/` 目录，由 console server 托管
  （由 `batata.console.ui.enabled` 控制，默认开启）
- 控制台 UI 在 `merged`（默认）和 `console` 部署模式下可用
- 启动后访问 **`http://localhost:8081/`**

端口按职责分离——控制台端口只提供 UI，不暴露 Nacos 兼容的数据接口：

| 端口 | 角色 | 用途 |
|------|------|------|
| `8081` | 控制台 | Web 控制台 UI + 控制台接口 |
| `8848` | SDK HTTP | Nacos 兼容 REST API |
| `9848` | SDK gRPC | 客户端 SDK 通信 |
| `9849` | 集群 gRPC | 节点间 / Raft 通信 |

## 快速开始

嵌入式 RocksDB 模式，无需数据库：

```bash
git clone https://github.com/easynet-cn/batata.git
cd batata

cargo build --release -p batata-server
./target/release/batata-server --batata.sql.init.platform=embedded

# 或使用启动脚本
./scripts/start-embedded.sh
```

首次启动后初始化管理员用户：

```bash
curl -X POST http://localhost:8848/nacos/v3/auth/user/admin \
  -d "username=nacos&password=nacos"
```

然后打开控制台：**http://localhost:8081/**。

MySQL / PostgreSQL 配置见 [`docs/database.md`](docs/database.md)

### 默认端口

| 端口 | 服务 | 描述 |
|------|------|------|
| 8848 | 主 HTTP API | Nacos 兼容 API |
| 8081 | 控制台 HTTP API | Web 管理控制台 |
| 8080 | Apollo HTTP API | Apollo 兼容的 Config/Admin/OpenAPI（集群模式下每节点使用独立端口） |
| 9848 | SDK gRPC | 客户端 SDK 通信 |
| 9849 | 集群 gRPC | 节点间通信 |
| 15010 | xDS gRPC | 服务网格 xDS/ADS 协议 |

## 兼容性

| 协议 | 支持度 | 说明 |
|------|--------|------|
| Nacos V2 / V3 | 完整 | V1 有意不支持 |
| Nacos gRPC | 完整 | 双向流式通信 |
| Consul | 完整 | Agent、Health、Catalog、KV、ACL |
| Apollo | ~81% | Config/Admin/OpenAPI 已移植 —— [`docs/compat/apollo/`](docs/compat/apollo/) |

客户端 SDK 示例见 [`docs/api.md`](docs/api.md#client-sdk-examples)

## 配置

主配置文件：`conf/application.yml`（已完整注释，为配置唯一真源）。
配置项使用扁平的 `batata.*` 前缀，可通过 `BATATA_*` 环境变量
或 `--batata.<key>=<value>` 命令行参数覆盖。

常用配置项：

| 配置项 | 默认值 | 描述 |
|--------|--------|------|
| `batata.server.main.port` | `8848` | SDK HTTP 端口 |
| `batata.console.port` | `8081` | 控制台端口 |
| `batata.deployment.type` | `merged` | `merged` / `server` / `console` / `serverWithMcp` |
| `batata.sql.init.platform` | _(空)_ | `mysql`、`postgresql`，或留空（= 嵌入式） |
| `batata.core.auth.enabled` | `true` | 是否启用认证 |

完整配置参考见 [`docs/configuration.md`](docs/configuration.md)

## 部署

| 模式 | 参数 | 描述 |
|------|------|------|
| `merged`（默认） | `-d merged` | 控制台（8081）与主服务（8848）同进程 |
| `server` | `-d server` | 仅主服务，无控制台 |
| `console` | `-d console` | 仅控制台，连接远程服务 |
| `serverWithMcp` | `-d serverWithMcp` | 主服务 + MCP 注册中心（9080） |

容器、Compose 与 Kubernetes 部署见 [`docs/deployment.md`](docs/deployment.md)

## 文档

| 主题 | 位置 |
|------|------|
| 架构与 crate | [`docs/architecture.md`](docs/architecture.md) |
| 配置 | [`docs/configuration.md`](docs/configuration.md) |
| API 参考 | [`docs/api.md`](docs/api.md) |
| 部署 | [`docs/deployment.md`](docs/deployment.md) |
| 数据库与迁移 | [`docs/database.md`](docs/database.md) |
| 客户端 SDK | [`docs/api.md`](docs/api.md#client-sdk-examples) |
| 监控（指标） | [`docs/configuration.md`](docs/configuration.md#metrics) |
| 开发与发布 | [`docs/development.md`](docs/development.md) |
| 与 Nacos 功能对比 | [`docs/compat/nacos/feature-comparison.md`](docs/compat/nacos/feature-comparison.md) |
| 协议兼容性 | [`docs/compat/`](docs/compat/) |
| 路线图 | [`docs/roadmap.md`](docs/roadmap.md) |

## 安全

请勿通过公开 issue 报告安全漏洞。
报告方式与响应时限见 [`SECURITY.md`](SECURITY.md)。

## 贡献

参见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。

## 许可证

基于 Apache-2.0 许可证 —— 详见 [LICENSE](LICENSE)。

## 致谢

- [Nacos](https://nacos.io/) - 原始设计与 API 规范
- [Consul](https://www.consul.io/) - KV 存储与服务发现 API 设计
- [Apollo](https://github.com/apolloconfig/apollo) - Config Service / Admin Service / OpenAPI 设计
- [batata-ui](https://github.com/easynet-cn/batata-ui) - Web 控制台前端
- [OpenRaft](https://github.com/datafuselabs/openraft) - Raft 共识实现
- [SeaORM](https://www.sea-ql.org/SeaORM/) - 异步 ORM 框架
- [Actix-web](https://actix.rs/) - 高性能 Web 框架
- [Tonic](https://github.com/hyperium/tonic) - gRPC 实现
