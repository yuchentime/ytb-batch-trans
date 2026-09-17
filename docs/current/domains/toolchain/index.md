---
status: current
layer: domain
domain: toolchain
canonical_for:
  - toolchain-entry
related:
  - docs/current/domains/download-engine/index.md
  - docs/current/platform/release-and-distribution.md
last_verified: 2026-09-17
---

# toolchain 文档路由

## Canonical Docs

- `flow.md`：启动检查 → 安装页 → 下载校验 → 解压安装 → 版本记账的完整流程。
- `api-contract.md`：清单文件 schema、命令与事件契约。
- `backend-behavior.md`：`BinariesManager`/提取器/并发锁/环境形态。
- `frontend-behavior.md`：`App.vue` 启动检查、安装页、`tool-card`、`binaries` store。
- `verification.md`：验证方式与回归清单。

## 范围

责任单元：**让应用始终拥有一份可用且版本正确的 yt-dlp / ffmpeg**。
包含清单获取与签名校验、按平台选择产物、流式下载 + sha256 校验、解压与 hoist、版本记账（`metadata.json`）、
安装页 UI 与进度事件。不包含实际使用这些二进制的参数构造（download-engine）。

## Task Routes

| Task | Read |
| --- | --- |
| 新增/升级一个工具 | `index.md` -> `api-contract.md` -> `backend-behavior.md` -> `verification.md` |
| 改安装页体验 | `index.md` -> `flow.md` -> `frontend-behavior.md` -> `verification.md` |
| 排查安装失败 | `flow.md` -> `api-contract.md`（错误阶段） -> `backend-behavior.md` |
| 改签名/清单流水线 | `api-contract.md` -> `../platform/release-and-distribution.md` |

## Related Domains

- `../download-engine/index.md`：二进制消费方（`PATH` 前置 `bin_dir`）。
- `../app-lifecycle/index.md`：启动时序与 `app_ready`。
- `../settings-preferences/index.md`：`update.updateBinaries` 开关决定前端是否发起检查。

## Historical References

- 无。

## Update Rules

- 清单字段、事件名或错误阶段变化时必须同步 `api-contract.md`。
- 工具来源（URL/版本/解压方式）变化时必须同步 `backend-behavior.md` 与 `verification.md` 的手工步骤。
- 本领域新增文档时更新本文件与 `docs/manifest.yaml` 的 `toolchain` 路由。
