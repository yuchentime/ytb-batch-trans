---
status: draft
layer: change
created_at: <YYYY-MM-DD>
---

# Doc Reconciliation: <YYYY-MM-DD>

Periodic pass to re-align `docs/current/` with the real project state. Seed the
signal sections from `scripts/reconcile_docs.py`, then verify each flagged domain
against its backing code. **Destructive actions require human approval**; this
report proposes, the developer disposes.

## Scope

- Domains reviewed this pass: <list>
- Tool run: `python3 skills/agent-runtime-docs/scripts/reconcile_docs.py docs --out <this-file>`

## Signal Summary

| Signal | Count | Confidence |
| --- | --- | --- |
| dangling-reference | <n> | high |
| code-newer-than-doc | <n> | soft |
| unsynced/unclosed change | <n> | high |

## Per-Domain Findings

<!--
One block per flagged domain. Load only that domain's current docs plus the
backing code, then classify each claim. Action ∈ update / supersede / archive /
delete. Anything beyond `update` waits for developer approval.
-->

### <domain>

| 声明/文档 | 证据（代码事实） | 判定 | 建议动作 | 需审批 |
|---|---|---|---|---|
| <current-doc claim> | <code path / grep / git date> | 仍有效 / 过时 / 描述不全 | update / supersede / archive / delete | 是/否 |

## Proposed Actions (awaiting approval)

- [ ] update `docs/current/domains/<domain>/<doc>.md`: <what>
- [ ] supersede `<doc>` (set `superseded_by`) → `<replacement>`
- [ ] archive `<doc>` → `docs/archive/...`
- [ ] delete `<doc>` (needs explicit developer confirmation)

## Close-Out

- [ ] Approved actions applied.
- [ ] `last_verified` refreshed on re-verified current docs.
- [ ] `docs/manifest.yaml` and domain `index.md` updated if paths changed.
- [ ] `scripts/check_doc_runtime.py docs` re-run with no new errors.
- [ ] `docs/change-log.md` timeline updated if anything material changed.
