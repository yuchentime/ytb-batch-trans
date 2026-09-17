---
status: current
layer: platform
domain: backend-runtime
canonical_for:
  - backend-runtime-facts
related:
  - docs/current/platform/frontend-runtime.md
  - docs/current/shared/ipc-conventions.md
last_verified: 2026-09-17
---

# 后端运行时

## Purpose

记录 Rust 侧的技术栈、模块划分、异步模型与命令注册面，作为后端改动的入口地图。

## Current Behavior

### 技术栈

| 项 | 值 |
| --- | --- |
| 运行时 | Tauri v2（`tauri = "2"`），`main.rs` 仅调用 `open_video_downloader_lib::run()` |
| 异步 | tokio（Tauri 内置 runtime）+ `futures`；阻塞任务走 `spawn_blocking` 或独立 OS 线程 |
| 序列化 | serde / serde_json（`rename_all = "camelCase"`） |
| 状态 | `arc_swap::ArcSwap`（配置快照）、`tokio::sync::Mutex/RwLock`、`std::sync::Mutex`、全局 `LazyLock` |
| HTTP | `reqwest`（仅二进制下载与清单） |
| 加密 | `tauri-plugin-stronghold` + `keyring`（master key）+ `ed25519-dalek`/`minisign`（清单验签） |
| 归档 | `zip`、`tar` + `bzip2`（解压二进制） |
| 日志 | `tracing` + `tracing-subscriber` + `sentry`（tracing 与错误上报） |
| 其他 | `shlex`（自定义 ffmpeg 参数校验）、`hex`、`base64`、`indexmap`、`uuid`、`regex` |
| macOS 通知 | Linux 用 `notify_rust`；Windows Job Object 用 `windows-sys`；Unix 信号用 `libc` |

### 模块地图（`src-tauri/src/`）

| 模块 | 职责 | 详细文档 |
| --- | --- | --- |
| `lib.rs` | 插件注册、状态装配、命令注册、tracing/Sentry 初始化 | `../domains/app-lifecycle/backend-behavior.md` |
| `main.rs` | 二进制入口（仅调用 `run()`） | — |
| `paths.rs` | 环境形态探测与目录解析 | `storage.md` |
| `state/` | `Config`/`Preferences` 持久化（`json_handle`/`json_state`/`config_models`/`preferences_models`） | `../domains/settings-preferences/backend-behavior.md` |
| `commands/` | Tauri 命令（每个文件一个命令） | `../shared/ipc-conventions.md` |
| `scheduling/` | 通用调度器、并发信号量、分组状态、编号、fetch/download 流水线 | `../domains/media-queue/backend-behavior.md` |
| `runners/` | yt-dlp 参数构造、执行、解析衔接、模板渲染、override 合并 | `../domains/download-engine/*` |
| `parsers/` | yt-dlp 输出的 JSON/行解析（info、progress、error、single/playlist/livestream） | `../domains/download-engine/progress-and-diagnostics.md` |
| `models/` | 跨层数据结构（download/parsed/payloads/progress/error/ytdlp） | 同上 |
| `binaries/` | 二进制清单、下载、解压、版本记账 | `../domains/toolchain/backend-behavior.md` |
| `stronghold/` | 保险库状态与凭据读取 | `../domains/auth-secrets/backend-behavior.md` |
| `logging/` | 分组日志环形缓冲与订阅 | `observability.md` |
| `i18n.rs` | 内嵌后端 locale 与占位符渲染 | `../domains/app-lifecycle/backend-behavior.md` |
| `menu.rs` / `tray.rs` / `window.rs` | 原生菜单、托盘、窗口几何与关闭行为 | 同上 |
| `diagnostic_rules.json` | 错误码规则（`include_str!` 嵌入） | `../domains/download-engine/progress-and-diagnostics.md` |

### 命令注册面（26 个）

`app_ready`、`media_size`、`media_info`、`media_playlist_expand`、`media_download`、`group_cancel`、
`logging_subscribe`、`logging_unsubscribe`、`config_get`、`config_reset`、`config_set`、
`preferences_get`、`preferences_reset`、`preferences_set`、`binaries_check`、`binaries_ensure`、
`updater_check`、`updater_download`、`updater_install`、`stronghold_init`、`stronghold_status`、
`stronghold_keys`、`stronghold_get`、`stronghold_set`、`get_platform`、`notify`。

