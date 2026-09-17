# Current Doc Updates: batch-transcribe-translate

实现完成**之后**（Phase C）必须执行的 `docs/current/` 同步清单。在代码落地并通过评审前，不得把这些改动提前写进 `current/`
（否则事实库会记录尚未实现的行为，违反 `docs/current/rules/documentation.rules.md`）。

每项都给出：目标文件、需要的改动、状态。改完后把 `last_verified` 刷成当天，并跑 `check_doc_runtime.py`。

## A. 新建领域文档

| 文件 | 内容要点 | 状态 |
| --- | --- | --- |
| `docs/current/domains/transcribe-translate/index.md` | 领域范围、职责单元（环境门禁 / 音频获取 / 分块转录 / 翻译与保真 / 产物与续跑）、任务路由、相邻领域 | pending |
| `.../flow.md` | 端到端流程（与 design.md 的 Mermaid 一致，去掉裁剪项）、关键节点表、状态机、跳过/续跑判定 | pending |
| `.../api-contract.md` | 新命令/事件字段级契约：`transcription_probe`、`transcribe_start`、`transcribe_stage`、`transcribe_progress`、`translate_progress`、`artifact_written`、`batch_summary`；含 DeepSeek `POST /chat/completions` 的请求/响应字段与错误码；probe 需展开 `logDir`/`logFile`/`logSizeBytes` 字段 | pending |
| `.../data-model.md` | `transcription`/`translation`/`output` 新字段、`.work/` 产物结构（`probe.json`/`chunks.json`/`chunk.json`/`segments.json`/`zh.blocks.json`/`usage.json`）、固定文件名与目录规则 | pending |
| `.../frontend-behavior.md` | 新 store、`MediaState` 新状态、步骤组件、`/setup` 门禁、设置页签、删除清单 | pending |
| `.../backend-behavior.md` | `whisper_runner`/`ffmpeg_runner`/`chunking`/`merge`/`transcript`/`blocks`/`deepseek_client`/`transcribe_pipeline` 职责与约束（含 W001 复用要求） | pending |
| `.../error-handling.md` | design.md 的错误码表（含 `chunkBoundaryRisk`）、状态影响、通知与汇总行为、取消语义 | pending |
| `.../verification.md` | 领域级验证：L1/L2/L3 命令、Must-Not-Break、回归矩阵（从 change 的 verification.md 提升稳定部分） | pending |

## B. 更新既有领域文档

| 文件 | 需要的改动 | 状态 |
| --- | --- | --- |
| `docs/current/domains/media-queue/index.md` / `flow.md` | 去掉 `configure` 步骤与体积查询；入队后自动进入音频下载；播放列表自动展开（不再有选择 UI 与拆分/合并阈值） | pending |
| `.../media-queue/frontend-behavior.md` | 更新状态迁移表与按钮矩阵；删除体积/偏好页/选择步骤相关段落 | pending |
| `.../media-queue/api-contract.md` | 删除 `media_size` 命令与事件；其余保留（`media_info`/`media_playlist_expand`/`media_download`/`group_cancel`） | pending |
| `.../media-queue/data-model.md` | 删除体积相关字段与 `configure` 语义；补充 `DownloadOptions` 收缩为音频的说明 | pending |
| `.../download-engine/index.md` / `flow.md` | 参数链收缩为：progress/network/auth/audio/input → 位置 → URL；删除字幕/SponsorBlock/后处理/格式链接 | pending |
| `.../download-engine/api-contract.md` | 删除「字幕参数」「SponsorBlock」「格式参数」「输出与后处理」大段；保留音频下载与网络/认证；新增「音频下载参数」段（`-f ba/best`、`-o`） | pending |
| `.../download-engine/backend-behavior.md` | 删除字幕/模板相关模块行；补充 `ffmpeg_runner`/`whisper_runner` 复用 `ytdlp_process.rs` 的说明 | pending |
| `.../download-engine/progress-and-diagnostics.md` | 保留诊断码表；备注"下载阶段仅音频"，并新增 whisper/ffmpeg 相关错误码交叉引用 | pending |
| `.../download-engine/verification.md` | 参数矩阵用例范围收缩（删除已移除 flag 的断言） | pending |
| `.../settings-preferences/data-model.md` | 用新 schema 替换 `output`/`subtitles`/`sponsorBlock`/`inputFilters` 段；标注被删字段为历史（可移入 `archive/superseded/`） | pending |
| `.../settings-preferences/api-contract.md` | `config_set` 示例改为新字段；注明旧文件兼容（未知键忽略） | pending |
| `.../settings-preferences/frontend-behavior.md` | 设置页签改为 转录/翻译/输出/网络/系统/关于；删除质量/字幕/过滤页 | pending |
| `.../auth-secrets/*` | 新增 `ai.apiKey` 键（表 + 命令示例）；强调与既有 5 个 auth 键并存、不得重命名 | pending |
| `.../app-lifecycle/flow.md` | 启动流程加入 `/setup` 门禁（环境未通过则输入禁用）；`backend-behavior.md` 补充新命令注册 | pending |
| `.../toolchain/*` | 说明 ffmpeg/ffprobe 同时被转录分块使用（不再是"仅下载后处理"） | pending |
| `docs/current/platform/observability.md` | 新增“文件日志”段：路径与轮转（`<app_dir>/logs/transcribe.log`、5MB × 5）、单行格式与事件码表（与 design 第 8 节一致）、`logging.verbose` 语义、脱敏规则、写失败降级；并更新新错误码的上报策略 | pending |
| `docs/current/platform/build-test-lint.md` | 新增 L2（`TRANSCRIBE_E2E=1`）与 L3 入口说明；fixture 二进制位置 | pending |
| `docs/current/platform/storage.md` | 新增输出目录默认值 `<下载目录>/ovd-transcripts` 与 `.work/` 生命周期 | pending |
| `docs/current/shared/data-ownership.md` | 新增"批次产物/用量/汇总由谁拥有"行；`.work` 与交付 txt 的清理归属 | pending |
| `docs/current/shared/ipc-conventions.md` | 新事件命名登记（`transcribe_*`/`translate_progress`/`artifact_written`/`batch_summary`） | pending |
| `docs/current/shared/naming.md` | 新术语（音频获取、分块、block、保真校验、跳过标记）与禁用别名 | pending |

