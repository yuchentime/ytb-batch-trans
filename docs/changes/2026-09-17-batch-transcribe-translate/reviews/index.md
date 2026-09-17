# Review Ledger: batch-transcribe-translate

Running, cross-loop ledger for every review issue raised on this change. Each
per-loop review (`reviews/Lxxx.md`) records the detail; this file answers
"are all blockers closed?" without reading every review file.
Pre-implementation Design Review lives in `reviews/design-review.md` and is listed
with Target Loop = `design`.

Issue ids are feature-scoped and stable. Never renumber. Update a row in place when
its status changes.

## Review Log

| Review | Target Loop | Result | Blockers Open After | Summary |
|---|---|---|---|---|
| design-review | design | PASS_WITH_WARNINGS | n/a | DESIGN_REVIEW_PASS: **passed**（2026-09-17 开发者方案 A，接受裁剪 C1–C7）；AC 校准 `approved`（5 条 revised：AC-02/03/06/14/16） |
| design-review（增量：长链路日志） | design | PASS_WITH_WARNINGS | n/a | 2026-09-17 开发者追加“必须有 `.log` 文件”；方案（`tracing-appender` 文件层 + 事件码 + 脱敏 + 5MB×5 轮转）通过，新增 AC-21–AC-27；W008 已关闭（决定 A：记完整 URL），W007 已关闭 |
| acceptance-review | design | PASS | n/a | 逐条对照真实代码核验 20+7 条 AC；发现并修正 1 处设计假设错误（whisper 段行在 stdout，见 W006） |

## Issue Ledger

| ID | 严重级别 | 状态 | 首次提出 | 维度 | 一句话问题 | Fixed In | Verified In |
|---|---|---|---|---|---|---|---|
| W001 | WARNING | open | design-review | Complexity — Architecture | `ffmpeg_runner`/`whisper_runner` 若不复用 `ytdlp_process.rs` 会形成第二套取消语义 | — | — |
| W002 | WARNING | closed | design-review | Complexity — Architecture | `translation_probe` / `transcription_download_model` / `targetLanguage` 属过度设计 | L001 前文档裁剪 | 裁剪 C2/C3/C5 已写回 design.md（2026-09-17） |
| W003 | WARNING | closed | design-review | Complexity — Architecture | 单次 change 体量偏大（3 阶段 21 任务 + 跨 6 领域文档） | L001 前文档裁剪 | 裁剪 C1–C6 已压缩第一期范围（2026-09-17） |
| W004 | WARNING | closed | design-review | Verification | L2 用 fake 二进制覆盖不了真实 whisper stderr/GPU OOM 契约 | — | 接受为 L3 义务，已写入 `verification.md` 的 Required Manual Checks（2026-09-17） |
| W005 | WARNING | open | design-review | Compatibility & Docs | 删除下载向能力会让 `download-engine/api-contract.md` 等大面积失效，文档工作量≈实现量 | — | — |
| W006 | WARNING | closed | acceptance-review | 事实正确性 | 设计原写“解析 stderr 的 segment 行”，真机实测段行在 **stdout**、tqdm 在 stderr、json 顶层为 `text/segments/language` | L001 前文档修正 | 2026-09-17 真机探针（tiny.en，CPU+GPU 两次运行）验证并回写 AC-03/design.md |
| N001 | NOTE | open | design-review | Verification | DeepSeek 成本/长视频 token 量级未估算，建议 Phase A 后用 1 个视频实测并回写 design | — | — |
| W007 | WARNING | closed | design-review（增量） | 依赖与配置面 | 日志需求新增 `tracing-appender` 依赖与 `logging.verbose` 配置项 | L001 前文档 | 接受：官方 tracing 生态依赖（MIT/Apache-2.0）+ 单一布尔字段（2026-09-17） |
| W008 | WARNING | closed | design-review（增量） | 脱敏与隐私 | 沿用现网行为记录**完整 URL**，部分站点 URL 带签名 query，会落入本地日志文件 | — | 2026-09-17 开发者决定 A：保留完整 URL（含 query）；缓解=仅本机/不外传/外发前检查提示 |
| N002 | NOTE | open | design-review（增量） | 可运维性 | 单文件 5MB × 5 的真实写入频率/体积需实测（含 `verbose=true` 时的增长速率） | — | — |

## Close-Out

- [x] `DESIGN_REVIEW_PASS` is `passed` or `overridden` (see `reviews/design-review.md`)
- [x] All BLOCKER issues are `closed` with a filled `Verified In`（当前无 BLOCKER）
- [ ] Remaining WARNING/NOTE issues are either closed or explicitly accepted with a reason（W001/W005/N001 仍开放，见下）
- [ ] The latest review Result and worklog `评审结论` for the final loop agree（L001–L003 均已落地且为 `pending review`；计划补 `reviews/L003.md` 并追溯 L001/L002）

### 仍开放项的处理约定

| ID | 处理计划 |
|---|---|
| W001 | Phase A 的 L004/L005 实现 `whisper_runner`/`ffmpeg_runner` 时强制复用 `ytdlp_process.rs`；代码走查确认后由对应 `reviews/Lxxx.md` 关闭 |
| W005 | Phase C 的文档同步循环处理；完成 `docs/current/domains/transcribe-translate/*` 与新路由后关闭 |
| W008 | ✅ 已关闭（2026-09-17 决定 A：日志记录完整 URL，含 query；缓解：仅本机、不外传、UI/文档提示外发前检查） |
| N001 / N002 | Phase A 首个视频实测后，把 token 量级与日志体积/增长速率写回 `design.md` 的 Risks 段与 `implementation-notes.md` |