每个命令在 `commands/<group>/<name>.rs` 中定义并通过 `commands/mod.rs` 重导出；名称与文件名一一对应。
新增命令需要三处改动：命令文件、`commands/mod.rs`、`lib.rs` 的 `invoke_handler`（外加隔离白名单）。

### 异步与线程模型

| 场景 | 机制 |
| --- | --- |
| 调度器主循环 | `tauri::async_runtime::spawn` 单任务；空闲时 `recv().await` |
| 单条任务 | 每条目一个 `spawn`，持有 `OwnedSemaphorePermit`，结束自动释放 |
| 子进程 | `std::process::Command`；stdout/stderr 各一个 OS 线程按行读取；另一个线程 `wait()` |
| 一次性命令 | `spawn_blocking`（例如 `Command::output()`） |
| 文件 IO | `tokio::fs`；大目录操作 `spawn_blocking`（`fs_extra::move_dir`） |
| 配置写入 | 同步（命令线程内），副作用异步 spawn（信号量 resize） |

线程间通信统一走 `tokio::sync::mpsc::unbounded_channel` 或 `tokio::sync::watch`（取消信号）。

### 状态与生命周期

| 状态 | 类型 | 生命周期 |
| --- | --- | --- |
| `PathsManager` | 克隆型 | 进程 |
| `SharedConfig`/`SharedPreferences` | `Arc<JsonStoreHandle<T>>` | 进程 |
| `DownloadLimiter`/`FetchLimiter` | `Arc<DynamicSemaphore>` | 进程，随配置 resize |
| `FetchSender`/`DownloadSender` | 通道发送端 | 进程（调度任务常驻） |
| `BinariesState` | `AtomicBool` | 进程 |
| `LogStoreState` | `RwLock<LogStore>` | 进程（按 group 清理） |
| `StrongholdState` | 三个 `Mutex` | 进程 |
| `TrayState` | `Mutex<Option<TrayIconId>>` | 进程（可空） |
| `Mutex<UpdateStore>` | `Update` + `bytes` | 进程（安装后清空字节） |
| `RUNNING_GROUPS` | `LazyLock<Mutex<HashMap<.., watch::Sender<bool>>>>` | 进程 |
| 计数器（fetch/download） | `LazyLock<Mutex<HashMap<String, usize>>>` | 进程（条目数归零即删除） |

### 错误处理约定

- 命令返回 `Result<T, String>`（`Box<dyn Error>` → `to_string()`），或直接返回 `T`（不可失败的命令）。
- 内部错误通过 `tracing::warn!` + 可选 `sentry::capture_error` 上报，不 panic（例外见 media-queue 的 `media_download`）。
- 解析器/纯函数返回 `Result<_, String>` 或自定义错误枚举（`YtdlpDownloadError`、`YtdlpInfoFetchError`、`ExtractError`）。

## Boundaries

- 不做多窗口（只有 `main`）、不做移动端（`#[cfg_attr(mobile, ...)]` 只是模板残留）。
- 不使用数据库；持久化只有 JSON store、stronghold 快照与二进制元数据文件。
- 无后台常驻任务（除调度器与进程读取线程）。

## Contracts

- 命令/事件命名与序列化：`../shared/ipc-conventions.md`。
- 归属与生命周期：`../shared/data-ownership.md`。
- 目录与环境：`storage.md`。
- 后端约束：`../rules/backend.rules.md`。

## Failure And Edge Cases

- `LazyLock<Mutex<...>>` 全局表在测试中跨用例共享（Rust 单测需注意 group id 唯一，例如 dispatcher 测试用 `group-a`/`group-b`）。
- 读线程按 1 字节读取会为每行产生一次 syscall；大量日志（例如 `--verbose`）下是潜在性能热点。
- `panic` 会终止该 async 任务而不影响其它任务；`media_download` 的 `unwrap()` 是已知例外点。
- `unwrap()` 在 `lib.rs`/`window.rs` 中多处存在（窗口获取、菜单构建），属于"配置错误即崩溃"的取舍。

## Verification

- `cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test`。
- 完整应用冒烟：`npm run tauri dev`（验证插件注册、命令注册与隔离白名单一致）。
