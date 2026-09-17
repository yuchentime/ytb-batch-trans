---
status: current
layer: domain
domain: media-queue
canonical_for:
  - media-queue-error-handling
related:
  - docs/current/domains/download-engine/progress-and-diagnostics.md
  - docs/current/platform/observability.md
last_verified: 2026-09-17
---

# media-queue 错误处理

## Purpose

规定队列侧两类错误信号（fatal / diagnostic）的分流、状态影响、用户可见反馈与跳过语义，
避免"错误被吞掉"或"跳过被当成失败"。

## Current Behavior

### 两类信号

| 信号 | 产生位置 | 语义 | 前端影响 |
| --- | --- | --- | --- |
| `media_fatal` | 非 0 退出、spawn 失败、JSON 解析失败、诊断规则文件损坏、直播、IPC 异常、事件流中断 | 该 item 彻底失败，不会再有后续进度 | item → `error`，`group.errored++`，弹 `downloadFailed` 通知（非合并组路径） |
| `media_diagnostic` | stdout/stderr 中以 `ERROR:` / `WARNING:` 开头的行，或匹配跳过行 | 可解释的问题或警告 | 仅记录，不改变状态；由日志页/卡片展示 |

`DiagnosticLevel` 只有 `error`/`warning` 两级；`code` 由 `diagnostic_rules.json` 决定，未命中为 `unknown`。

### fatal 的 group 级汇总（`useMediaDiagnosticsStore.processMediaFatalPayload`）

1. 写入 `fatals[id]`。
2. 若 group 当前是 `fetching`/`fetchingList`：`rejectPendingReadyGroup`，让 `waitForGroupReady` 的等待者立即失败（"add and download" 不会静默卡住）。
3. `group.processed++`、`group.errored++`。
4. **播放列表 leader 存在**（即合并/播放列表组）：
   - 若仍在 `fetchingList`，直接返回（抓取阶段的失败只计数，不把卡片打到 error）。
   - 否则把该 item 置 `error`，并检查所有非 leader item 是否都已终结：
     - 全部 `done` → 组 `done`；
     - 全部 `error` → 组 `error` + `downloadFailed` 通知；
     - 混合 → 组 `done`（部分成功不视为整组失败）。
5. **无 leader**：若非合并组且 `processed === total > 1`，先 `finalizePlaylistGroup`（保证拆分组能创建出来），再把组置 `error` 并通知。

关键取舍：**只有全失败才把组标成错误**；部分失败通过卡片上的 `失败 N / 跳过 M` 计数暴露。

### 跳过（`input_filter_skipped`）

- 由 `YtdlpErrorParser::parse_skip_line` 识别，固定 `code = "input_filter_skipped"`、`level = warning`、`component = "download"`。
- 目前识别的三类原因：`upload date is not in range`、`file is larger than max-filesize`、以及同类体积/日期跳过文案（见 `parsers/ytdlp_error.rs` 的 `normalize_skip_message`）。
- 前端 `countSkippedDiagnostics(diagnostics)` 统计后展示为"跳过"，与 `group.errored` 分开显示（`MediaConfigureStep` 的 `itemOutcomeDisplay` 有四种组合文案）。
- 跳过**不计入** `group.errored`，也不触发任何通知。

### 诊断码的用户可见化

- `useDiagnostic(diagnostic)`：按 `code` 查 `errors.runner.<code>` 的 i18n 文案（`message`/`shortMessage`），失败时回退到后端 `message`。
- `code === 'unknown'` 且 `level === 'error'` 时按钮允许上报 Sentry（`Sentry.captureMessage`，tag `user-reported=true`）。
- `relatedFatal` 会把同 id 的 fatal 关联到诊断卡片上，形成"诊断 + 终止原因"的完整叙述。

### 其它错误路径

| 场景 | 处理 |
| --- | --- |
| 入队 URL 非法 | 入口层丢弃，不入队（无错误信号） |
| `media_info` IPC reject | `dispatchMediaInfoFetch` 的调用方 catch：拖放/导入显示错误 toast，快捷键路径静默失败 |
| `media_download` IPC reject | `downloadGroup` 构造本地 `MediaFatalPayload`（`internal=true`）写入 diagnostics，并 `console.error` |
| `expandPlaylistGroup` 无匹配条目 | 抛 `Error('No playlist entries match the selected range.')`，由 `PlaylistSelectionStep` toast |
| 抓取到的 group 已被删除 | `processMediaAddPayload` / `processMediaFatalPayload` 抛 `Orphaned media item found`（HMR 或快速删除竞态） |
| 重试时 group.url 为空 | toast `media.card.toasts.retryError`，不发起调用 |

### 错误可见位置

- 卡片：`MediaErrorStep`（最后一条 fatal 的消息 + 重试按钮 + 相关诊断入口）。
- 媒体详情页：`DiagnosticCard`（按 group 聚合，支持复制/上报）、`TheMediaLogs`（原始行）。
- 通知：`downloadFailed`（仅合并组全失败或单条失败路径触发一次）。
- 窗口：`error` 状态卡片带 `border-error` 描边（`MediaCard.statusOutline`）。

## Boundaries

- fatal 只影响单个 item（合并组除外），不会暂停其它 group。
- 后端不会重试；"重试"由用户显式触发且等价于重新抓取。
- 诊断去重只在展示层做（同 id 保存全部事件），不做频率限制。

## Contracts

- 诊断规则文件的结构与全部 code：`docs/current/domains/download-engine/progress-and-diagnostics.md`。
- i18n key 约定：`errors.runner.<code>.message` / `.shortMessage`（前端 locale 文件）。
- 通知种类与策略：`docs/current/domains/app-lifecycle/backend-behavior.md`。

## Failure And Edge Cases

- 同一 item 可能收到多条 `media_fatal`（例如 spawn 失败后又收到终止事件）；前端以 `fatals[id]` 覆盖，只保留最后一条。
- `group.errored` 会在抓取阶段与下载阶段累积，UI 文案需同时覆盖两种语境。
- 若 `media_fatal` 在 group 被删除后到达，前端抛错并中断该次处理，不写入任何 store。
- 诊断规则文件损坏属于内部错误：会发 `internal=true` 的 fatal，并上报 Sentry（`InvalidDiagnosticRules`）。

## Verification

- 单测：`tests/unit/skippedDiagnostics.spec.ts`、`tests/unit/mediaStore.spec.ts`（错误路径）、`tests/unit/mediaProgress.spec.ts`。
- E2E：`tests/e2e/download-progress.spec.ts` 覆盖失败与进度事件的界面表现。
- 规则文件回归：`cargo test`（`parsers/ytdlp_error.rs` 相关）与 `docs/current/domains/download-engine/verification.md`。
