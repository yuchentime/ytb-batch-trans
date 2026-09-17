---
status: draft
layer: source
canonical_for:
  - batch-transcribe-translate-requirement
related:
  - docs/source/batch-transcribe-translate/raw-requirement.md
last_verified: 2026-09-17
---

# 主题：批量转录 + AI 翻译中文（需求澄清中）

## 状态

**文档已就绪，等待开工指令**：正式变更目录 `docs/changes/2026-09-17-batch-transcribe-translate/` 已建齐
（brief/design/tasks/verification/worklog/implementation-notes/current-doc-updates + reviews），
`DESIGN_REVIEW_PASS = passed`（开发者方案 A，接受裁剪 C1–C7），`acceptance_review_status = approved`。
2026-09-17 追加需求：长链路必须有 `.log` 文件记录关键事件与错误（已同步进 design 第 8 节、AC-21–AC-27 与增量评审）。
开发者要求先完成全部文档、暂不实施代码；未收到开工指令前不得追加 worklog 实现循环。

## 文件

- `raw-requirement.md`：需求原文、本机环境实测事实、硬技术约束、开放问题清单（Q1–Q16）与已确认决策。

## 正式变更（后续唯一实施入口）

- 目录：`docs/changes/2026-09-17-batch-transcribe-translate/`
- 必读：`design.md`（含裁剪后的最终方案）→ `verification.md`（AC-01–AC-20）→ `implementation-notes.md`（真机实测与落点映射）→ `tasks.md`（Phase A/B/C）→ `current-doc-updates.md`（落地后的文档同步清单）

## 来源

用户需求（2026-09-17，本仓库二次开发）：把当前的下载工具改造成“批量转录 YouTube 视频并用 AI 翻译成中文”的工具。
四项关键决策（2026-09-17）：AI 用 **DeepSeek**；译稿**语义不得增删、口误可微调**；**只下音频并转录后删除**；应用**直接改造覆盖**下载用途。

## 下一步

1. 等待开发者下达**开工指令**（本轮已按开发者要求只准备文档）。
2. 开工后：按 `tasks.md` 的 Phase A 开始 L001（配置 schema 增量 + stronghold 新键），逐循环记 `worklog.md`，
   L004/L007/… 前执行 re-anchor。
3. 落地后：按 `current-doc-updates.md` 同步 `docs/current/`、更新 `manifest.yaml`、把 design.md 置 `implemented` + `landed_in`。
