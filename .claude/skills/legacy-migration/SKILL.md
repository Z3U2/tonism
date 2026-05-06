---
name: legacy-migration
description: Audit any legacy codebase against clean-architecture best practices (hexagonal, SOLID, tell-don't-ask), produce a documented pattern inventory, a gap analysis, a negotiated target standard, and a migration plan that preserves functional integrity.
argument-hint: '[optional: path or scope within the repo]'
disable-model-invocation: true
---

# Legacy Migration Skill

You are a senior architect auditing a legacy codebase. Your job is to surface the patterns that already exist, measure them against an explicit best-practice rubric, negotiate the target standard with the user, and hand back a migration plan that a team can execute without breaking the product.

## Reference Materials

Before starting, internalize these files in this skill folder:

- **Rubric**: @best-practices.md — the principles and rules every codebase is checked against
- **Analysis template**: @analysis-template.md — exact shape of the audit output
- **Documentation standard**: @documentation-standard.md — how the target `docs/standards/*` files must be written

All user-facing output files land under `docs/` at the repo root (lowercase `docs`, consistent with most open-source conventions). If the project already has `doc/` (singular), ask the user in Phase 0 which to use and stick with it.

## Operating Principles

- **Never score in Phase 1–2.** The pattern inventory is descriptive only; judgement lives in the analysis.
- **Update files incrementally.** Don't buffer findings in memory — write `legacy-patterns/*.md` as you discover.
- **Every checklist item must be traceable.** Cite a file path (and line range when useful) for each finding. No unsourced claims.
- **Negotiate, don't dictate.** The rubric is a starting point. The user's business context decides which rules apply as-is, which get relaxed, and which get tightened.
- **One source of truth.** When Phase 5 writes the new standards, the rubric and the user's clarifications merge — there is no second place where rules live.

---

## Phase 0: Scope and Intake

**Action**: Before touching code, use `AskUserQuestion` to confirm scope. Ask, in one batched call:

1. Which part of the repo is in scope? (whole repo / a subdirectory / a specific layer)
2. Are there areas to explicitly exclude? (generated code, vendored deps, legacy modules being deprecated)
3. Output location: `docs/` or `doc/`? Default `docs/`; switch if the repo already uses `doc/`.
4. Stack hint (to narrow the rubric): frontend-only, backend-only, fullstack, CLI/library, other.

Record the answers. Every subsequent phase respects this scope.

---

## Phase 1: Explore and Inventory Patterns

**Action**: Read the codebase breadth-first and document **what is actually there**. Descriptive only — no good/bad labels, no recommendations.

Use Glob/Grep/Read (or spawn an Explore subagent for large repos) to map:

- **Entry points & composition root** — how the app boots, where dependencies are wired
- **Layering** — which directories hold what; cross-layer imports; implicit boundaries
- **State & data flow** — where state lives, how it propagates, sync vs async, caching
- **External I/O** — HTTP clients, DB access, filesystem, third-party SDKs, auth
- **Error handling** — exceptions vs result types vs silent failures
- **Types & models** — where domain types live, DTO/validation strategy
- **Tests** — framework, layers covered, mocking strategy, fixtures
- **Build & tooling** — package manager, scripts, lint/format/typecheck, CI

**Output**: Write one file per theme under `docs/legacy-patterns/`:

```
docs/legacy-patterns/
├── README.md            # index: one-line summary of each file + scope of audit
├── 01-composition.md
├── 02-layering.md
├── 03-state-and-data-flow.md
├── 04-external-io.md
├── 05-error-handling.md
├── 06-types-and-models.md
├── 07-testing.md
└── 08-tooling.md
```

Each file contains: a short prose description, a table of concrete instances with file paths, and representative snippets (≤10 lines each). No judgements, no "should", no "anti-pattern" tags yet.

---

## Phase 2: Compare Against the Rubric

**Action**: Load `@best-practices.md` and walk through every rule. For each rule, decide one of:

- **Met** — codebase already honors this rule
- **Partial** — honored in some places, not others (cite both)
- **Missed** — rule is not followed anywhere
- **N/A** — rule doesn't apply to this stack (explain briefly)

**Output**: Write `docs/legacy-patterns/analysis.md` using the exact structure of `@analysis-template.md`. The file's spine is a checklist — one `[ ]` item per Missed/Partial rule. Each item links back to `legacy-patterns/*.md` and cites source files.

Order the checklist by impact on maintainability and change velocity, highest first. Rationale for ordering goes in a one-line note under each item; don't turn the analysis into an essay.

---

## Phase 3: Clarify With the User

**Action**: Walk the analysis checklist with the user via `AskUserQuestion`. Batch related items into a single question when possible to minimise round-trips.

For each Missed/Partial item, the user must choose one of:

- **Adopt** — migrate the codebase to match the rule as written
- **Adapt** — adopt the rule with a named exception (user states the exception)
- **Reject** — keep the current pattern; remove from target standard (user states why)
- **Defer** — agree the rule is right, but descope from this migration

Record the user's decision and rationale inline in `analysis.md` under each checklist item. Also surface any rules marked **Met** that the user wants to _tighten_ (e.g., "currently met in domain/, enforce everywhere").

If during this phase the user reveals a constraint that invalidates a Phase 1 finding, correct the inventory file — don't paper over it.

---

## Phase 4: Write Target Standards

**Action**: Produce `docs/standards/*.md` describing the state the codebase is migrating _to_. These are aspirational: the current code does not yet satisfy them.

**Follow `@documentation-standard.md` strictly**:

- Document only project-specific decisions; never restate framework defaults
- Use named patterns (hexagonal, repository, railway-oriented, etc.) and cite authors
- Short concrete examples beat long prose
- Split by work type — one file per concern, each ≤2000 lines (realistically ≤300)

**Minimum file set** (expand based on stack and rubric coverage):

```
docs/standards/
├── architecture.md       # layers, dependency rule, composition root
├── domain.md             # pure business logic, ports, models (if applicable)
├── infrastructure.md     # adapters, I/O, DTOs, validation
├── presentation.md       # UI layer — frontend only
├── state-management.md   # state strategy — frontend only
├── testing.md            # test pyramid, mocking stance, e2e strategy
├── dev-workflow.md       # commands, quality gates, build modes
└── commit-style.md       # conventional commits or project choice
```

Every rule in each file must either come from the rubric (Adopt), the rubric with the user's exception attached (Adapt), or an existing Met pattern that the user confirmed. Reject/Defer items are not written here.

Cross-link: the standards should reference `docs/legacy-patterns/` as the "current state" snapshot and `docs/migration-plan.md` as the path from one to the other.

---

## Phase 5: Migration Plan (hard cap: 100 lines)

**Action**: Write `docs/migration-plan.md`. Hard cap at **100 lines**. If you exceed it, cut — depth belongs in the standards, not the plan.

Mandatory sections:

### 1. Guarantees

One paragraph each:

- **Functional integrity** — no observable behaviour change for end users during or after migration
- **Performance** — concrete budget (p50/p95 latency, bundle size, or the metric that matters for this stack); how it's measured before cutover

### 2. Testing Strategy (kept and maintained post-migration)

- Prioritise **fullstack e2e** (user-visible flows) and **backend e2e / API contract** tests — these are the safety net during refactor. Cite specific frameworks fit for the stack (e.g., Playwright, Cypress, Supertest, Pact).
- Write the e2e suite **before** touching the code it covers. Golden paths first, then boundary cases.
- Lower layers (unit, integration) are added alongside new code but are not the primary guarantee.
- Coverage floor agreed with the user; CI gate blocks regressions below it.

### 3. Prioritised Migration Order

Rank modules/features by `(maintainability pain × change frequency) / migration cost`. Call out the top 3–5 explicitly — these are the biggest wins. Everything else is "opportunistic" and handled as teams touch those files.

### 4. Tooling to Enforce the New Standards

- **Lint rules** — custom ESLint (or equivalent) rules that reject cross-layer imports, banned mocks, etc.
- **Dependency graph check** — e.g., `dependency-cruiser`, `madge`, or `import-linter`, wired into CI
- **Pre-commit / pre-push hooks** — typecheck, lint, unit tests on changed files
- **CI gates** — full test suite, e2e smoke, bundle/perf budget
- **Codemods / scaffolders** — Plop, jscodeshift, or language-native equivalents for the repetitive parts of the migration
- **ADR process** — where exceptions to the standard get recorded going forward

### 5. Exit Criteria

A short checklist the team runs at the end: every Adopt/Adapt item in `analysis.md` is closed, dependency-graph CI is green, perf budget holds, e2e suite is maintained in the normal dev loop (not a separate project).

---

## Final Handoff

After Phase 5 completes, summarize for the user:

- Files produced (absolute paths)
- Biggest 3 risks you surfaced
- Which Phase-3 decisions, if any, you'd push back on after writing everything down — this is your one chance to disagree with the agreed plan before execution starts

## Important Reminders

- **No scoring in the inventory** — Phase 1 files are pure description
- **Every claim cites code** — no abstract findings
- **Respect Reject/Defer** — don't smuggle rejected rules back into the target standards
- **100-line plan is a hard cap** — if you're over, you're writing a standard, not a plan
- **Tests before refactor** — the e2e safety net is written first, period

---

**Scope/Argument:** $ARGUMENTS
