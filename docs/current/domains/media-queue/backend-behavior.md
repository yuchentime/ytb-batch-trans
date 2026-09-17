---
status: current
layer: domain
domain: media-queue
canonical_for:
  - media-queue-backend-behavior
related:
  - docs/current/domains/download-engine/backend-behavior.md
  - docs/current/shared/data-ownership.md
last_verified: 2026-09-17
---

# media-queue 后端行为

## Purpose

说明队列侧后端的调度模型、并发与分组生命周期，以及命令层如何把请求接入调度器。
参数构造/进程执行属于 `download-engine`，本文只覆盖"任务排队与分派"。

## Current Behavior

### 命令层（`src-tauri/src/commands/`）

| 命令 | 关键行为 |
| --- | --- |
| `media_info` | `ensure_group_running(group_id)` → `FetchSender.send(Pipeline(FetchRequest::Initial{..}))`；发送失败返回 `Err(String)` |
| `media_playlist_expand` | 同上，`FetchRequest::Playlist{ entries, overrides }` |
| `media_size` | 同上，`FetchRequest::Size{ format }` |
| `media_download` | `ensure_group_running` → `DownloadSender.send(Pipeline(DownloadRequest::Batch{..}))`；**发送失败直接 `unwrap()` panic**（历史上不会发生，但属于已知脆弱点） |
| `group_cancel` | `cancel_group` + 向两个调度器发 `Cleanup` + `LogStore.remove_group` |

命令本身不做校验（URL 合法性、group 存在性都由前端保证），也不返回业务错误。

### 分组运行状态（`scheduling/group_state.rs`）

全局表 `RUNNING_GROUPS: HashMap<String, watch::Sender<bool>>`：

- `ensure_group_running`：存在则 `send(true)`（并发安全的"复活"），否则新建通道值为 `true`。
- `cancel_group`：存在则 `send(false)`，否则插入 `false`（保证后续 `is_group_running` 为假）。
- `is_group_running`：通道当前值为 `true` 才算运行；不存在视为未运行。
- `remove_group`：从表中删除（由 `Cleanup` 触发）。
- `subscribe_group`：给下载任务返回 `watch::Receiver<bool>`，用于 `tokio::select!` 中断。

关键语义：**取消是"粘性"的**——`cancel_group` 之后，除非再次 `ensure_group_running`，该 group 的任何新任务都不会被调度。

### 通用调度器（`scheduling/dispatcher.rs`）

`GenericDispatcher<Req>` 是 fetch/download 共用的单线程事件循环（`tauri::async_runtime::spawn`）：

1. 用 `try_recv` 排空请求：`Cleanup{group_id}` → 丢弃该 group 的队列项并 `remove_group`；`Pipeline(req)` → `make_entries(req)` 展开为条目；若首个条目的 group 正在运行，则为每个条目分配编号并入队，否则整批丢弃。
2. `queues.retain(is_group_running)`：被取消/已清理的 group 剩余条目自动出队。
3. 把上一轮的 `pending_requeue` 追加到队尾（实现 group 间轮转）。
4. 队列为空时阻塞在 `rx.recv().await`，收到后把请求回投给自己继续循环（避免空转）。
5. 队列非空时循环：取一个许可 → 弹出一个 group 的一个条目 → `spawn` 执行 `run_job`（携带许可，任务结束自动释放）→ 若该 group 还有剩余条目则放入 `pending_requeue`。

由此得到的行为：

- **组内串行、组间轮转**：同一 group 的条目按顺序一次只跑一个；不同 group 交替获得许可。
- **新 group 不被饿死**：`pending_requeue` 在下一轮整体追加到队尾，新入队 group 排在其后但不会被无限挤后。
- **许可与任务一一对应**：许可证由 spawn 的任务持有，任何路径结束都会释放；取消的 group 不会再取新许可。

### 并发控制（`scheduling/concurrency.rs`）

`DynamicSemaphore` = `tokio::Semaphore` + `AtomicUsize max` + `held: Mutex<Vec<OwnedSemaphorePermit>>`：

- `acquire_owned()` 取许可。
- `resize(new_max)`：调大时先归还 `held` 中暂存的许可，不足再 `add_permits`；调小时用 `acquire_owned` 逐个"借走"多余许可并存入 `held`，从而在不影响在跑任务的前提下降低并发上限。
- 下限强制为 1（`max(1)`）。

`DownloadLimiter` 与 `FetchLimiter` 是两个独立实例，初值都取 `config.performance.maxConcurrency`；
`config_set` 触发 `Config::on_updated` 时对两者异步 `resize`。因此**同时进行抓取与下载时瞬时并发可达 2×maxConcurrency**。

