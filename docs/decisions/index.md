---
status: current
layer: decisions-index
canonical_for:
  - decisions-entry
last_verified: 2026-09-17
---

# decisions/ 使用说明（ADR）

记录**长期有效**的技术取舍：为什么选了这个方案、放弃了什么、在什么条件下应当重新评估。
模板：`docs/templates/adr-template.md`；命名：`ADR-XXXX-<slug>.md`。

## 何时写 ADR

- 影响多个领域或难以回滚的选择（持久化格式、并发模型、安全边界、进程/运行形态）；
- 有明确替代方案且当时比较过；
- 后续开发者很可能问"为什么不那样做"。

不需要 ADR 的情况：可逆的小决策、纯实现细节、已被 change 的 `Design Deviations` 覆盖的临时偏差。

## 现有隐含决策（尚未写成 ADR）

以下取舍目前已固化在代码与 `docs/current/` 中，若发生争议应补写 ADR：

| 主题 | 现状 | 事实出处 |
| --- | --- | --- |
| 用调度器 + 动态信号量统一并发（而非每命令自管并发） | `GenericDispatcher` + `DynamicSemaphore` | `domains/media-queue/backend-behavior.md` |
| 抓取与下载使用两个独立信号量 | 同时进行时瞬时并发可达 2× 上限 | 同上 |
| 队列不持久化（重启即清空） | 会话内存模型 | `product/overview.md`、`shared/data-ownership.md` |
| 凭据放 stronghold + 系统钥匙串 | 不写配置文件 | `domains/auth-secrets/*` |
| 二进制工具由签名清单托管（而非随包分发） | yt-dlp/ffmpeg 运行时下载 | `domains/toolchain/*` |
| 事件驱动而非轮询（进度/状态） | `app.emit` + 前端 store | `shared/ipc-conventions.md` |
| 隔离模式 + 命令白名单 | `src-isolation` | `platform/frontend-runtime.md` |
| 诊断码配置化（规则文件） | 不在代码散落字符串 | `domains/download-engine/progress-and-diagnostics.md` |
| 文件模板渲染保留未知占位符并消毒路径 | 交由 yt-dlp 填未知字段 | `domains/download-engine/api-contract.md` |
| `docs/` 同时承载官网与文档库 | 官网为静态文件，Markdown 新增不影响构建 | `docs/README.md` |

## 约束

- ADR 一旦被取代，保留原文并标注 `superseded_by`（不要删除）。
- ADR 只记录决策与理由，不重复 `current/` 的事实；事实变更时改 `current/`，决策变更时写新 ADR。

## Verification

- 新增 ADR 后更新本文件与 `docs/manifest.yaml` 的 `history_on_demand`（如被某领域引用）。
