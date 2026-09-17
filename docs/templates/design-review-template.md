<!--
Pre-implementation Design Review for a change that already has design.md.

This is NOT a loop review (reviews/Lxxx.md). It gates coding and task
materialization. Complexity is one required dimension; add more dimension
sections over time without renaming the gate.

After the gate passes and BEFORE coding, the change's `verification.md`
Acceptance Criteria (AC) must be strictly reviewed against the actual code,
and the mutually agreed result recorded in the verification.md
`Acceptance Criteria Review` section (acceptance_review_status: approved).

Vocabulary:
- Verdict: PASS / PASS_WITH_WARNINGS / NEEDS_CUT / FAIL
- confirmation_status: pending_confirmation / approved / cut_accepted / rejected / overridden
- Milestone Gate: DESIGN_REVIEW_PASS = pending / passed / failed / overridden
  (independent of CODE_PASS / DOCS_PASS / READY_FOR_PR / PR_PASS)

Gate rule:
- PASS → may proceed to implementation after this file is written.
- PASS_WITH_WARNINGS / NEEDS_CUT / FAIL → MUST stop coding, persist this file,
  surface findings to the developer, and wait for confirmation.
  PASS_WITH_WARNINGS also stops by default (not a soft continue).
- Explicit developer override may set confirmation_status=overridden and
  DESIGN_REVIEW_PASS=overridden; record the reason below.
-->

---
status: pending_confirmation
layer: change
review_type: design_review
created_at: <YYYY-MM-DD>
design_path: docs/changes/<feature>/design.md
verdict: PASS | PASS_WITH_WARNINGS | NEEDS_CUT | FAIL
confirmation_status: pending_confirmation | approved | cut_accepted | rejected | overridden
---

# Design Review: <feature-name>

## Summary

| Field | Value |
|---|---|
| Design | `docs/changes/<feature>/design.md` |
| Verdict | PASS / PASS_WITH_WARNINGS / NEEDS_CUT / FAIL |
| confirmation_status | pending_confirmation / approved / cut_accepted / rejected / overridden |
| DESIGN_REVIEW_PASS | pending / passed / failed / overridden |

## Scope Anchor

Re-state the original ask in one short paragraph, then check design Goals /
Non-Goals against it.

- Original ask:
- Design goals in scope:
- Design items outside ask (if any):

## Dimensions

<!--
Active dimensions for this review. Required dimensions must be filled.
Future dimensions may be added as new `###` sections; mark unused optional
ones `n/a` or omit them only when the skill's active dimension list says they
are optional and not applicable.
-->

### Complexity — Architecture

| Check | Result | Notes |
|---|---|---|
| Scope matches Goals/Non-Goals | pass / warning / fail |  |
| No premature new abstraction layer | pass / warning / fail | framework / bus / plugin / shared SDK |
| No speculative generalization | pass / warning / fail | “以后可能” without a second consumer now |
| Cross-cutting surface growth justified | pass / warning / fail | shared contract / enum / cross-domain API |
| Operational weight justified | pass / warning / fail | cache / queue / job / third-party |
| Alternatives Considered includes a simpler option | pass / warning / fail |  |
| Touch/complexity budget reasonable | pass / warning / fail |  |

### Complexity — Data Model

| Check | Result | Notes |
|---|---|---|
| New tables justified vs reuse | pass / warning / fail |  |
| Alterations / migrations scoped | pass / warning / fail | columns, indexes, enums |
| Cross-domain FK / shared tables justified | pass / warning / fail |  |
| Rollback / backfill cost acceptable | pass / warning / fail |  |
| No speculative schema for unused futures | pass / warning / fail |  |

### Verification — Acceptance Criteria

> 触发条件：change 的 `verification.md` 包含 Acceptance Criteria 时必填；否则写 `n/a`。
> 时序：本维度在本门禁中评审 AC 质量；门禁通过后、编码前，再对照实际代码执行 AC 严格评审，协商一致结论写入 `verification.md` 的 `Acceptance Criteria Review` 章节（`acceptance_review_status: approved` 后才可开工）。

| Check | Result | Notes |
|---|---|---|
| AC 存在且编号连续 | pass / warning / fail | AC-xx 覆盖全部关键判断点 |
| AC 代码接地 | pass / warning / fail | 引用的接口路径/类/字段/权限码/错误码真实存在 |
| AC 可判定 | pass / warning / fail | 每条 AC 可证伪，无业务语言模糊项 |
| AC 粒度合适 | pass / warning / fail | 只覆盖关键判断点，不细到伪代码 |
| 与 design.md 目标对应 | pass / warning / fail | 每条业务目标都有 AC 落点 |
| Must-Not-Break 业务语义保留 | pass / warning / fail | 不被 AC 替代 |

### <Future Dimension Name>

<!-- Optional / future. Example placeholders: permission boundary, compatibility,
current-doc conflict. Fill when the skill marks the dimension active; otherwise
write `n/a` or delete this stub when copying the template. -->

| Check | Result | Notes |
|---|---|---|
| n/a | n/a |  |

## Findings

### Over-design / risk findings

- <dimension>: <item> — why it exceeds the ask or current necessity

### Suggested cuts (thinner design)

- drop / defer / replace with <simpler approach>

### Worth keeping (surprising but justified now)

- <item> — why it is necessary in this change

## Developer Decision

<!-- Leave blank when writing the first review. Fill after the developer replies. -->

- Decision: pending / approved / cut_accepted / rejected / overridden
- Reason / accepted cuts:
- Confirmed at: <YYYY-MM-DD> / pending

## Gate

| Gate | Result | Notes |
|---|---|---|
| DESIGN_REVIEW_PASS | pending / passed / failed / overridden | coding blocked until passed or overridden |
