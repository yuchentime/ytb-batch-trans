---
status: current
layer: domain
domain: download-engine
canonical_for:
  - download-engine-entry
related:
  - docs/current/domains/media-queue/flow.md
  - docs/current/platform/observability.md
last_verified: 2026-09-17
---

# download-engine 文档路由

## Canonical Docs

- `flow.md`：从 `DownloadEntry` 到进程退出的事件循环与责任节点。
- `api-contract.md`：与 yt-dlp 的外部契约（参数矩阵、stdout/stderr 行格式、退出码）。
- `backend-behavior.md`：`YtdlpRunner`、进程控制、取消、Sentry 上报策略。
- `progress-and-diagnostics.md`：进度/阶段/目标路径解析与诊断规则文件。
- `verification.md`：验证方式与回归清单。

## 范围

责任单元：**把一条下载任务翻译成 yt-dlp 调用、执行、并把执行结果转成前端可消费的事件**。
包含参数构造（format/output/location/network/auth/subtitle/sponsorblock/input-filters）、进程启动与终止、
逐行解析（进度、阶段、目标路径、诊断）、日志落缓冲、取消与错误映射。
不包含队列编排（media-queue）与设置 schema（settings-preferences）。

## Task Routes

| Task | Read |
| --- | --- |
| 改参数构造 | `index.md` -> `api-contract.md` -> `backend-behavior.md` -> `verification.md` |
| 改进度/阶段 | `index.md` -> `progress-and-diagnostics.md` -> `api-contract.md` -> `verification.md` |
| 改错误识别 | `index.md` -> `progress-and-diagnostics.md` -> `backend-behavior.md` -> `verification.md` |
| 排查下载卡住/无输出 | `flow.md` -> `backend-behavior.md` -> `progress-and-diagnostics.md` |

## Related Domains

- `../media-queue/index.md`：谁产生 `DownloadEntry`、谁消费事件。
- `../settings-preferences/index.md`：`OutputSettings`/`SubtitleSettings`/`NetworkSettings` 等 base 值。
- `../auth-secrets/index.md`：`AuthSecrets` 注入点。

## Historical References

- 无。

## Update Rules

- 参数矩阵变更必须同步 `api-contract.md`；解析格式变更必须同步 `progress-and-diagnostics.md`。
- 新增诊断码时必须改 `src-tauri/src/diagnostic_rules.json`（而不是在解析器里加 `if`），并同步前端 i18n 的 `errors.runner.<code>`。
- 本领域新增文档时更新本文件与 `docs/manifest.yaml` 的 `download-engine` 路由。
