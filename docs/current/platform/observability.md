---
status: current
layer: platform
domain: observability
canonical_for:
  - observability-facts
related:
  - docs/current/domains/download-engine/progress-and-diagnostics.md
  - docs/current/rules/security.rules.md
last_verified: 2026-09-17
---

# 可观测性

## Purpose

说明应用产生哪些日志/错误信号、它们存在哪里、谁能看到、以及上报第三方时的边界。

## Current Behavior

### 三类信号

| 信号 | 面向 | 去向 | 生命周期 |
| --- | --- | --- | --- |
| `tracing` 日志 | 开发者 | stdout/stderr（fmt 层）+ Sentry（事件层） | 进程输出；不落文件 |
| 分组日志缓冲 | 用户（Logs 页） | `LogStoreState` 环形缓冲 → `logging_append` 事件 | 进程内，超限丢最旧 |
| 诊断事件 | 用户 + Sentry（手动） | `media_diagnostic` / `media_fatal` 事件 → 前端 store → 诊断卡片 | 进程内 |

### tracing 配置（`lib.rs::init_tracing`）

- fmt 层级别：debug 构建 `DEBUG`，release `INFO`；被屏蔽的 target：`tauri_plugin_updater`、`tao::...::event_loop::runner`、`h2`、`hyper_util`。
- Sentry 层：同样过滤规则；`sentry::init` 的 `traces_sample_rate = 0.05`、`sample_rate = 0.25`，`release` 取 crate 版本。
- `ClientInitGuard` 被 manage 到应用状态中（否则 Sentry 客户端会在 setup 结束时被丢弃）。
- 关键日志点：环境形态与目录（`paths.rs`，debug）、每次 yt-dlp 运行的摘要（`log_run_summary`，info）、调度器调度/完成（trace）、失败原因（warn）。

### 分组日志（`logging/`）

- `LogStore { groups: IndexMap<String, GroupLog>, total_bytes, max_bytes, subscribed: HashSet<String> }`。
- 每个 group 记录 `VecDeque<String>` 与字节数；超过上限时从最旧开始丢弃，直到低于上限。
- 订阅：`logging_subscribe(groupId)` → 返回当前全文并登记订阅；之后新行通过 `logging_append{groupId, line}` 事件推送；`logging_unsubscribe` 取消。
- 清理：`group_cancel` 会 `remove_group` 该 group 的日志；下载完成后日志仍保留（直到用户取消/删除 group）。
- 写入者：`ytdlp_download`（跳过空行与 `RAW` 开头行）与 `ytdlp_info`（stderr 全部 + 解析用的 stdout 行）。

### 诊断事件

- 产生：`YtdlpErrorParser`（`ERROR:`/`WARNING:` 前缀与跳过文案）→ `media_diagnostic`；失败路径 → `media_fatal`。
- 存储：前端 `media-diagnostics` store（`diagnostics[id]` 数组、`fatals[id]` 单值）。
- 展示：`DiagnosticCard`（按 group 聚合，`useDiagnostic` 本地化 message/shortMessage，支持复制原文与上报）、`TheMediaLogs`（原始日志）。
- 详见 `../domains/download-engine/progress-and-diagnostics.md`。

### Sentry 边界

| 来源 | 内容 | 是否含用户数据 |
| --- | --- | --- |
| 后端 `sentry::capture_error` | 错误类型与 `Display` 文本（spawn 失败、规则文件损坏、事件流中断、解析失败等） | 可能含 URL/文件路径；**不含凭据**（`RunLogSummary` 只记录布尔） |
| 后端 tracing→Sentry 层 | 带 `warn!`/`error!` 的日志（受采样率影响） | 同上 |
| 前端 Vue 集成 | 未捕获异常、console 错误、路由事务（browserTracing） | 取决于异常内容 |
| 用户手动上报 | 诊断卡片的 `message`（tag `user-reported=true`） | 用户确认后发送，可能含 yt-dlp 输出 |

`__DEV__` 与 `__E2E__` 下前端 Sentry 不初始化；后端 Sentry 始终初始化（包括 debug 构建）。

### 排障入口（按用户可提供的信息排序）

| 信息 | 获取方式 |
| --- | --- |
| 环境形态与目录 | 启动日志（debug 构建）或让用户报告安装形态 |
| yt-dlp 命令行摘要 | 日志中的 `Running yt-dlp command`（`arg_count`/`has_*`） |
| 具体命令与参数 | `tracing::debug!("Running command: yt-dlp {}", args)`（仅 debug 构建；**含 URL 但不含凭据值**） |
| 原始 yt-dlp 输出 | 媒体详情页 Logs 标签（用户可复制） |
| 诊断码 | 卡片上的本地化消息（code 对应 `errors.runner.<code>`） |
| 崩溃堆栈 | Sentry（若用户开启上报） |

## Boundaries

- 不写日志文件、不做日志轮转；用户重启应用后旧日志消失。
- 没有结构化指标（metrics）与分布式追踪；只有 Sentry 的错误/事务采样。
- 前端没有日志缓冲（`console.*` 只到 WebView 控制台）。

## Contracts

- 诊断码全集与扩展方式：`../domains/download-engine/progress-and-diagnostics.md`。
- 禁止项（凭据、明文密钥进日志）：`../rules/security.rules.md`、`../rules/forbidden.rules.md`。
- 事件字段：`../domains/media-queue/api-contract.md`。

## Failure And Edge Cases

- 日志缓冲上限是全局字节数（跨 group），因此一个超长输出的 group 会挤掉其它 group 的日志。
- `logging_append` 只推送给已订阅的 group；用户在 Logs 页切换 group 会先 unsubscribe 再 subscribe（`useGroupLog` 的 watch）。
- 抓取路径把 stdout 全部行（JSON 结果）也写进日志，因此单个大播放列表会产生很长的日志。
- 后端 Sentry 在 debug 构建同样上报，本地开发可能污染 Sentry 项目的统计（采样率 0.25 缓解）。
- 用户手动上报依赖 `code === 'unknown'`：已知错误码不会出现上报按钮。

## Verification

- 观察：`npm run tauri dev` 中触发一次成功下载与一次失败下载，检查 stdout 摘要、Logs 页面内容与诊断卡片。
- 不泄漏回归：`cd src-tauri && cargo test` 中的 `summary_*` 用例。
- 覆盖率：CI 上传 `coverage/{units,e2e,rust}` 到 Codecov（`.codecov.yaml` 配置忽略 `src-tauri/**` 与测试工具目录等，见该文件）。
