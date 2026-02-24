# Batata Client Tests

本目录包含 Batata Client 的单元测试和集成测试。

## 测试结构

- `unit_test.rs` - 单元测试，不需要运行服务器
- `integration_test.rs` - 集成测试，需要运行 Batata 服务器

## 单元测试

单元测试可以独立运行，不需要任何外部依赖。它们测试各个模块的功能，包括：

### HTTP 客户端配置测试
- `test_http_config_default` - 测试默认配置
- `test_http_config_builder` - 测试构建器模式
- `test_http_config_multiple_servers` - 测试多服务器配置

### gRPC 客户端配置测试
- `test_grpc_config_default` - 测试默认 gRPC 配置
- `test_grpc_config_custom` - 测试自定义 gRPC 配置

### 模型序列化测试
- `test_namespace_serialization` - Namespace 模型序列化
- `test_config_basic_info_serialization` - ConfigBasicInfo 序列化
- `test_config_all_info_serialization` - ConfigAllInfo 序列化
- `test_member_serialization` - Member 模型序列化
- `test_page_serialization` - Page 模型序列化
- `test_instance_info_serialization` - InstanceInfo 序列化
- `test_api_response_generic` - ApiResponse 泛型支持

### 限流器测试
- `test_rate_limiter_single_request` - 单个请求限流
- `test_rate_limiter_exceeds_limit` - 超出限流阈值
- `test_rate_limiter_window_reset` - 时间窗口重置
- `test_rate_limiter_concurrent` - 并发请求限流

### 指标监控测试
- `test_simple_counter` - SimpleCounter 功能
- `test_metrics_monitor` - MetricsMonitor 功能

### 工具函数测试
- `test_build_service_key` - 服务键构建
- `test_build_cache_key` - 缓存键构建
- `test_namespace_default` - Namespace 默认值
- `test_config_basic_info_default` - ConfigBasicInfo 默认值
- `test_page_default` - Page 默认值

### 错误处理测试
- `test_error_display` - 错误信息显示

### 并发测试
- `test_concurrent_counter` - 并发计数器

### 运行单元测试

```bash
# 运行所有单元测试
cargo test --test unit_test

# 运行特定测试
cargo test --test unit_test test_rate_limiter_single_request

# 显示测试输出
cargo test --test unit_test -- --nocapture
```

## 集成测试

集成测试需要运行一个 Batata 服务器。这些测试验证客户端与服务器之间的完整交互。

> **重要**: `BatataHttpClient::new()` 在构造时自动进行认证，因此不需要单独测试登录功能。

### HTTP 客户端测试
- `test_http_client_basic_request` - HTTP 客户端创建和认证
- `test_http_client_namespace_list` - 获取命名空间列表

### gRPC 客户端测试
- `test_grpc_client_connect_config_module` - 连接到 config 模块
- `test_grpc_client_connect_naming_module` - 连接到 naming 模块
- `test_grpc_client_multi_module` - 连接到多个模块

### 配置服务测试
- `test_config_service_publish_and_get` - 发布和获取配置
- `test_config_service_with_config_type` - 带类型的配置发布（YAML/JSON 等）
- `test_config_service_update_existing` - 更新现有配置
- `test_config_service_get_nonexistent` - 获取不存在的配置

### 命名服务测试
- `test_naming_service_register_and_deregister` - 实例注册和注销
- `test_naming_service_subscribe_and_unsubscribe` - 服务订阅和取消订阅
- `test_naming_service_multiple_instances` - 多实例管理
- `test_naming_service_get_instances_empty_service` - 获取空服务的实例列表

### 综合测试
- `test_config_and_naming_services` - 配置和命名服务协同工作

### 运行集成测试

首先启动 Batata 服务器：

```bash
# 使用 Docker Compose
docker-compose -f docker-compose.test.yml up -d

# 或者手动启动服务器
cargo run --bin batata-server
```

然后运行集成测试：

```bash
# 运行所有集成测试
cargo test --test integration_test -- --ignored

# 运行特定集成测试
cargo test --test integration_test test_config_service_publish_and_get -- --ignored

# 显示测试输出
cargo test --test integration_test -- --ignored --nocapture
```

## 测试配置

