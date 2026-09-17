# Worklog: <feature-name>

<!--
Column contract (do not invent alternatives):
- 验证结果: report what was run and the outcome. Distinguish TASK-RELEVANT
  failures from a KNOWN pre-existing broken baseline. Tag an unrelated
  chronically-red check as `[quarantined: <reason + tracking ref>]` so a real
  regression can never hide behind the same excuse. Never let a quarantined
  signal silently absorb a new failure.
- 评审结论: `pending review` / `PASS` / `PASS_WITH_WARNINGS` / `FAIL`.
  Use exactly the same value as the matching review's Result. Do not use ad-hoc
  words like `REJECTED`.
- Commit: a landed loop should carry the real short hash. `not committed` is only
  valid while the loop is still in progress or intentionally uncommitted; a loop
  that claims PASS should be tied to a commit.
- Re-anchor: most rows are `n/a`. The re-anchor CHECK runs before starting each
  loop that follows a multiple-of-3 boundary, and its outcome is recorded on the
  row of the loop it gates: L004, L007, L010, ... (not the preceding L003/L006).
  Value = `on-track` / `scope-drift` / `intent-drift` / `doc-drift` + one line.
- 主要改动: keep loop-sized. When a change needs durable, reviewer-facing detail
  that outlives the loop (mappings, boundary decisions, evidence), put it in
  implementation-notes.md and reference it here instead of pasting a wall of text.
-->

| Loop | 本轮目标 | 主要改动 | 验证结果 | 评审结论 | 下一轮动作 | 风险/待评估点 | Commit | Re-anchor |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| L001 | 这一轮要完成什么 | 改了哪些核心内容（大段细节外溢到 implementation-notes.md） | 跑了什么，结果如何；无关的长期红检查标 `[quarantined: 原因+追踪链接]` | `pending review` / `PASS` / `PASS_WITH_WARNINGS` / `FAIL` | 如果评审有后续动作，这里写下一轮要补什么；否则写 `none` | 给评审者看的重点 | `abc123` / `not committed` | `n/a`（仅 L004/L007/L010/… 必填 `on-track` / `scope-drift` / `intent-drift` / `doc-drift` + 一句话） |
