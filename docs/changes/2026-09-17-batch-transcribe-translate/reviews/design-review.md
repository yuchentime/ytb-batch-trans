---
status: approved
layer: change
review_type: design_review
created_at: 2026-09-17
design_path: docs/changes/2026-09-17-batch-transcribe-translate/design.md
verdict: PASS_WITH_WARNINGS
confirmation_status: approved
---

# Design Review: batch-transcribe-translate

## Summary

| Field | Value |
|---|---|
| Design | `docs/changes/2026-09-17-batch-transcribe-translate/design.md` |
| Verdict | PASS_WITH_WARNINGS |
| confirmation_status | approved |
| DESIGN_REVIEW_PASS | passed |

## Scope Anchor

- Original ask：把下载工具改造成"多链接 → 每视频两份 txt（英文原稿 + 中文译稿）"，译稿由 AI（DeepSeek）
  生成且**语义不得增删**（口误可微调），原稿**保持原文**；只下最低成本媒体（已确认 `bestaudio`）、
  GPU 转录（whisper，默认 `small`）、转录后删除媒体；应用**直接改造覆盖**下载用途。
- Design goals in scope：G1–G6 与 ask 一一对应；Q1/Q4/Q11/Q15 的决策已写入 Problem/Proposed Behavior。
- Design items outside ask（合理外延）：批次汇总 `summary.md`、`/setup` 环境页、隔离白名单与 i18n 同步、
  `docs/current` 的同步更新——都是"覆盖式改造"不可避免的连带工作，且都有明确归属。

## Dimensions

### Complexity — Architecture

| Check | Result | Notes |
|---|---|---|
| Scope matches Goals/Non-Goals | pass | 六个目标与非目标边界清楚；已删除能力有明确清单 |
| No premature new abstraction layer | warning | 新增 `transcribe-translate` 领域是必要的（真实阶段归属）；但 `runners/ffmpeg_runner.rs` 与 `runners/whisper_runner.rs` 会再抄一份进程/取消/Job Object 逻辑，应**显式复用** `runners/ytdlp_process.rs` 原语而不是新写一套 |
| No speculative generalization | warning | `translation.targetLanguage`、`transcription.silenceAligned`、`transcription_download_model`、`translation_probe` 都是"以后可能需要"；第一期没有第二个消费者 |
| Cross-cutting surface growth justified | warning | 配置 schema 增删 + 新命令/事件 + 状态机扩展 + `media_size` 移除；面很大，但有 AC-18 兼容性护栏与隔离白名单流程兜底 |
| Operational weight justified | warning | 引入付费第三方（DeepSeek）与 461MB 模型下载，带来成本/隐私/限流；缓解手段（块级续跑、用量可见、`maxRetries` 退避、UI 明示）已写入设计 |
| Alternatives Considered includes a simpler option | pass | 含"固定切分""阶段级队列""本地 LLM""整篇翻译"等更简单/更重的对照方案 |
| Touch/complexity budget reasonable | warning | 3 个阶段 21 项任务，同时删除下载向代码与跨 6 个领域的文档；单次 change 偏大，需要用裁剪项压缩第一期 |

### Complexity — Data Model

| Check | Result | Notes |
|---|---|---|
| New tables justified vs reuse | pass | 无数据库；产物落在文件系统 `.work/`，复用现有 Group/Item 与限流器 |
| Alterations / migrations scoped | pass | 配置只做"新增 + 删除字段"，无迁移代码；`materialize` 深合并保证旧文件可加载（AC-18） |
| Cross-domain FK / shared tables justified | pass | `ai.apiKey` 复用 stronghold；不改已有 5 个认证键名 |
| Rollback / backfill cost acceptable | pass | 回滚 = `git revert`；用户输出目录与 `.work` 不受影响；旧配置字段仍留在文件中 |
| No speculative schema for unused futures | warning | `targetLanguage`、`output.chineseFileName` 可配置属于过度设计；建议固定 `zh-Hans` 与固定文件名，减少配置面与文档维护 |

### Verification — Acceptance Criteria

| Check | Result | Notes |
|---|---|---|
| AC 存在且编号连续 | pass | AC-01…AC-20，覆盖门禁/参数/分块/合并/原稿保真/翻译契约/密钥/重试/跳过/清理/取消/并发/兼容/汇总/无 panic |
| AC 代码接地 | warning | 引用既有实体（`state/json_state.rs::materialize`、`commands/group/group_cancel.rs`、`DynamicSemaphore`、stronghold 键名）成立；新模块路径尚不存在，属新建代码——AC 评审阶段需标注"实现后核验路径"，不得据现状声称已验证 |
| AC 可判定 | pass | 每条都有前置条件 + 预期 + 禁止项 + 判定方式；不含"体验良好"类模糊项 |
| AC 粒度合适 | pass | 停在关键判断点（契约校验、并发上限、清理时机、兼容性），未下沉到伪代码 |
| 与 design.md 目标对应 | pass | G2→AC-09/10/11；G3→AC-02/03/04/17；G4→AC-05/06/07/08/14；G5→AC-12/13；G6→AC-14/15/18/19 |
| Must-Not-Break 业务语义保留 | pass | M1–M6 与 AC 并存，未被 AC 替代 |

