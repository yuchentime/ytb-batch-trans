---
status: current
layer: shared
domain: ipc-conventions
canonical_for:
  - ipc-conventions
related:
  - docs/current/platform/backend-runtime.md
  - docs/current/platform/frontend-runtime.md
last_verified: 2026-09-17
---

# IPC 约定（Tauri 命令与事件）

## Purpose

统一前端 `invoke`/`listen` 与后端命令/事件之间的命名、序列化、错误与安全约定，
使任何新增 IPC 都有唯一正确写法。

## Current Behavior

### 命令命名与注册

| 规则 | 说明 |
| --- | --- |
| 命名 | `snake_case`，动词+名词（`media_info`、`config_set`、`updater_download`） |
| 定义位置 | `src-tauri/src/commands/<group>/<name>.rs`，每个文件一个命令 |
| 导出 | `commands/<group>/mod.rs` → `commands/mod.rs` → `lib.rs` 的 `generate_handler!` |
| 隔离白名单 | 必须加入 `src-isolation/main.ts` 的 `allowedCommands`，否则前端调用抛 `Unauthorized command` |
| 前端调用 | `invoke<T>('<name>', { camelCaseArgs })` |
| 测试 mock | 必须加入 `tests/utils/mocks/*Handlers.ts`，否则 E2E 会超时 |

### 参数与序列化

- Rust 侧参数用 `snake_case`（如 `group_id`），前端传 `camelCase`（`groupId`）——Tauri v2 自动转换。
- 结构体统一 `#[serde(rename_all = "camelCase")]`；枚举用 `#[serde(rename_all = "lowercase")]`（如 `TrackType`）或 `camelCase`
  （如 `VideoContainer`）或 `UPPERCASE`（如 `InputFilterSizeUnit`），以前端 `tauri/types/*.ts` 的取值为准。
- 二进制数据（保险库）用 `number[]`；其余全部是 JSON 可表达类型。
- 前端类型必须与 Rust 结构体字段一一对应；Rust 侧的 `#[serde(default)]` 让缺失字段回落到默认值。

### 返回值

| 形态 | 使用场景 | 前端处理 |
| --- | --- | --- |
| `T` | 不可失败（`config_get`、`get_platform`、`media_download`） | 直接使用 |
| `Result<T, String>` | 有业务失败可能（大多数命令） | `try/catch` 或 `.catch()`；错误是字符串 |
| `()` | 纯副作用（`group_cancel`、`app_ready`） | 无返回值 |

- 错误字符串是 Rust 错误 `Display` 的原文（英文），**不面向用户本地化**；用户可见文案由前端 i18n 或诊断 code 生成。
- 命令不做参数校验，校验责任在前端（URL 合法性、group 存在性等）。

### 事件

| 规则 | 说明 |
| --- | --- |
| 命名 | `snake_case` 名词短语（`media_add`、`media_progress_stage`、`binary_download_error`） |
| 载荷 | 专用 payload 结构体（`models/payloads.rs` 或各领域模型），字段 camelCase |
| 发送 | `app.emit("<name>", payload)`；发射失败一律忽略（`let _ =`） |
| 订阅 | `src/tauri/listeners/<topic>.ts`，由 `src/plugins/tauriListeners.ts` 统一注册 |
| 广播范围 | 全局（无窗口定向）；同一载荷发给所有监听者 |

事件清单（按主题）：

| 主题 | 事件 |
| --- | --- |
| 队列/媒体 | `media_add`、`media_size`、`media_complete`、`media_fatal`、`media_diagnostic` |
| 进度 | `media_progress`、`media_progress_stage`、`media_destination` |
| 日志 | `logging_append` |
| 二进制 | `binary_download_start`、`binary_download_progress`、`binary_download_complete`、`binary_download_error`、`binary_update_complete` |
| 更新 | `updater_download_progress`、`updater_finished`、`updater_error` |
| 外壳 | `navigate`、`shortcut_action` |

### 一致性与无重放

- 事件没有序号、时间戳幂等键或重放机制；前端必须容忍重复与乱序（例如重复 `media_add` 覆盖同 id）。
- 所有事件都是"尽力而为"：进程重启/页面重载后不会补发。
- 事件与命令结果无关联 id（除载荷内的 `id`/`groupId`）；发送方需自行携带归属标识。

### 安全

- 前端只能调用白名单命令；插件能力由 `capabilities/default.json` 限制（opener 仅 `https:` URL 与任意路径打开、clipboard 只读文本）。
- CSP 禁止内联样式与外部脚本（`frontend-runtime.md`）。
- 事件载荷中不得包含密钥明文（见 `../rules/security.rules.md`）。

## Boundaries

- 不使用 Tauri 的 `Channel`/`ipc::Response` 流式 API：所有流式数据都通过事件表达。
- 不做版本化的 IPC 协议（前后端同版本发布）。
- 不使用 HTTP 语义（无状态码、无路径参数、无分页）。

## Contracts

- 各领域的字段级契约：`../domains/media-queue/api-contract.md`、`../domains/settings-preferences/api-contract.md`、
  `../domains/toolchain/api-contract.md`、`../domains/auth-secrets/api-contract.md`、`../domains/app-lifecycle/api-contract.md`。
- 命名细则：`naming.md`。

## Failure And Edge Cases

- 忘记加白名单 → 前端 `Unauthorized command`（隔离钩子抛错），dev 下表现为功能静默失效。
- 忘记加 mock → E2E 用例在 `invoke` 处挂起直到超时（不会明确报"未 mock"）。
- 命令返回 `Err` 时前端若未 catch 会产生未处理的 Promise rejection（Sentry 可能收到噪声）。
- `media_download` 的 `unwrap()` 是唯一"失败即 panic"的命令路径，见 `../domains/media-queue/backend-behavior.md`。

## Verification

- `tests/unit/tauriListeners.spec.ts` 验证监听注册齐全。
- 手工：改完 IPC 后必须跑一次 `npm run tauri dev`（验证白名单与真实序列化），再跑 E2E（验证 mock 完整）。
