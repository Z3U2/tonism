# Codebase Analysis — `{{project name}}`

> Template for `docs/legacy-patterns/analysis.md`. Replace every `{{placeholder}}`. Keep the structure; do not add preamble or reorder sections.

---

## Scope

- **Audited paths**: {{paths included}}
- **Excluded paths**: {{paths excluded}}
- **Stack**: {{frontend / backend / fullstack / lib}} — {{primary framework(s)}}
- **Date**: {{YYYY-MM-DD}}
- **Commit SHA**: {{sha at time of audit}}
- **Rubric version**: see `.claude/skills/legacy-migration/best-practices.md` (sections A–J)

---

## Summary

A 5–8 line executive summary. No bullet lists here.

- What the codebase does well (1–2 sentences).
- Where the largest gaps sit relative to the rubric (1–2 sentences).
- The top 3 items that — if fixed — unlock the most change velocity (1–2 sentences).

---

## Rubric Coverage

One row per rubric ID from `best-practices.md`. Status is one of **Met / Partial / Missed / N/A**. `Evidence` cites at least one file path; prefer a range `src/foo.ts:42-78`.

| ID  | Rule (short)            | Status  | Evidence                                   |
| --- | ----------------------- | ------- | ------------------------------------------ |
| A1  | Hexagonal architecture  | Partial | `src/domain/x.ts:12` imports `next/router` |
| A2  | Functional core / shell | {{...}} | {{...}}                                    |
| ... | ...                     | ...     | ...                                        |

---

## Checklist — Items to Address

Ordered by `(maintainability pain × change frequency) / migration cost`, highest first. One-line rationale under each item.

> Each item is resolved in Phase 3 with one of: **Adopt / Adapt / Reject / Defer**. Fill the `Decision` block inline during Phase 3; do not leave it for later.

- [ ] **A3 — Domain imports infrastructure concerns**
  - _Evidence_: `src/domain/todos/todos.list.store.ts:3` imports `axios`; `src/domain/auth/auth.store.ts:7` imports `next/headers`.
  - _Why it's high priority_: every domain test now boots a mock HTTP layer; change velocity on the domain is bottlenecked by infra changes.
  - _Decision_: {{Adopt / Adapt / Reject / Defer}} — {{user rationale, filled in Phase 3}}

- [ ] **B4 — Fat ports**
  - _Evidence_: `UserPort` exposes 14 methods in `src/domain/.../user.ports.ts`; only 3 are used by >1 consumer.
  - _Why it's high priority_: fakes in tests grow to match; onboarding cost scales with port surface.
  - _Decision_: {{...}}

- [ ] **G2 — Module-level mocks**
  - _Evidence_: 47 `jest.mock(...)` calls across `src/**/*.spec.ts`; none register via DI.
  - _Why it's high priority_: blocks refactors — moving a file breaks tests that never touched the behaviour.
  - _Decision_: {{...}}

{{... one entry per Missed/Partial rule ...}}

---

## Met Rules Worth Tightening

Rules currently **Met** in part of the codebase that the team may want to enforce globally.

- [ ] **{{ID}} — {{rule}}**: currently honoured in `{{path}}`, not enforced in `{{path}}`. Proposed action: `{{machine-checked rule via dependency-cruiser / eslint / etc.}}`.

---

## N/A Rules

Rules that don't apply to this stack, with a one-line reason each. Keep this short — if the list is long, you are probably mis-classifying.

- **{{ID}}**: {{why N/A — e.g. "no persistent state, CLI executes once per invocation"}}

---

## Risks and Open Questions

Anything the audit surfaced that isn't a rubric item but will bite the migration:

- {{risk 1 — e.g. "auth module is being rewritten in parallel; freeze coordination needed"}}
- {{risk 2}}

---

## Next Step

Phase 3 clarification with the user, starting with the top {{N}} checklist items. Unresolved `{{Decision}}` blocks block progression to Phase 4.