## C. 产品与规则

| 文件 | 需要的改动 | 状态 |
| --- | --- | --- |
| `docs/current/product/overview.md` | 产品定位改为"批量转录 + 中文翻译"；能力/边界重写；删除下载向特性描述 | pending |
| `docs/current/product/glossary.md` | 新增/替换术语：Transcript、Segments、Block、Chunk、Fidelity Check、Skip Marker；删除清晰度/编码/字幕/SponsorBlock 术语 | pending |
| `docs/current/product/user-journeys.md` | 新增 J9（批量转录：入队→下载音频→转录→翻译→两份 txt→汇总），并删除/改写下载向旅程 | pending |
| `docs/current/product/acceptance.md` | 删除下载向 A10–A15 中不再成立的条目；新增 A26+（原稿零改写、译稿不增删、密钥不外泄、跳过不重复计费、取消即停） | pending |
| `docs/current/rules/architecture.rules.md` | 补充"转录阶段必须串行（GPU）"与"原稿路径不得依赖 LLM"两条约束 | pending |
| `docs/current/rules/forbidden.rules.md` | 新增禁止项：把 LLM 用在原稿路径、在 CI 真实调用 DeepSeek、跳过 `.tmp` 原子写 | pending |
| `docs/current/rules/security.rules.md` | 补充 `ai.apiKey` 的存放/日志/上报要求 | pending |
| `docs/current/rules/documentation.rules.md` | 无需改动（已含 reviews 命名适配说明） | n/a |

## D. 路由、检查清单与归档

| 文件 | 需要的改动 | 状态 |
| --- | --- | --- |
| `docs/index.md` | 领域表新增 `transcribe-translate`，并在"平台与共享契约"补充新文档入口 | pending |
| `docs/manifest.yaml` | 新路由 `transcribe-translate`（required/optional/token_budget/code_globs）；更新 `media-queue`/`download-engine` 的 required 与 `code_globs`（删除已移除的模块路径，新增 `transcribe`/`translation` 路径）；`onboarding` 路由加入新领域 overview | pending |
| `docs/evals/regression-matrix.md` | 新增转录/翻译相关行（分块、跳过、限流重试、取消、密钥、旧配置） | pending |
| `docs/evals/feature-checklist.md` | 新增"AI 输出契约校验是否覆盖""密钥是否检查"两项 | pending |
| `docs/evals/release-checklist.md` | 新增"模型缓存与首次下载"与"DeepSeek key 不随包分发"检查 | pending |
| `docs/archive/superseded/` | 若 `download-engine/api-contract.md` 的删除量过大，把旧版本整体归档并标 `superseded_by` 指向新文档 | pending |
| `docs/source/batch-transcribe-translate/` | 实现落地后把主题状态改为"已进入 changes 并落地"，链接到 change 目录与 `landed_in` | pending |

## 收尾检查

```bash
python docs/scripts/check_doc_runtime.py docs      # 0 error；WARN 需逐条判断
python docs/scripts/reconcile_docs.py docs        # 悬空引用应为 0（删除模块后必须清理文档引用）
```

- [ ] 所有 `last_verified` 更新为落地当天
- [ ] `docs/manifest.yaml` 的 `code_globs` 不再引用已删除文件
- [ ] 被取代的文档已归档并标 `superseded_by`
- [ ] `design.md` 置 `status: implemented` + `landed_in`

## 2026-09-18 同步进度（本次提交）

已完成：

- **A 全部**：新建 `docs/current/domains/transcribe-translate/`（index / flow / api-contract / data-model / backend-behavior / frontend-behavior / error-handling / verification）。
- B：`media-queue/index.md` 与 `download-engine/index.md` 加「2026-09-18 状态」说明（下载 UI 已移除、下载向描述转为历史）；`settings-preferences/data-model.md` 标注当前 Config 形态与历史字段；`auth-secrets/api-contract.md` 新增 `ai.apiKey` 行；`platform/observability.md` 新增「文件日志」段；`platform/storage.md` 新增「转录输出目录」段。
- C：`product/overview.md` 改写定位与用户任务/边界。
- D：`docs/index.md` 领域表与定位；`docs/manifest.yaml` 新增 `transcribe-translate` 路由（required 8 篇 + observability，含 code_globs）。

仍未完成（下一批）：B 的 download-engine 各细分文档（api-contract/backend-behavior/progress-and-diagnostics/verification）与 media-queue 细分文档的收缩、toolchain / app-lifecycle / shared（data-ownership、ipc-conventions、naming）、build-test-lint 的 L2/L3 入口；C 的 glossary / user-journeys(J9) / acceptance(A26+) / rules（architecture、forbidden、security）；D 的 evals 三份清单、archive 决策、`docs/source/batch-transcribe-translate/` 状态、`design.md` 置 `implemented` + `landed_in`。

`reconcile_docs.py` 仍会报 media-queue/download-engine/settings-preferences/auth-secrets/app-lifecycle/frontend-runtime/build-test-lint/debugging 的 `code-newer-than-doc` —— 因为上面未完成的细分文档 `last_verified` 仍是 2026-09-17；完成后再逐领域复核并刷新。
