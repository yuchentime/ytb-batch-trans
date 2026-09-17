# Review: <feature-name>

<!--
Vocabulary contract (do not invent alternatives):
- Loop review Result (per review): `PASS` / `PASS_WITH_WARNINGS` / `FAIL`.
  This is the same value written back into the worklog `评审结论` column.
- Milestone Gates (progression): `DESIGN_REVIEW_PASS` / `CODE_PASS` / `DOCS_PASS` /
  `READY_FOR_PR` / `PR_PASS`, each with state `pending` / `passed` / `failed`
  (`DESIGN_REVIEW_PASS` may also be `overridden`). Gates are NOT loop results.
  `DESIGN_REVIEW_PASS` is decided in `reviews/design-review.md` before coding;
  this loop review does not re-open that gate unless the design itself changed.
- Issue IDs are feature-scoped and stable across all reviews of this change.
  Never renumber. `B` = blocker, `W` = warning, `N` = note.
  Reuse the same id across loops; only its `状态` / `Fixed In` / `Verified In` change.
  The running ledger of all issues lives in `reviews/index.md`.
-->

## Review Summary

| Review | Target Loop | Result | Summary | Next Action |
|---|---|---|---|---|
| R001 | L001 | PASS / PASS_WITH_WARNINGS / FAIL |  |  |

---

## Independent Verification (mandatory)

Do not trust the worklog's self-reported verification. The reviewer must
independently execute at least one verification command or inspection and record
it here. If nothing could be re-run, state why and lower `验证可信度`.

| 动作 | 命令/检查 | 结果 | 备注 |
|---|---|---|---|
| <re-run tests> | `<command>` | pass / fail |  |
| <inspect real path> | `<what was inspected>` | ok / mismatch |  |

---

## Acceptance Criteria Coverage

<!--
The review standard is the change's verification.md Acceptance Criteria (AC),
not only the business-level goals in design.md. Judge each AC against the
actual code; every AC must be marked. Any `not_covered` AC must appear in
Issues with a suggested handling.
-->

| AC | 判定 | 证据 / 说明 |
| --- | --- | --- |
| AC-01 <名称> | pass / fail / not_covered | `path:line` 或未覆盖原因 |
| AC-02 <名称> | pass / fail / not_covered |  |

---

## Evaluation

| 维度 | 结果 | 说明 |
|---|---|---|
| 需求一致性 | pass / warning / fail | 逐条对照 verification.md 的 Acceptance Criteria（见 AC 覆盖矩阵），不以 design.md 业务目标作主观判断 |
| 功能正确性 | pass / warning / fail | 正常路径、异常路径、边界条件是否正确 |
| 回归风险 | pass / warning / fail | 是否破坏旧功能、共享模块、配置或依赖 |
| 验证可信度 | pass / warning / fail | build/test/lint 是否被本次评审独立复跑，结果是否可信 |
| 技术债风险 | pass / warning / fail | 是否引入高风险临时方案、重复逻辑、复杂耦合 |
| 文档一致性 | pass / warning / fail | worklog / docs / PR 报告是否和代码一致 |

---

## Issues

<!--
Use feature-scoped, monotonically increasing, stable ids. When an earlier issue
is addressed in a later loop, update its row in place (状态 -> closed, fill
Fixed In / Verified In) rather than opening a new id. Add brand-new issues with
the next unused number. Mirror every row into reviews/index.md.
-->

| ID | 严重级别 | 状态 | 维度 | 问题 | 证据 | 建议 | Fixed In | Verified In |
|---|---|---|---|---|---|---|---|---|
| B001 | BLOCKER | open / closed | 功能正确性 |  | `path:line` |  | `Lxxx` / — | `Rxxx` / — |
| W001 | WARNING | open / closed | 技术债风险 |  | `path:line` |  | `Lxxx` / — | `Rxxx` / — |
| N001 | NOTE | open / closed | 可维护性 |  | `path:line` |  | `Lxxx` / — | `Rxxx` / — |

---

## Final Verdict

| Gate | Result | Notes |
|---|---|---|
| DESIGN_REVIEW_PASS | pending / passed / failed / overridden | pre-implementation; see design-review.md |
| CODE_PASS | pending / passed / failed |  |
| DOCS_PASS | pending / passed / failed |  |
| READY_FOR_PR | pending / passed / failed |  |
| PR_PASS | pending / passed / failed |  |

<!--
DESIGN_REVIEW_PASS is set by reviews/design-review.md before coding.
CODE_PASS cannot pass while any BLOCKER is open.
READY_FOR_PR cannot pass while CODE_PASS or DOCS_PASS is failed.
-->