### 默认服务器地址
- 服务器: `http://127.0.0.1:8848`
- 用户名: `nacos`
- 密码: `nacos`
- 命名空间: `""` (空字符串表示公共命名空间)
- 分组: `DEFAULT_GROUP`

### 自定义配置

如果需要使用不同的服务器配置，可以修改 `integration_test.rs` 中的常量：

```rust
const TEST_SERVER_ADDR: &str = "http://your-server:8848";
const TEST_USERNAME: &str = "your-username";
const TEST_PASSWORD: &str = "your-password";
const TEST_NAMESPACE: &str = "your-namespace";
const TEST_GROUP: &str = "your-group";
```

## 测试原理说明

### HTTP 客户端认证
`BatataHttpClient::new()` 会在构造函数中自动调用 `authenticate()` 方法完成认证：
1. 向 `/v1/auth/login` 端点发送 POST 请求
2. 使用用户名和密码进行认证
3. 获取 accessToken 并缓存
4. 后续请求自动携带 accessToken header

因此，测试中不需要单独测试登录，只需要验证客户端能够成功创建即可。

### gRPC 客户端连接
`GrpcClient::new()` 创建客户端，`connect()` 方法建立与服务器的连接：
1. 连接到指定的 gRPC 服务器地址
2. 发送请求到指定的模块（config/naming）
3. 建立双向通信通道

### 配置服务测试流程
1. 使用 `publish_config()` 发布配置
2. 使用 `get_config()` 获取配置并验证内容
3. 使用 `remove_config()` 删除配置
4. 再次尝试获取配置，确认已删除

### 命名服务测试流程
1. 使用 `register_instance()` 注册服务实例
2. 使用 `get_all_instances()` 获取所有实例并验证
3. 使用 `deregister_instance()` 注销实例
4. 可选：使用 `subscribe()` 和 `unsubscribe()` 进行服务订阅

## 故障排查

### 集成测试失败

如果集成测试失败，请检查：

1. **服务器是否运行**
   ```bash
   curl http://127.0.0.1:8848/nacos/v2/cs/config
   ```

2. **认证凭据是否正确**
   - 确认用户名和密码
   - 确认用户有访问测试命名空间的权限

3. **网络连接**
   - 检查防火墙设置
   - 确认服务器地址和端口正确

4. **超时问题**
   - 增加 HttpClientConfig 中的超时时间
   - 检查服务器响应时间

5. **gRPC 端口**
   - 确认 gRPC 服务在指定端口运行
   - 检查模块名称（config/naming）是否正确

### 单元测试失败

单元测试失败通常是代码问题，请检查：

1. **依赖版本** - 确保 `Cargo.toml` 中的依赖版本正确
2. **Rust 版本** - 使用与项目兼容的 Rust 版本
3. **编译错误** - 运行 `cargo build` 查看详细错误信息

## 添加新测试

### 添加单元测试

在 `unit_test.rs` 中添加新的测试函数：

```rust
#[test]
fn test_your_feature() {
    // Arrange
    let input = "some input";

    // Act
    let result = your_function(input);

    // Assert
    assert_eq!(result, "expected output");
}
```

### 添加集成测试

在 `integration_test.rs` 中添加新的测试函数，并标记为 `#[ignore]`：

```rust
#[tokio::test]
#[ignore]
async fn test_your_integration() -> anyhow::Result<()> {
    // Create client (auto-authenticates)
    let config = create_http_config();
    let _client = BatataHttpClient::new(config).await?;

    // Your test code here
    println!("✓ Test passed");

    Ok(())
}
```

## 测试覆盖率

运行测试并生成覆盖率报告：

```bash
# 安装 tarpaulin
cargo install cargo-tarpaulin

# 生成覆盖率报告
cargo tarpaulin --out Html --output-dir coverage
```

## 注意事项

1. **集成测试清理** - 每个集成测试会自动清理创建的资源（配置、实例等），但如果测试失败，可能需要手动清理

2. **命名空间隔离** - 使用唯一的 ID 避免测试之间的冲突：
   ```rust
   let data_id = format!("test-config-{}", uuid::Uuid::new_v4());
   ```

3. **并发测试** - 单元测试包含并发测试，验证线程安全性

4. **性能考虑** - 集成测试会创建和删除资源，避免在关键环境中运行

5. **认证自动处理** - HTTP 客户端和 gRPC 客户端都会自动处理认证，测试中无需手动管理 token
