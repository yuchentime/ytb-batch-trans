---
status: current
layer: domain
domain: download-engine
canonical_for:
  - download-engine-backend-behavior
related:
  - docs/current/domains/download-engine/flow.md
  - docs/current/platform/observability.md
last_verified: 2026-09-17
---

# download-engine 后端行为

## Purpose

说明 `YtdlpRunner` 的能力边界、进程生命周期、事件循环的并发模型与上报策略，
让修改执行层的改动不必反推整个调度器。

## Current Behavior

### 模块划分（`src-tauri/src/runners/`）

| 文件 | 职责 |
| --- | --- |
| `ytdlp_runner.rs` | Builder：累积 argv、注入 env、`output()`（一次性执行）与 `spawn()`（流式执行）；字幕/SponsorBlock/日志摘要的纯函数 |
| `ytdlp_process.rs` | 平台差异：Unix 进程组、Windows Job Object、`CREATE_NO_WINDOW`、kill 语义 |
| `ytdlp_download.rs` | 下载任务编排：参数链、解析器、`tokio::select!` 事件循环、退出处理、内部 fatal |
| `ytdlp_info.rs` | 抓取任务：`-J --flat-playlist`、日志写入、stderr 截断、退出码与解析错误处理 |
| `override_resolver.rs` | `ApplyPatch<T>` 实现与 `resolve_with_patch`（全局 + override 三态合并） |
| `template_context.rs` | 文件名/目录模板渲染与路径字符消毒 |
| `ytdlp_args/format_args.rs` | `-f` selector 与 `-S` 排序字段 |
| `ytdlp_args/output_args.rs` | 容器/策略/预设/后处理参数、公共输出开关、部分下载 |
| `ytdlp_args/location_args.rs` | 目录与文件名模板选择 + `-o` |
| `ytdlp_args/input_filter_args.rs` | `-I`、体积/日期/匹配过滤、playlist 模式 |
| `ytdlp_args/tests.rs` | 参数矩阵回归（约 900 行断言） |

### `YtdlpRunner` 生命周期

- `new(app)`：快照 `Config`（`Arc<Config>`）与 `Preferences`，取 `bin_dir`，argv 以 `--encoding utf-8` 起始。
- Builder 方法逐段追加 argv，均返回 `Self`（原地可变），因此参数顺序即调用顺序。
- `output()`：`spawn_blocking` 中执行 `Command::output()`，返回 `{status, stdout, stderr}`（用于抓取）。
- `spawn()`：管道 stdin/stdout/stderr，`configure_command` → `platform_process_from_child` → 起两个读线程 + 一个等待线程，返回 `(UnboundedReceiver<YtdlpCommandEvent>, YtdlpChild)`。
- 事件类型：`Stdout(Vec<u8>)`、`Stderr(Vec<u8>)`、`Error(String)`、`Terminated{code}`。
- 读线程按 1 字节读取，遇 `\n`/`\r` 成行；EOF 时把残余缓冲作为最后一行发出。

### 事件循环

```text
loop {
  select! {
    event = rx.recv() => { 取消检查 -> 按类型处理/转发 -> ... }
    _ = cancel_rx.changed() => { 若已取消 -> kill_tree + Ok(()) }
  }
}
```

- 每个事件到达时先做一次取消检查（保证取消后立即停）。
- `Stdout`/`Stderr`：写日志（跳过空行与 `RAW` 前缀）→ 进度解析 → 错误解析。
- `Terminated{Some(0)}` → `media_complete` + `Ok(())`。
- `Terminated{其它}` → `media_fatal`（携带退出码）+ `Err(NonZeroExit)`。
- `Error` → 内部 fatal + `Err(RunnerError)`。
- 通道关闭（无 `Terminated`）→ 内部 fatal + `Err(EventStreamEnded)`。

### 并发模型

- 每条下载任务是一个独立 async 任务（由调度器 spawn），阻塞操作（`Command::output`、spawn）都在
  `spawn_blocking` 或独立 OS 线程中，不阻塞 tokio 工作线程。
- 与任务的通信通过 `tokio::sync::mpsc::unbounded_channel`（行事件），不设背压；日志写入是同步 `RwLock`。
- 取消信号来自 `scheduling::group_state::subscribe_group` 的 `watch::Receiver<bool>`。

### 日志摘要（防泄漏）

`log_run_summary` 只记录布尔摘要：`arg_count`、`has_proxy`、`has_cookies`、`has_browser_cookies`、`has_auth`。
设计上**从不读取参数值**，因此任何形态的密钥（包括以 `-` 开头的值）都不会进入日志（见 `ytdlp_runner.rs` 的测试 `summary_flags_presence_regardless_of_value_shape`）。

### Sentry 上报策略

| 层 | 上报的错误 |
| --- | --- |
| 下载任务 | `SpawnFailed`、`InvalidDiagnosticRules`、`EventStreamEnded` |
| 抓取任务 | `InvalidDiagnosticRules`、`RunnerFailed`、`ParseFailed` |
| 不上报 | `NonZeroExit`（业务失败）、`InvalidArguments`（用户配置问题）、`RunnerError`（多数为环境问题） |

前端另有独立通道：诊断卡片上的"上报"按钮（`code=unknown` 且 `level=error`）与 Vue 全局 Sentry 插件。

## Boundaries

- runner 不做速率限制、重试、断点续传控制（yt-dlp 自身的重试除外）；`--continue` 也未显式传递，依赖 yt-dlp 默认行为。
- 模板渲染只处理本应用认识的占位符；其余原样交给 yt-dlp。
- 事件不保证顺序地在日志与业务事件之间一致（日志先写，事件后发）。

## Contracts

- 参数矩阵：`docs/current/domains/download-engine/api-contract.md`。
- 解析契约：`docs/current/domains/download-engine/progress-and-diagnostics.md`。
- 配置字段：`docs/current/domains/settings-preferences/data-model.md`。

## Failure And Edge Cases

- 部分下载（`--download-sections`）时百分比来自 ffmpeg 时间戳而非文件字节，因此"进度 = 已处理时长 / 区段时长"。
- 若未提供 `subtitleInventory`，即使全局开启字幕也不会产生字幕参数（后端无法判断可用语言）。
- `killed` 的进程退出码在 Windows 上为 `1`（`TerminateJobObject(…, 1)`），在 Unix 上为信号终止（`code()` 为 `None`）；取消路径不会读退出码，因此不影响业务。
- Windows 上 Job Object 创建失败会返回 `yt-dlp job object error`，此时任务以 `SpawnFailed` 结束（进程已被 kill）。

## Verification

见 `docs/current/domains/download-engine/verification.md`；参数矩阵的主要回归在 `runners/ytdlp_args/tests.rs` 与 `runners/ytdlp_runner.rs` 的 `#[cfg(test)]` 块。
