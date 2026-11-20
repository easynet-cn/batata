# Raft 超时基线、推导与可观测性

> 本文回答三个问题：超时到底是多少、依据是什么、异常时怎么判别。
> 对应任务 T1（基线审计）/ T2（指标）/ T3（联动改造）/ T4（根因）/ T5（门禁）。
> 所有数字均以源码为准；上游结论附可核验锚点，未取得官方锚点的一律标注为「参考」。

## 1. 结论先行

1. **上一轮口径需要纠正**：此前把 batata 的选举窗口说成 `3000–6000ms`，那只是
   `RaftConfig::default()`（库默认，仅单测与 `RaftNodeBuilder` 使用）。生产路径
   `Configuration -> typed_config -> startup/persistence.rs` 用的是 **1000ms min /
   2000ms max / 200ms 心跳**。因此「leaderless 等待 5s < 选举窗口 6s」这一量纲错位
   **只在库默认路径成立**；生产里 5s 反而大于 2s。
2. 但问题性质没有变：三层默认值互不一致、所有等待量是互不相关的硬编码常量、
   没有任何不等式校验、Raft 核心路径零指标。这正是「偶发异常无法判别」的根因。
3. 现在的做法（T3 已落地）：所有等待由 `RaftConfig::timing()` 从选举窗口与 RPC
   超时推导，并在 `RaftConfig::validate()` 里校验不变式；节点启动即 fail-fast。

## 2. 三层默认值与实际生效值

| 层 | 位置 | election min | election max | heartbeat | 说明 |
|---|---|---|---|---|---|
| 库默认 | `crates/batata-consistency/src/raft/config.rs` `RaftConfig::default()` | 3000ms | 6000ms（`to_openraft_config` 取 `2×min`） | 300ms | 单测、`RaftNodeBuilder` 用 |
| **生产默认** | `crates/batata-server-common/src/model/typed_config.rs:338-346` | **1000ms** | **2000ms** | **200ms** | `startup/persistence.rs:348` 注入 `RaftConfig` |
| 配置文件注释 | `conf/application.yml` | 1000ms | 未写（由代码 ×2） | 200ms | 仅文档，键被注释掉 |

另有 `crates/batata-server-common/src/model/config.rs` 的 getter 文档曾写
「default 5000 / 1000」，与真实值不符，T1 已修正。

其它生产值：`rpc_request_timeout_ms = 5000`、`snapshot_transfer_timeout_ms = 30000`、
`forward.max_retries = 3`、`forward.initial_delay_ms = 200`。

## 3. 改造前的调用点清单（T1 审计结果）

| 位置 | 原值 | 性质 | 问题 |
|---|---|---|---|
| `node.rs` `CLIENT_WRITE_TIMEOUT` | 15s | 硬编码 | 与选举窗口无推导关系 |
| `node.rs` `LEADERLESS_MAX_POLLS × LEADERLESS_POLL_INTERVAL` | 25 × 200ms = 5s | 硬编码 | 库默认 6s 选举窗口下会提前放弃；生产 2s 下又过长且无依据 |
| `node.rs` `wait_for_applied(..., 5s)` | 5s | 调用点硬编码 | 与 leaderless 预算同值但无关联 |
| `config.rs` `election_timeout_ms * 2` | ×2 | 唯一有推导关系的值 | 无注释、无校验 |
| `config.rs` 字段注释 | 5000ms / 1000ms | 文档 | 与真实默认值失真 |
| `model/config.rs` getter 注释 | 5000ms / 1000ms | 文档 | 与 typed 默认值失真 |

## 4. 上游 / 框架对照（带锚点）