### Compatibility & Docs Impact（本门禁新增维度）

| Check | Result | Notes |
|---|---|---|
| 旧配置/旧密钥兼容 | pass | AC-18 + 不改 stronghold 已有键名 |
| 被删除能力有明确去向 | warning | 下载向 UI/参数/配置被删除后，`docs/current/domains/download-engine/api-contract.md` 等文档会大面积失效；设计已列 Current Doc Updates，但**文档工作量与实现量相当**，需要单独排期（建议 Phase C 处理，且旧文档移入 `archive/superseded/`） |
| 用户可见承诺不被破坏 | pass | 认证/代理/工具链/外壳行为均列入 Must-Not-Break 并各有 AC 或手工项 |

### Observability / Logging（增量评审，2026-09-17 开发者追加需求）

需求：长链路必须有一个 `.log` 文件，随时记录关键事件与错误，方便出错时即时排查。

| Check | Result | Notes |
|---|---|---|
| 需求覆盖完整（关键事件 + 错误 + 即时性） | pass | design 第 8 节给出逐阶段事件码表（含 `whisper.start/ok/fail/oom`、`translate.block.*`、`video.*`、`run.*`）；每行含 `run`/`group`/`stage`；追加 + 逐行 flush；L3 增加“仅凭日志定位一次故意失败”的实战验收 |
| 实现方式最小化 | pass | 在 `init_tracing` 现有 fmt/Sentry 两层上再挂一个 `tracing-appender` 文件层；**无新 IPC、无新 capability、无前端读取内容**（避免成为第二个预览实现，与 C4 一致） |
| 不引入第二套日志系统 | pass | 现有内存 `LogStore`/`logging_append` 与 Sentry 行为保留（M7）；文件层是旁路 sink，不参与状态机 |
| 体积与写入风险已控 | pass | 5MB × 5 轮转 + non-blocking writer + `logging.verbose` 默认关 + 写失败只告警一次且不阻断（AC-25/AC-26） |
| 脱敏可验证 | pass | 已定：记录**完整 URL**（含 query，开发者 2026-09-17 决定 A，W008 已关闭），缓解为“仅本机不外传 + 外发前检查”；其余敏感值（key/Cookie/Bearer/请求头）一律不记，AC-23 断言 |
| 新增依赖/配置项合理 | pass | 仅 `tracing-appender`（官方生态、MIT/Apache-2.0）与 `logging.verbose` 一个字段；路径/阈值/份数不做配置 |

## Findings

### Over-design / risk findings

- **Architecture**：`ffmpeg_runner`/`whisper_runner` 若各自实现进程树杀灭与取消，会与 `ytdlp_process.rs` 的 Windows Job Object / Unix `setpgid` 逻辑分叉，形成第二套取消语义（AC-16 的判定会变模糊）。要求：复用同一进程封装模块。
- **Architecture**：`translation_probe`（`GET /models`）与真实翻译请求的鉴权路径重复；401 已能给出确定性错误码，多一个探测命令只是多一处需要维护的错误映射。
- **Architecture**：`transcription_download_model` 需要"用极短静音触发下载"这类技巧，且模型下载进度与 whisper 自己的下载日志耦合；把失败与进度都交给首次真实转录更简单（失败可重试，模型已下载则无副作用）。
- **Data model**：`targetLanguage`、`output.chineseFileName` 等"可配置但只有一个取值"的字段会扩散到设置 UI、i18n、文档与 AC；第一期固定更省。
- **Data model**：`artifacts_read` + 详情页预览会新增一个 IPC 与一套前端渲染（截断、错误、编码），而系统默认程序打开 txt 已能满足"查看"。
- **Verification**：L2 依赖 fake whisper/ffmpeg 与 mock DeepSeek，能覆盖联动但**不能覆盖真实 whisper 行为**（stderr 进度行格式、`--output_format json` 的文件名与结构、GPU OOM 文案）。这些必须进 L3，且 AC-03 的 argv 不能代表真实模型可用。
- **Verification**：AC-17（并发上限）用"时间戳不重叠"判定，需要 fake whisper 保证长跑（>1s）；不要把该断言写成"进程数 ≤1 的瞬时采样"，否则会偶发失败。
- **Compatibility**：`media_size` 命令与体积 store 的移除会触及 `media-queue` 现有前端测试与 mock；移除动作必须与 mock/测试同步删除，避免"未 mock 命令导致 E2E 挂起"的既有坑（`docs/current/shared/testing-strategy.md` 已记录该失效模式）。
- **Cost**：DeepSeek 费用与 1 小时视频的 token 量未在设计里给出量级估算；建议在 Phase A 完成后用 1 个视频实测并把量级写回 design（不改目标，只补数据）。
- **增量需求（日志）**：逐阶段事件码必须是**稳定契约**（AC-22 逐字断言），否则日志会被改坏且回归无感；另需接受一个现实：`tracing` 的既有调用（如 `url={}`）会把完整 URL 写进文件，属可接受但需声明的取舍（W008）。

