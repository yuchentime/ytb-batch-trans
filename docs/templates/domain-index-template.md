---
status: current
layer: domain
domain: <domain>
canonical_for:
  - <capability>
last_verified: <YYYY-MM-DD>
---

# <Domain> Documentation Route

## Canonical Docs

- `overview.md`: Current capability, boundaries, and status.
- `flow.md`: End-to-end user and system flow.
- `api-contract.md`: Complete API schema contract, including request/response structures, field descriptions, nested objects, and error cases.
- `frontend-behavior.md`: Client behavior, state, UI, and runtime constraints.
- `backend-behavior.md`: Service behavior, permissions, transactions, and side effects.
- `verification.md`: Tests and manual acceptance for this domain.

## Task Routes

| Task | Read |
| --- | --- |
| Implement or change normal flow | `overview.md` -> `flow.md` -> `api-contract.md` -> `frontend-behavior.md` -> `backend-behavior.md` -> `verification.md` |
| Change frontend behavior | `overview.md` -> `flow.md` -> `frontend-behavior.md` -> `api-contract.md` -> `verification.md` |
| Change backend behavior | `overview.md` -> `flow.md` -> `backend-behavior.md` -> `api-contract.md` -> `verification.md` |
| Debug production-like failure | `flow.md` -> `api-contract.md` -> `backend-behavior.md` -> `verification.md` |

## Related Domains

- `../<related-domain>/index.md`

## Historical References

- `../../../decisions/<ADR>.md`
- `../../../postmortems/<YYYY-MM-DD-topic>.md`

## Update Rules

- Update this route when adding, splitting, renaming, or deprecating a domain document.
- Keep required reading short and current-first.
- Move historical rationale to decisions or postmortems.