| 来源 | heartbeat | election | 写/RPC 超时 | 依据 |
|---|---|---|---|---|
| openraft 0.10.1（本项目框架） | 50ms | 150–300ms | 无整体 deadline | `~/.cargo/registry/.../openraft-0.10.1/src/config/config.rs:59-84` `DEFAULTS` |
| openraft 官方建议 | `≈ election/3` | `≈ 10–20× RTT` | — | docs.rs `openraft/0.10.1/struct.Config.html` §Timing Constraints 原文 |
| openraft 硬约束 | `election_timeout_min > heartbeat_interval` | `min < max` | — | 同文件 `Config::validate()`（:901-935） |
| hashicorp/raft `DefaultConfig()` | 1000ms | 1000ms | Commit 50ms、LeaderLease 500ms | `github.com/hashicorp/raft/config.go` `DefaultConfig()` |
| hashicorp/raft `ValidateConfig` | `lease ≤ heartbeat ≤ election` | | | 同文件，原文：`LeaderLeaseTimeout ... cannot be larger than heartbeat timeout`、`ElectionTimeout ... must be equal or greater than Heartbeat Timeout` |
| Consul 默认（multiplier=5） | 5000ms | 5000ms | `rpc_hold_timeout = 7s` | Consul 官方 [General parameters](https://developer.hashicorp.com/consul/docs/reference/agent/configuration-file/general) 与 [Server Performance](https://developer.hashicorp.com/consul/docs/v1.18.x/reference/architecture/server) |
| etcd（参考，未取官方锚点） | 100ms | 1000–2000ms | — | 社区资料 `ElectionTick=10 / HeartbeatTick=1 / TickMs=100`；PreVote 默认说法互相矛盾，**不作为推导依据** |
| **batata 生产（改造前）** | **200ms** | **1000–2000ms** | **15s / 5s / 5s** | 无联动、无校验、无指标 |

Consul 官方对 `rpc_hold_timeout` 的定义：*"A duration that a client or server will
retry internal RPC requests during leader elections. Under normal circumstances,
this can prevent clients from experiencing 'no leader' errors. Defaults to 7s."*
batata 的 leaderless 等待在语义上就是它：**选举窗口 + 一次 RPC 重试预算**。
Consul 用同一个 `raft_multiplier` 推导全部时序；batata 之前各写各的常量。

Consul 官方还明确点名了本项目的可疑现象：*"Spurious leader elections can be
caused by networking issues between the servers or insufficient CPU resources"*，
并建议用 `consul.raft.leader.lastContact` 观测。这是 T2/T4 的动机。

### openraft 的两个默认行为（T4 关键线索，T6 已处置）

- `enable_pre_vote: Option<bool>` 默认 `None`，docs.rs 原文：*"None (the default)
  leaves the choice to Openraft, which currently treats it as disabled."*
  改造前 batata `to_openraft_config()` 用 `..Default::default()` 未设置该字段 ⇒
  **PreVote 关闭**；T6 已显式设为 `Some(true)`（见 §6.2）。
- `enable_leader_restore` 默认开启：*"A node whose saved vote identifies it as the
  former leader starts an election in a higher term"*，且不等选举超时。
  regression 矩阵会在六个组合间反复重启节点，重启的前 leader 会立即发起选举，
  可能加剧 term 膨胀；本项保持上游默认，未改动。
- 这两条曾被用来解释「稳态出现多次选举」；T6-a 实测后确认真正的触发源是跨
  `.await` 持锁（§6.1 第 3 点 / §6.2），PreVote 关闭只是放大了停顿的后果。

## 5. 推导公式与不变式（T3 落地）

记 `E = election_timeout_ms`、`E_max = 2E`、`H = heartbeat_interval_ms`、
`R = rpc_request_timeout_ms`，`C = E_max + R` 为「一个选举周期」：

| 派生量 | 公式 | 生产值 | 库默认值 | 语义 |
|---|---|---|---|---|
| `election_timeout_min/max` | `E` / `2E` | 1000 / 2000ms | 3000 / 6000ms | openraft 窗口 |
| `election_cycle` | `C = E_max + R` | 7000ms | 11000ms | 对齐 Consul `rpc_hold_timeout`（5s 选举 → 7s） |
| `leaderless_wait` | `C` | 7000ms | 11000ms | follower 等 leader 出现 |
| `apply_wait` | `C` | 7000ms | 11000ms | follower 等本机 FSM 追平 |
| `client_write` | `2C` | 14000ms | 22000ms | 覆盖写入期间的一次 leadership change |
| leaderless 采样 | `H/2`，夹在 `[10ms, 200ms]` | 100ms × 70 次 | 150ms × 73 次 | 采样快于选举，不会漏掉 Candidate 态 |

`RaftConfig::validate()` 强制：

- `H > 0`、`E > H`（openraft 硬约束）、`R > 0`；
- `snapshot_transfer_timeout_ms > R`（原注释要求，之前没有代码保证）；
- `forward_initial_delay_ms > 0`（当 `forward_max_retries > 0`）；
- 溢出检查（`2E`、`E_max + R`、`2C`）；
- `leaderless_wait ≥ E_max`（**正是消除 5s < 6s 错位的断言**）、`client_write > leaderless_wait`。

节点创建时 `RaftNode::new_with_full_options` 调用 `validate()`，在任何目录/RocksDB
打开之前 fail-fast，并把派生结果打进启动日志，便于现场核对。

## 6. 可判别规则（T2 指标 + T5 门禁）

T2 新增 `crates/batata-consistency/src/raft/telemetry.rs`，订阅 openraft 的
metrics watch 通道（不是定时轮询：实测 100ms 轮询会漏掉 leader 上仅持续几十毫秒的
Candidate 态），导出 Prometheus 指标与 `batata_raft_health` 结构化日志（命名对齐
Consul telemetry，加 `batata_` 前缀）：

| 类型 | 指标 | 用途 |
|---|---|---|
| Gauge | `batata_raft_term/role/leader_id/last_log_index/last_applied_index/apply_lag/steady_state_elections` | 状态与滞后 |
| Counter | `batata_raft_elections_total`、`steady_state_elections_total`、`leader_changes_total`、`term_changes_total`、`client_write_timeouts_total`、`apply_wait_timeouts_total`、`forwarded_writes_total`、`forward_failures_total`、`leaderless_give_ups_total` | 异常次数 |
| Histogram | `batata_raft_election_duration_seconds`、`client_write_duration_seconds`、`apply_wait_duration_seconds`、`forward_duration_seconds` | 耗时分布 |

判别口径：

- **稳态**（已见过 leader 后）出现 Candidate ⇒ 记 `steady_state_election` 并打 WARN；
  冷启动的第一次选举不算异常（对应 Consul `candidate == 0 while leader exists`）。
- 门禁只把 `steady_state_election` / `apply_wait_timeout` /
  `client_write_timeout` / `leaderless_give_up` 计为 blocker，要求全部为 0；
  冷启动选举只记录、不计数（INFO），cluster 组合缺少 `telemetry_started`
  则 fail-closed，防止「遥测根本没起来」被误判为健康。
- 关联判别：选举时刻 CPU 打满 ⇒ runtime 饥饿；CPU 正常但 AppendEntries/forward
  失败 ⇒ 网络/端口串扰；`apply_lag` 或 `apply_wait` 尖峰 ⇒ RocksDB/FSM 阻塞。
- 资源侧沿用现有 `system_cpu_usage_percent` / `process_memory_bytes` 周期采样，
  不在本轮重复造指标栈。

### 6.1 T4 稳态选举根因（实测证据）

证据目录 `/tmp/batata-raft-health-1795179141/`：全新 release 二进制、3 节点冷启动，
Go Consul SDK 与 `batata-consul-client` 两套用例均 `rc=0`，但 T2 指标抓到：

| 现象 | 实测 |
|---|---|
| node1 稳态选举 / apply 超时 | 3 次 / 3 次；term 最终 7、leader 变更 7 次 |
| node2 | 稳定（0 次稳态选举） |
| node3 | term 变更 7 次，跟随 node1 的 term 抬升 |
| 结束时 `apply_lag` | 0（不是「落后但能追上」，是选举期间整段停摆） |
| CPU / 内存 | `system_cpu_usage_percent` 4.29–11.27%，进程 171–206MB |

三次选举的 openraft deadline 全部**迟到约 3.6–3.7s** 才被处理：

| `lease_expires_at` | 实际触发 | 延迟 |
|---|---|---|
| 12:53:08.950191 | 12:53:12.639813 | 3.690s |
| 12:53:17.221654 | 12:53:20.795244 | 3.574s |
| 12:53:26.320849 | 12:53:29.891071 | 3.570s |

每次触发前 node1 还有约 **7s 的全进程日志静默**（12:53:05.33→12:53:12.64 等）。
日志里三条 deadline 均为 `pre_vote_retry_at: None`，直接证实 PreVote 关闭。

源码侧的解释链（T6-a 复核后修正；最终根因见 §6.2）：

1. `crates/batata-server/src/main.rs` 用 `#[actix_web::main]`；`Cargo.lock` 的
   `actix-rt 2.16.0` 在 `runtime.rs:23` 用 `Builder::new_current_thread()`，
   `openraft-rt-tokio` 的 `C::spawn` 就是 `tokio::spawn`。RaftCore、SM worker、
   Raft gRPC server、xDS 快照发布全落在同一个 `main` 线程。
2. `network.rs::effective_timeout()` 在 `RPCOption::hard_ttl()` 非零时直接采用它；
   openraft 0.10.1 的 `heartbeat/worker.rs:105` 与 `replication/mod.rs:333` 把 TTL
   设为 `heartbeat_interval`，`heartbeat/worker.rs:118` 还自行包了一层
   `C::timeout(200ms)`。因此**生产配置里的 `rpc_request_timeout_ms=5000` 对
   AppendEntries / Vote 完全不生效，实际硬超时只有 200ms**（leader 实测 24s 内
   217 次 `AppendEntries RPC timed out after 200ms`）。
3. **真正的触发源是跨 `.await` 持有同步锁**（§6.2）：`connect_ca.rs::update_intention()`
   把 DashMap 分片写守卫一路握到 `raft.write(...).await`，而 FSM apply hook 在
   `main` 上要 `intentions.clear()`，于是单线程执行器被冻住约 7s（正好一次
   `apply_wait`），期间心跳 / AppendEntries 全部 200ms 超时。
4. PreVote 关闭（`to_openraft_config` 用 `..Default::default()`，openraft
   `get_enable_pre_vote()` = `unwrap_or(false)`；日志 `pre_vote_retry_at: None`）
   ⇒ 漏心跳直接抬 term 打断 leader ⇒ 在途写丢失 ⇒ `wait_for_applied` 7s 超时。

因此 **`apply_wait_timeout` 是下游症状，不是独立缺陷**；T3 把超时联动做严密并不能
消除触发源。CPU 只有 4–11% 也排除了「CPU 打满导致 spurious election」这一常见解释，
指向的是**单线程运行时上的同步阻塞 / 跨 await 持锁**，而不是参数本身太小。

`enable_leader_restore`（前 leader 重启立即以更高 term 竞选）在矩阵跨组合重启节点时
仍可能加剧 term 膨胀，但本轮定向验证没有节点重启，它不是这 3 次选举的触发源。

### 6.2 T6 根因修复（PreVote + 同步 I/O 隔离 + 跨 await 持锁修复）

**(1) 最终根因：跨 `.await` 持有 DashMap 分片写守卫。**

`crates/batata-plugin-consul/src/connect_ca.rs::update_intention()` 原先写成
`if let Some(mut entry) = self.intentions.get_mut(id) { ... raft.write(...).await ... }`：
HTTP handler 在 actix arbiter 线程上持有 `intentions` 的同步分片写锁，一直握到转发写
结束。与此同时 Raft FSM apply 在 `main` 线程上提交 `IntentionUpsert` 后派发
`HookDispatch::CaChange`（`raft/plugin_handler.rs:213/242`）→ `plugin.rs:139
on_ca_change()` → `refresh_from_rocks()` → `intentions.clear()` + 逐条 insert
（`connect_ca.rs:1674`），需要同一张表的**全部分片写锁**。单线程运行时上这直接把
`main`（RaftCore / Raft gRPC server / heartbeat / deadline notifier）冻结，直到
arbiter 上的 `wait_for_applied` 在 7s 后超时并释放守卫 —— 正好解释三次 7s 停顿、
`main` 在超时瞬间复活、以及期间每 200ms 一次的 AppendEntries 超时。
`peering.rs` 的同类写法早已显式 `drop(peering)`，只有这里漏了。

修复：把变更收敛到**同步**私有 helper `build_updated_intention()`，它返回 owned
`Intention`；编译器因此保证守卫不可能活到 `update_intention` 的任何 `.await`。
`update_intention_has_no_shard_guard_across_await` 是架构守卫测试，把 `get_mut(`
放回异步函数体会立即变红（mutant 已验）。

**(2) 协议级兜底：启用 PreVote。**

`to_openraft_config()` 显式设置 `enable_pre_vote: Some(true)`。openraft 0.10.1 的
`pre_elect()`（`engine_impl.rs:335-375`）明确「neither updates `vote` nor changes the
server state: the node stays a Follower until a quorum would grant the vote」，且
leader lease 未过期时直接跳过 pre-elect。因此一次瞬时停顿不会再变成真实 term 抬升。

**(3) 同步 I/O 隔离（不是根因，但消除同类 head-of-line 风险）。**

`RocksLogStore` 的 `save_vote` / `append` / `truncate_after` / `purge` 原本用
`set_sync(true)` 在 RaftCore 所在线程上同步 fsync；现统一经 `blocking_write()` 交给
`spawn_blocking`。持久性不变（`IOFlushed` 仍在 fsync 完成后才回调），写入顺序不变
（RaftCore 逐个 await），但磁盘忙时运行时线程不再被独占。

## 7. 遗留与边界

- 本轮**不改** `E/H/R` 的默认值（属于行为变更），只把等待量联动起来；是否把
  生产 `rpc_request_timeout_ms`（5000ms，比 `E_max` 大 2.5 倍）收紧，需单独评估。
  T6-a 另发现该配置对 AppendEntries / Vote **实际不生效**（§6.1 第 2 点）：openraft
  用 `heartbeat_interval` 作为 `RPCOption::hard_ttl`，并在 heartbeat worker 里自行
  `C::timeout(200ms)`，所以调 `rpc_request_timeout_ms` 不会改变复制 RPC 的 200ms
  预算；真要放宽只能同步上调 `heartbeat_interval` 与选举窗口，需独立评估。
- 根因已按 §6.2 修复，T5 的阻断语义保持不变：`regression.sh --full` 的 matrix
  阶段继续把 `steady_state_election` / `apply_wait_timeout` /
  `client_write_timeout` / `leaderless_give_up` 视作 blocker，cluster 组合缺少
  `telemetry_started` 则 fail-closed。红灯不是门禁误报，是不让偶发被复跑掩盖。
- `apply_lag` 用 `last_log_index - last_applied`，是本机视角；跨节点 commit 差异
  由 `cluster_committed` 提供，本轮未纳入。
- `RocksLogStore` 的读路径（`try_get_log_entries`）与 SM `apply` 的同步 RocksDB
  调用仍在主运行时上；T6 只把四处 fsync 写移出。若后续在高写入负载下再出现运行时
  停顿，优先考虑隔离 SM apply 路径，而不是调大超时。

## 参考

- openraft 0.10.1 `Config` 文档：https://docs.rs/openraft/0.10.1/openraft/struct.Config.html
- openraft 0.10.1 源码 `src/config/config.rs`（本地 cargo registry）
- Consul General parameters：https://developer.hashicorp.com/consul/docs/reference/agent/configuration-file/general
- Consul Server Performance：https://developer.hashicorp.com/consul/docs/v1.18.x/reference/architecture/server
- hashicorp/raft `config.go`：https://github.com/hashicorp/raft/blob/main/config.go
