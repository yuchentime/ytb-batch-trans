---
status: current
layer: domain
domain: settings-preferences
canonical_for:
  - settings-preferences-entry
related:
  - docs/current/domains/download-engine/api-contract.md
  - docs/current/platform/storage.md
last_verified: 2026-09-17
---

# settings-preferences 文档路由

## Canonical Docs

- `flow.md`：加载 → 编辑 → 深合并写入 → 副作用 → 重置的完整流程。
- `data-model.md`：`Config`（设置）与 `Preferences`（偏好）的完整字段、默认值与语义。
- `api-contract.md`：`config_*` / `preferences_*` / `notify` 命令契约。
- `frontend-behavior.md`：settings/preferences store、设置页草稿机制、各设置页职责。
- `backend-behavior.md`：`JsonStoreHandle`、深合并、`on_updated` 副作用、窗口几何防抖。
- `verification.md`：验证方式与回归清单。

## 范围

责任单元：**跨会话持久化的全局设置（Config）与本地偏好（Preferences）**。
包含 schema、默认值、深合并 patch 语义、写入时机、写入后的副作用（并发上限/快捷键/托盘/语言/自启动）、
设置界面与下载位置界面。不包含组级 override（media-queue）与密钥（auth-secrets）。

## Task Routes

| Task | Read |
| --- | --- |
| 新增一个设置项 | `index.md` -> `data-model.md` -> `api-contract.md` -> `frontend-behavior.md` -> `backend-behavior.md` -> `verification.md` |
| 改设置界面 | `index.md` -> `frontend-behavior.md` -> `data-model.md` -> `verification.md` |
| 改持久化/合并语义 | `index.md` -> `backend-behavior.md` -> `data-model.md` -> `verification.md` |
| 排查设置不生效 | `backend-behavior.md`（on_updated） -> `flow.md` -> `verification.md` |

## Related Domains

- `../media-queue/index.md`：`inputFilters`、`performance.splitPlaylistThreshold`、`output.*` 的消费者。
- `../download-engine/index.md`：`OutputSettings`/`SubtitleSettings` 等被解析成 yt-dlp 参数。
- `../app-lifecycle/index.md`：`system`/`appearance`/`update`/`notifications` 的副作用执行方。

## Historical References

- 无。

## Update Rules

- 新增字段必须同时更新 `data-model.md`（含默认值）、`src/tauri/types/config.ts` 的默认值对象、Rust `Default` 实现与 `verification.md` 的用例。
- 行数超预算时按「下载/输出」「网络/认证」「系统/外观」拆分文档，而不是压缩表格。