### fetch 流水线（`scheduling/fetch_pipeline.rs`）

`FetchRequest` 四种：`Initial`、`Playlist`、`Size`、`SizePlaylist`。

- `expand_fetch_request`：`Initial`/`Size` 展开为 1 条；`Playlist`/`SizePlaylist` 为每条 entry 生成新 uuid，并把 `GROUP_COUNTERS[group_id] = total`。
- `FetchEntry { group_id, id, url, total, format, overrides }`；`group_key()` 恒为 `None`（抓取阶段不参与编号）。
- `handle_fetch_entry`：调用 `run_ytdlp_info_fetch` → 按结果分流：
  - `Single` + `format` → `media_size` 事件；`Single` 无 format → `media_add`。
  - `Playlist` + `format` → **重新入队** `FetchRequest::SizePlaylist`（体积查询最终仍逐条走 Single 路径）。
  - `Playlist` 无 format → `media_add`（带 entries；`total` 用 `pl.entries.len()`）。
  - `Livestream` → `media_fatal`（`internal=true`）。
  - `None`（已发过事件）→ 不做事。
- 计数器递减到 0 时发 `Cleanup`，并 `remove_group`（通过调度器）。
- 计数器只在 `Single` 与 `Livestream` 分支递减；`Playlist` 分支的重入队通过新的 `SizePlaylist` 展开继续计数。

### download 流水线（`scheduling/download_pipeline.rs`）

- `DownloadRequest::Batch { group_id, items }` → `DOWNLOAD_COUNTERS[group_id] = items.len()`，展开为 `DownloadEntry`。
- `DownloadEntry { group_id, id, url, format, subtitle_inventory, overrides, template_context }`；
  `group_key()` 返回模板变量里的 `playlist_id`（用于组内编号）。
- 任务体：调 `run_ytdlp_download`，捕获错误并按白名单上报 Sentry（`SpawnFailed`、`InvalidDiagnosticRules`、`EventStreamEnded`）；随后递减计数器，归零时发 `Cleanup`。
- 计数器以 `items.len()` 为基准，因此**同一条目重复下发会导致计数不匹配**（例如前端重复点下载）。

### 编号（`scheduling/numbering.rs`）

`NumberingManager` 属于调度器实例，进程内单调：

- `next_autonumber` 从 1 开始，每个被调度条目 +1 → 写入模板变量 `autonumber`。
- `per_group[group_key]` 按 `playlist_id` 累加 → 写入模板变量 `playlist_autonumber`（仅当 `group_key` 为 `Some`）。
- 编号在**条目被派发进入队列时**分配，而不是执行时；重新入队（例如暂停后恢复）会继续递增。

### 与下载引擎的接口

调度器只负责"何时跑"，把 `DownloadEntry` 原样交给 `run_ytdlp_download`（见 `download-engine/backend-behavior.md`）。
`Cleanup` 只是移除该 group 的排队项与运行状态标记，**不会杀进程**——杀进程只发生在 `group_cancel` 的取消信号路径。

## Boundaries

- 调度器是全局单例（每个 pipeline 一个），没有优先级、没有持久化、没有重试策略。
- 后端不知道 group 的 UI 状态，也不校验 `items` 是否与前一次重叠。
- `enforce` group 存在性由前端保证；后端对不存在的 group 会直接创建运行状态（`ensure_group_running`）。

## Contracts

- 事件契约见 `docs/current/domains/media-queue/api-contract.md`。
- 状态归属：运行状态表与计数器都是进程内瞬态，见 `docs/current/shared/data-ownership.md`。

## Failure And Edge Cases

- `media_download` 的 `unwrap()` 会在调度器任务已退出时 panic（Rust panic 会终止该 async 任务；commands 在 IPC 线程上，实际表现为调用失败）。
- 取消后立刻恢复：`ensure_group_running` 会把通道值改回 `true`，但已被 `retain` 丢弃的排队条目不会回来——恢复依赖前端重新下发 items。
- `Playlist` 分支不递减计数器（只有 `Single`/`Livestream` 会）。因此若某条 entry 又返回 Playlist（嵌套播放列表）且不带 format，该批次计数器无法归零，`Cleanup` 不会发出，group 会滞留在 `RUNNING_GROUPS` 中，直到用户取消/删除该 group。
- 计数器使用 `LazyLock<Mutex<HashMap>>` 全局静态，`group_id` 复用（理论上不会）会覆盖计数。

## Verification

见 `docs/current/domains/media-queue/verification.md`；Rust 侧覆盖：`scheduling/dispatcher.rs` 的并发与轮转测试（`dispatcher_respects_single_concurrency`、`dispatcher_round_robins_groups_with_single_permit`、`dispatcher_does_not_starve_new_group`）。