### Suggested cuts (thinner design)

| # | 裁剪项 | 影响 | 保留代价（不裁的话） |
|---|---|---|---|
| C1 | 去掉静音对齐，v1 固定切分 + `chunkBoundaryRisk` 告警；AC-06 延后 | 少量边界切词 | 多一次 ffmpeg 全量扫描 + 解析 + 单测 |
| C2 | 去掉 `transcription_download_model` 命令与 `/setup` 的下载按钮，改为首次转录自动下载 + 失败提示 | 首次体验略差（无进度条） | 一个命令 + 进度耦合 + i18n |
| C3 | 去掉 `translation_probe`，key 校验由首次真实请求的 401 → `deepseekAuthFailed` 承担 | 设置页少一个"测试连接" | 一个命令 + 错误映射 |
| C4 | 去掉 `artifacts_read` 与详情页文稿预览，改为"打开输出目录/用系统程序打开 txt" | 少一个内嵌预览 | 一个命令 + 详情页渲染 + 截断逻辑 |
| C5 | `translation.targetLanguage` 固定 `zh-Hans`；`output.chineseFileName`/`englishFileName` 固定 | 设置面更小 | 配置字段 + UI + 文档 + AC |
| C6 | 术语表 v1 只用 textarea 的 `原文=译文` 纯文本（存 `String`），不做结构化编辑 | 编辑体验一般 | 结构数组 + 校验 UI + 迁移 |
| C7 | 去掉 `skipped` 独立状态，用 `done` + 汇总标记表达"已存在跳过" | 状态机改动面变小 | 一个状态 + 一个步骤组件 |

建议：**C1–C5 全部采纳**（压缩约 1 个阶段的工作量）；C6 采纳；C7 由开发者决定（影响汇总可读性）。

### Worth keeping (surprising but justified now)

- **分块转录**（含 AC-05/07/08）：这是"长视频可续跑 + 进度可解释 + 避免幻觉循环"的唯一现实手段，不能裁。
- **`TranscribeLimiter(1)`**：本机实测空闲显存仅 1.23GB，GPU 串行是硬约束而非优化。
- **AC-09（原稿零 LLM）**：把"保持原文"变成可判定规则，是本次需求的核心承诺。
- **AC-18（旧配置兼容）**：schema 大改下唯一防止"升级即砖机"的护栏。
- **块级 `zh.blocks.json` + 原子写**：DeepSeek 失败/限流是常态，块级续跑直接决定批量场景可用性。
- **删除下载向代码（G6）**：开发者明确要求覆盖；保留会造成两套参数构造并存与误导性文档。

## Developer Decision

- Decision: approved（方案 A：整体确认 + 接受裁剪 C1–C7）
- Reason / accepted cuts: 开发者于 2026-09-17 选择方案 A。C1（去掉静音对齐，改固定切分 + `chunkBoundaryRisk`）、
  C2（不新增 `transcription_download_model`，首次转录自动下载）、C3（不新增 `translation_probe`，401 → `deepseekAuthFailed`）、
  C4（不新增 `artifacts_read`，改用 opener 打开目录/文件）、C5（`targetLanguage` 与文件名固定）、
  C6（术语表为纯文本 `原文=译文`）、C7（不新增 `skipped` 状态）全部接受，已写回 design.md/tasks.md/verification.md。
- 增量评审追加（2026-09-17，同日）：开发者追加“长链路必须有 `.log` 文件”需求，已按上述 Observability / Logging 维度评审：
  方案为 `tracing-appender` 文件层 + 稳定事件码 + 集中脱敏 + 5MB × 5 轮转 + `logging.verbose`；新增 AC-21–AC-27，均已核验落点。
  唯一待拍板项：W008（日志是否记录完整 URL / 是否 strip 签名 query）—— **已定：记录完整 URL（含 query），决定 A；W008 关闭**。
- Confirmed at: 2026-09-17

## Gate

| Gate | Result | Notes |
|---|---|---|
| DESIGN_REVIEW_PASS | passed | 2026-09-17 开发者确认（方案 A）；AC 校准已完成（`verification.md` 的 `acceptance_review_status: approved`，含 5 条 revised + 7 条日志 AC）。同日追加日志需求的增量评审已完成（Observability / Logging 维度）。编码仍由开发者指令控制：本轮只需文档就绪，等待开工指令后才追加 worklog L001 |
