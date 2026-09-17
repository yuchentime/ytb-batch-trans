# Review Ledger: <feature-name>

Running, cross-loop ledger for every review issue raised on this change. Each
per-loop review (`reviews/Lxxx.md`) records the detail; this file is the single
place to answer "are all blockers closed?" without reading every review file.
Pre-implementation Design Review lives in `reviews/design-review.md` and is
listed in the Review Log with Target Loop = `design` (not an Lxxx).

Issue ids are feature-scoped and stable. Never renumber. Update a row in place
when its status changes.

## Review Log

| Review | Target Loop | Result | Blockers Open After | Summary |
|---|---|---|---|---|
| design-review | design | PASS / PASS_WITH_WARNINGS / NEEDS_CUT / FAIL | n/a | DESIGN_REVIEW_PASS: pending / passed / failed / overridden |
| R001 | L001 | PASS / PASS_WITH_WARNINGS / FAIL | <count> |  |

## Issue Ledger

| ID | 严重级别 | 状态 | 首次提出 | 维度 | 一句话问题 | Fixed In | Verified In |
|---|---|---|---|---|---|---|---|
| B001 | BLOCKER | open / closed | R001 | 功能正确性 |  | `Lxxx` / — | `Rxxx` / — |
| W001 | WARNING | open / closed | R001 | 技术债风险 |  | `Lxxx` / — | `Rxxx` / — |

## Close-Out

- [ ] `DESIGN_REVIEW_PASS` is `passed` or `overridden` (see `reviews/design-review.md`).
- [ ] All BLOCKER issues are `closed` with a filled `Verified In`.
- [ ] Remaining WARNING/NOTE issues are either closed or explicitly accepted with a reason.
- [ ] The latest review Result and worklog `评审结论` for the final loop agree.
