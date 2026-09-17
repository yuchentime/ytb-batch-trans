---
status: current
layer: domain
domain: media-queue
canonical_for:
  - media-queue-entry
related:
  - docs/current/domains/media-queue/flow.md
  - docs/current/domains/download-engine/index.md
last_verified: 2026-09-17
---

# media-queue 文档路由

## Canonical Docs

- 本领域的能力、边界与状态见下方「范围」段（不单设 `overview.md`）。
- `flow.md`：从入队到下载完成/失败的端到端流程。
- `api-contract.md`：本领域拥有的 Tauri 命令与事件的完整字段契约。
- `data-model.md`：`Group`/`MediaItem`/`DownloadOptions`/`DownloadOverrides` 等数据模型。
- `frontend-behavior.md`：pinia store 分工、卡片步骤组件、状态迁移规则。
- `backend-behavior.md`：fetch/download 调度器、分组状态、编号、并发与清理。
- `error-handling.md`：fatal 与 diagnostic 的分流、group 级错误汇总、通知与跳过语义。
- `verification.md`：本领域的验证方式与回归清单。

## 范围

责任单元：**用户的一次入队从 URL 变成可下载的队列条目，直到把条目交给下载引擎**。
包含入队渠道、元数据抓取、播放列表拆分/合并与条目选择、队列卡片状态机、队列级操作（下载/暂停/恢复/重试/删除/清空）、
以及下载触发时下发给后端的请求组装。不包含 yt-dlp 参数细节（见 `download-engine`）、设置持久化（见 `settings-preferences`）。

## Task Routes

| Task | Read |
| --- | --- |
| 改动正常流程 | `index.md` -> `flow.md` -> `api-contract.md` -> `frontend-behavior.md` -> `backend-behavior.md` -> `verification.md` |
| 只改前端队列交互 | `index.md` -> `flow.md` -> `frontend-behavior.md` -> `data-model.md` -> `verification.md` |
| 只改调度/抓取 | `index.md` -> `flow.md` -> `backend-behavior.md` -> `api-contract.md` -> `verification.md` |
| 排查队列卡住/状态错乱 | `flow.md` -> `frontend-behavior.md` -> `error-handling.md` -> `verification.md` |

## Related Domains

- `../download-engine/index.md`：参数构造、进度、诊断。
- `../settings-preferences/index.md`：`inputFilters`、`performance.splitPlaylistThreshold`、下载目录。
- `../auth-secrets/index.md`：抓取与下载所需的认证参数来源。

## Historical References

- 无（本领域尚无 ADR/复盘记录）。

## Update Rules

- 新增入队渠道或队列操作时，必须同步 `flow.md` 的 Mermaid 图与 `api-contract.md` 的事件/命令表。
- 状态枚举或迁移规则变化时，先改 `frontend-behavior.md` 的迁移表，再改代码。
- 本领域新增文档时更新本文件与 `docs/manifest.yaml` 的 `media-queue` 路由。
