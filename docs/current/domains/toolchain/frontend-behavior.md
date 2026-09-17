---
status: current
layer: domain
domain: toolchain
canonical_for:
  - toolchain-frontend-behavior
related:
  - docs/current/domains/app-lifecycle/frontend-behavior.md
last_verified: 2026-09-17
---

# toolchain 前端行为

## Purpose

说明前端何时检查/安装二进制、安装页的状态与失败呈现、以及 `binaries` store 的数据流。

## Current Behavior

### 启动检查（`src/App.vue`）

```
checkTools() -> binariesStore.check()
  - 若 settings.update.updateBinaries === false -> 直接返回 []
  - invoke('binaries_check') -> { tools: string[] }
  - 初始化 binariesStore.tools = { [tool]: { total: 0, percent: 0, received: 0 } }
  - tools.length > 0 -> router.push('/install')
```

调用是 fire-and-forget（`void checkTools()` 外层 try/catch），失败只 `console.error`，不阻塞应用启动。

### `binaries` store（`src/stores/binaries.ts`）

| 成员 | 说明 |
| --- | --- |
| `tools: Record<string, BinaryProgress>` | 每个工具 `{ received, total, percent, version?, error? }` |
| `check()` | 受 `update.updateBinaries` 门控；重置 `tools` 后按后端返回初始化 |
| `ensure(toEnsure?)` | `invoke('binaries_ensure', { tools: toEnsure ?? Object.keys(tools) })`；**不捕获错误**，由调用方处理 |
| `processBinaryDownloadStart` | 写入 `version` |
| `processBinaryDownloadProgress` | 计算 `percent = round(received/total*100)`（`total=0` 时为 `NaN`，会显示异常） |
| `processBinaryDownloadComplete` | `received = total`、`percent = 100` |
| `processBinaryDownloadError` | 写 `error = "[stage] error"` |

事件订阅在 `src/tauri/listeners/binaries.ts` 注册（`binary_download_start/progress/complete/error`）。
**注意**：`binary_update_complete` 未订阅。

### 安装页（`src/views/full/InstallView.vue`）

- 路由：`/install`，使用 `FullLayout`（无 Header/Footer），页面在应用启动早期可独立显示。
- `onMounted` → `binariesStore.ensure()`：
  - 成功 → `isInstalling = false`，启动 5 秒倒计时，倒计时结束后可点击"继续"进入主页。
  - 抛错 → `installPartialFail = true`，标题/副标题切换为失败文案，按钮始终可点（允许带着失败的工具继续）。
- `tools` 列表逐项渲染 `tool-card`（名称、版本、进度条、错误文案）。
- 通知与国际化：使用 `install.*` i18n key。

### `tool-card`（`src/components/tool-card/ToolCard.vue`）

展示单个工具：名称、目标版本、进度百分比、错误信息；错误时以 `alert-error` 呈现（`[stage] message` 原文）。

### 缓存与版本提示

安装成功后 `metadata.json` 记录版本，下次启动 `check` 不再返回该工具，因此安装页只在"确实需要"时出现。

## Boundaries

- 前端不做 hash/签名校验，也不感知下载 URL；只消费事件。
- 安装过程中用户无法取消（没有取消按钮与 IPC）。
- 安装页不订阅 `binary_update_complete`，因此"部分失败"靠 `ensure` 的 reject + 各工具 `error` 字段体现。

## Contracts

- 事件契约：`docs/current/domains/toolchain/api-contract.md`。
- 设置门控：`settings.update.updateBinaries`（默认 `true`），见 `../settings-preferences/data-model.md`。

## Failure And Edge Cases

- `binaries_check` 失败（离线）→ 不跳转安装页，用户进入主界面；之后的下载会因缺少 yt-dlp 失败。
- `progress.total === 0`（无 `Content-Length`）→ `percent` 为 `NaN`；进度条渲染取决于组件对 `NaN` 的处理。
- 用户停留在安装页时再次热更新/路由跳转可能重复调用 `ensure`，后端独占锁会静默忽略第二次调用。
- 5 秒倒计时期间用户点击"继续"立即跳转；倒计时不阻塞实际安装完成（安装已完成才会进入该状态）。

## Verification

见 `docs/current/domains/toolchain/verification.md`；E2E：`tests/e2e/binaries.spec.ts`。
