---
status: current
layer: change-index
canonical_for:
  - changes-entry
last_verified: 2026-09-17
---

# changes/ 使用说明

`changes/` 保存**重要或复杂改动**的过程记录。它是过程历史，不是当前事实：稳定下来的结论必须同步进 `docs/current/`。

## 何时建 change 目录

满足任一条件：

- 需要多个实现循环（worklog 出现 L002 及以上）；
- 跨领域或影响共享契约（IPC、设置 schema、诊断码、路径/存储）；
- 需要独立评审记录（review）；
- 需要独立分支/PR 或对外可感知的行为变化。

**不需要**建目录：单文件小修、文案改错、依赖升级、纯文档更新。

## 目录结构（模板见 `docs/templates/`）

```text
docs/changes/YYYY-MM-DD-slug/
  brief.md                  # 背景与目标（change-brief-template.md）
  design.md                 # 方案、备选、端到端数据流、前后端抓取逻辑、偏差（change-design-template.md）
  tasks.md                  # 任务拆分（change-tasks-template.md）
  verification.md           # 业务验收 + 代码级 AC + AC 评审记录（verification-template.md）
  worklog.md                # 每个循环一行（change-worklog-template.md）
  implementation-notes.md   # 跨循环仍有价值的细节
  current-doc-updates.md    # 需要同步的 current 文档清单（可选）
  reviews/
    index.md                # issue 记账（review-issue-ledger-template.md）
    design-review.md        # 编码前门禁（design-review-template.md）
    L001.md                 # 逐循环评审（change-review-template.md）
```

## 强制流程

1. **编码前**：写 `reviews/design-review.md`（门禁）。结论 `PASS` 才能开始编码；`PASS_WITH_WARNINGS`/`NEEDS_CUT`/`FAIL` 必须停下等确认。
2. **AC 校准**：门禁通过后、编码前，逐条核对 `verification.md` 的 AC 是否对应真实代码实体，并在其 `Acceptance Criteria Review` 段记录双方一致结论（`acceptance_review_status: approved`）。
3. **每个循环**：`worklog.md` 追加一行；`评审结论` 只能取 `pending review`/`PASS`/`PASS_WITH_WARNINGS`/`FAIL`；
   `Commit` 在 PASS 时必须填真实短哈希；每 3 个循环后的下一行（L004、L007、…）必须填 Re-anchor 结论
   （`on-track`/`scope-drift`/`intent-drift`/`doc-drift`）。
4. **显式评审请求**：写 `reviews/Lxxx.md`（AC 覆盖矩阵 + 独立验证），并在 `reviews/index.md` 记账；
   issue 编号特性级稳定（`B`/`W`/`N` + 递增序号，永不重编号，修复后原地更新状态）。
5. **落地**：`design.md` 置 `status: implemented` + `landed_in`；同步 `current/`；更新领域 `index.md` 与 `manifest.yaml`。
6. **交付后反馈**：触发 `version: 2.0` 版本升级并追加 `Version History`；实现期偏差只写 `Design Deviations`。

## 禁止

- 复用旧 change 目录做新任务（即使主题相似）。
- 把未定型的草案放这里（应放 `docs/source/`）。
- 在 worklog 里用 Gate 词（`CODE_PASS`/`READY_FOR_PR` 等）当评审结论（那是门禁层词汇）。
- 用 `not committed` 占位后直接标记 PASS。

## 现有记录

| Change | 状态 | 说明 |
| --- | --- | --- |
| `2026-09-17-batch-transcribe-translate/` | 文档已就绪，等待开工指令（`DESIGN_REVIEW_PASS: passed`，`acceptance_review_status: approved`） | 把工具改造成“批量转录 + DeepSeek 翻译中文”；已含 brief/design/tasks/verification/worklog/implementation-notes/current-doc-updates + reviews；已接受裁剪 C1–C7，并已纳入追加需求“长链路 `.log` 文件”（AC-21–AC-27） |

其余既有能力（下载/队列/工具链等）上线时未走 change 流程，其事实已直接沉淀到 `docs/current/`。

## Verification

- `python docs/scripts/check_doc_runtime.py docs` 会检查 worklog 列结构、评审结论词汇、Re-anchor 与提交哈希约定。
- `python docs/scripts/reconcile_docs.py docs` 会报告未闭环的 change（`draft` 但已有 PASS 循环等）。
