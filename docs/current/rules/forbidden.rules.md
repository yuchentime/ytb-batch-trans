---
status: current
layer: rules
domain: forbidden
canonical_for:
  - forbidden-practices
related:
  - docs/current/rules/security.rules.md
  - docs/current/rules/architecture.rules.md
last_verified: 2026-09-17
---

# 禁止项

以下做法在本仓库中被明确禁止。违反即视为缺陷，必须在 review 阶段拦下。

## 数据与安全

| 禁止 | 原因 | 正确做法 |
| --- | --- | --- |
| 把密码/Token/请求头明文写入 JSON 配置、日志、事件、Sentry | 泄漏 | stronghold + 钥匙串；日志只记布尔摘要 |
| 在事件载荷或命令参数里传明文机密 | IPC 可见性 | 传字节数组，仅在后端解密使用 |
| 前端直接读写 `vault.hold` 或钥匙串 | 破坏加密边界 | 走 `stronghold_*` 命令 |
| 新增遥测/上报点（除 Sentry 与用户手动上报） | 隐私承诺 | 先改产品文档并评审 |
| 放宽 CSP、加入 `unsafe-inline`、引入外部脚本/CDN | XSS 面 | 使用本地资源与 Tailwind 类 |
| 把远端文本当 HTML 插入（`v-html`、`innerHTML`） | XSS | 文本插值 + `useLinkify` |
| 用 shell 字符串拼接执行 yt-dlp/ffmpeg | 注入 | `Command::args` / `shlex` 校验后传参 |

## 架构与状态

| 禁止 | 原因 | 正确做法 |
| --- | --- | --- |
| 在命令里直接 spawn yt-dlp（绕过调度器） | 并发与取消失效 | 入队 `GenericDispatcher` |
| 新增独立并发控制（自建信号量、sleep 节流） | 破坏统一上限 | 复用 `DynamicSemaphore` |
| 用"忽略事件"实现取消 | 进程泄漏 | 走 `group_state` 取消通道并杀进程树 |
| 组件直接 `invoke` 业务命令或直接改 store 原始 ref | 状态不一致 | 经 store 动作 |
| 复制已有换算逻辑（模板/语言/格式匹配） | 双份实现漂移 | 上移到 `helpers/` 或后端模块 |
| 新增持久化文件而不走 `JsonBackedState` | 无默认值/无迁移 | 实现 `JsonBackedState` |
| 后端引用前端概念（`MediaState`、`Group`） | 分层破坏 | 只接受快照结构（`DownloadItem`） |

## 流程与代码质量

| 禁止 | 原因 | 正确做法 |
| --- | --- | --- |
| 有 `design.md` 的改动未过 Design Review 就开写代码 | 跳过硬门禁 | 先写 `reviews/design-review.md` 并取 `DESIGN_REVIEW_PASS` |
| 修改已发布的诊断码字符串（重命名/复用） | 破坏前端 i18n 与历史日志 | 新增 code，旧 code 保留或标记废弃 |
| 修改 `--progress-template` 而不改解析器字段索引 | 进度静默失效 | 同步改 `ytdlp_progress.rs` 与契约文档 |
| 重命名 `STRONGHOLD_KEYS` 的键名而不做迁移 | 丢失用户凭据 | 保持键名稳定，必要时写迁移 |
| 新增命令/事件而漏改隔离白名单或 E2E mock | 功能静默失效/测试挂起 | 三处同步（handler、白名单、mock） |
| 在 `current/` 文档中留待办标记或"尚未实现" | 事实库失效 | 未定内容放 `source/` 或 change 目录 |
| 无测试的 store/helper 逻辑改动 | 回归不可控 | 至少一个 Vitest 用例 |
| 大范围"顺手重构" | 评审与回滚困难 | 拆成独立 change |
| 提交 `console.log`/`dbg!`/`println!` 调试残留 | 噪声与泄漏面 | `tracing`/`console.warn`，或删除 |

## Verification

- Review 时按本表逐项对照；安全类改动同时对照 `docs/evals/security-checklist.md`。
- 结构性检查：`python docs/scripts/check_doc_runtime.py docs`（文档层）+ 代码层 lint/test。
