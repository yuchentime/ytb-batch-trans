---
status: current
layer: domain
domain: app-lifecycle
canonical_for:
  - app-lifecycle-entry
related:
  - docs/current/platform/backend-runtime.md
  - docs/current/platform/frontend-runtime.md
last_verified: 2026-09-17
---

# app-lifecycle 文档路由

## Canonical Docs

- `flow.md`：启动时序、窗口/托盘/菜单/快捷键/关闭行为、应用自更新流程。
- `api-contract.md`：`updater_*` / `get_platform` / `notify` 命令与 `navigate`/`shortcut_action` 事件。
- `frontend-behavior.md`：前端启动引导、布局与头部/底部、主题、i18n、更新提示条、窗口观察器。
- `backend-behavior.md`：`lib.rs` 装配顺序、窗口管理、托盘/菜单、快捷键、i18n 管理器、关闭策略。
- `verification.md`：验证方式与回归清单。

## 范围

责任单元：**应用的启动/退出与系统集成**（窗口、托盘、菜单、全局快捷键、单实例、自启动、关闭行为、主题、界面语言、通知时机）**以及应用自更新**。
不包含下载/队列业务（media-queue、download-engine）与二进制工具链（toolchain）。

## Task Routes

| Task | Read |
| --- | --- |
| 改启动/关闭行为 | `index.md` -> `flow.md` -> `backend-behavior.md` -> `verification.md` |
| 改托盘/菜单/快捷键 | `index.md` -> `backend-behavior.md` -> `api-contract.md` -> `verification.md` |
| 改界面语言/主题 | `index.md` -> `frontend-behavior.md` -> `backend-behavior.md`（i18n） -> `verification.md` |
| 改应用更新 | `index.md` -> `flow.md`（更新段） -> `api-contract.md` -> `verification.md` |

## Related Domains

- `../settings-preferences/index.md`：`system`/`appearance`/`update`/`notifications` 字段来源与副作用。
- `../toolchain/index.md`：启动时的二进制检查与安装页。
- `../media-queue/index.md`：通知的业务触发点。

## Historical References

- 无。

## Update Rules

- 启动时序变化必须同步 `flow.md` 的时序图；新增命令/事件必须同步 `api-contract.md`。
- i18n key 变化必须同时改前后端两份 locale（见 `../shared/naming.md`）。
- 平台差异（Windows/macOS/Linux）必须显式标注，不得只描述单一平台行为。
