# Best Practices Rubric

This file is the **checklist** used in Phase 2 of the skill. Every rule has an ID (used in `analysis.md`), a short statement, and a one-line "why". Cite the author/work once; do not re-explain concepts the reader already knows.

Not every rule applies to every stack. In Phase 2 mark rules as **Met**, **Partial**, **Missed**, or **N/A** — with a one-line reason for N/A.

---

## A. Architecture & Boundaries

| ID  | Rule                                                                                                                                                                                      | Why it matters                                                                                   |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| A1  | **Hexagonal architecture** (Alistair Cockburn). Domain at the centre; adapters on the outside. Business logic has zero imports from UI frameworks, HTTP clients, ORMs, or the filesystem. | Lets the business logic be tested and evolve independently of volatile I/O choices.              |
| A2  | **Functional core / imperative shell** (Gary Bernhardt). Side effects are pushed to a thin shell; the core is pure.                                                                       | Pure code is trivially testable; shells are trivially swappable.                                 |
| A3  | **Dependency rule**: dependencies point inward. Presentation and Infrastructure depend on Domain; Domain depends on neither.                                                              | Prevents the domain from being dragged along every time an external vendor changes.              |
| A4  | **Ports as abstract contracts**. External dependencies are accessed only through explicit interfaces (abstract class / interface / protocol) declared in the domain.                      | Inversion of control; enables fakes in tests and alternate adapters in production.               |
| A5  | **One composition root**. DI wiring happens in exactly one place at app start; the rest of the code never touches the container.                                                          | Service location sprinkled through the code kills testability and obscures the dependency graph. |
| A6  | **No layer-skipping**. Presentation does not call Infrastructure directly; Infrastructure does not import Presentation.                                                                   | Layer skips metastasise — one skip legitimises the next.                                         |

## B. SOLID

| ID  | Rule                                                                                                                                           | Why it matters                                                                     |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| B1  | **SRP** — each module has one reason to change. Classes/functions do one thing cohesive to a single actor.                                     | Fewer diff collisions; simpler mental model per file.                              |
| B2  | **OCP** — extend via new implementations, not by editing existing stable code.                                                                 | Stable abstractions become the leverage point; churn concentrates in adapters.     |
| B3  | **LSP** — subtypes/implementations honour the contract of the port. No surprise throws, no weakened postconditions.                            | Substitution breaks silently otherwise; tests become coupled to concrete types.    |
| B4  | **ISP** — ports are narrow. A consumer that needs `read` does not depend on `write`.                                                           | Fat interfaces force fake implementations to grow; tests couple to unused methods. |
| B5  | **DIP** — high-level modules don't depend on low-level modules; both depend on abstractions. The abstraction lives with the high-level module. | The port belongs to the consumer, not the provider.                                |

## C. Object / Function Design

| ID  | Rule                                                                                                                          | Why it matters                                                              |
| --- | ----------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| C1  | **Tell, Don't Ask** (Pragmatic Programmer). Call methods on an object to act; don't pull its state out and decide externally. | Logic stays with the data it governs; reduces shotgun surgery.              |
| C2  | **Law of Demeter** — talk only to your immediate collaborators. No `a.b.c.d.method()` chains across layer boundaries.         | Chains propagate coupling; one refactor cascades everywhere.                |
| C3  | **Immutability by default**. Prefer value objects and pure functions; mutate only where performance demands.                  | Eliminates a whole class of concurrency and aliasing bugs.                  |
| C4  | **Composition over inheritance** for behaviour reuse.                                                                         | Inheritance hierarchies ossify; composition stays pliable.                  |
| C5  | **Value Objects** for domain primitives (Money, Email, UserId). No stringly-typed domain IDs.                                 | Type system becomes the first line of validation.                           |
| C6  | **Fail fast at boundaries**. Validate external input once at the shell; trust it inside the core.                             | Nested defensive checks muddy the core and still miss the unknown-unknowns. |

## D. Error Handling

| ID  | Rule                                                                                                                                                                     | Why it matters                                                     |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------ |
| D1  | **Explicit failure types**. Return `Result<T, E>` / `Either` / tagged unions for expected failures (railway-oriented, Scott Wlaschin). Exceptions for truly exceptional. | Makes the failure surface part of the type signature.              |
| D2  | **Error taxonomy** — domain-level error types distinct from infrastructure errors.                                                                                       | Callers can react differently to "not found" vs "network timeout". |
| D3  | **No silent catches** (`catch {}` with no handling). Every catch either recovers, translates, or re-throws with context.                                                 | Silent catches are where production incidents hide for years.      |
| D4  | **Errors carry context** — original cause, correlation ID, enough to debug without reproducing.                                                                          | Minutes to root-cause beats hours of log spelunking.               |

## E. Data, Types, Contracts

| ID  | Rule                                                                                                               | Why it matters                                                    |
| --- | ------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------- |
| E1  | **DTOs at the edge**, domain models inside. Translate in the adapter; never leak raw HTTP/DB shapes into the core. | Domain doesn't shift every time the API version bumps.            |
| E2  | **Runtime validation of external data** (Zod, io-ts, Pydantic, JSON schema). Parse, don't validate.                | The type system lies about what came off the wire; schemas don't. |
| E3  | **No `any` / untyped escape hatches** in domain code; isolate to adapter boundaries if unavoidable.                | `any` is a type-system opt-out that compounds.                    |
| E4  | **Discriminated unions for state**, not boolean flag soup (`isLoading && !isError && data`).                       | Makes illegal states unrepresentable.                             |

## F. State & Concurrency

| ID  | Rule                                                                                                              | Why it matters                            |
| --- | ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| F1  | **Single source of truth per piece of state**. No parallel copies that can drift.                                 | Drift bugs are the hardest to reproduce.  |
| F2  | **Derived state is computed, not stored**. Caches are explicit and invalidation-aware.                            | Stored derivations go stale silently.     |
| F3  | **Side effects in named, testable units** (stores, use cases, sagas) — not scattered in components / controllers. | Keeps the side-effect surface reviewable. |
| F4  | **Observable / event contracts are read-only outward**. Consumers subscribe; only the owner emits.                | Shared-write channels become untrackable. |

## G. Testing

| ID  | Rule                                                                                                                                                | Why it matters                                                                          |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| G1  | **Test pyramid** skewed to the layers that give real confidence: fullstack e2e + backend e2e as the foundation, unit tests for domain, thin middle. | Unit-heavy pyramids pass while the product breaks. See _Testing Trophy_, Kent C. Dodds. |
| G2  | **No module mocks** (`jest.mock`, `vi.mock`, monkey-patching) where DI would do. Register fakes via the container.                                  | Module mocks bypass the code under test and rot when imports move.                      |
| G3  | **Test the behaviour, not the implementation**. Assert on observable outcomes; avoid spies on internal methods.                                     | Behaviour tests survive refactors; implementation tests block them.                     |
| G4  | **Real dependencies over deep mocks at boundaries**. MSW for HTTP, test containers for DBs, real queues in integration tests.                       | Mocks lie about the shape of the wire; integration catches what unit cannot.            |
| G5  | **Tests are co-located** with the code they exercise; no parallel test tree.                                                                        | Moving code moves its tests automatically.                                              |
| G6  | **Flakes are bugs**. Quarantine, then fix — never retry-in-CI as policy.                                                                            | Retries rot the signal value of the suite.                                              |
| G7  | **Boundary-value analysis** (Myers) for domain invariants: edges, zero, off-by-one, max.                                                            | Most defects cluster at boundaries.                                                     |

## H. Documentation & Comments

| ID  | Rule                                                                                                 | Why it matters                                                       |
| --- | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| H1  | **Comment the _why_, not the _what_** (Ousterhout, _A Philosophy of Software Design_).               | Names describe what; comments earn their keep explaining motivation. |
| H2  | **Docs co-located by concern**, not one monolith. Short, navigable, ≤300 lines per file in practice. | Monolithic docs are read once, then never.                           |
| H3  | **Document project-specific decisions only**. Do not restate framework documentation.                | Reduces noise; AI agents and new devs already know the framework.    |
| H4  | **ADRs for non-trivial decisions** — context, options, decision, consequences. MADR-style template.  | Future maintainers know why the choice was made, not just what.      |

## I. Tooling & Process

| ID  | Rule                                                                                                                                                   | Why it matters                                              |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------- |
| I1  | **Conventional Commits** or project-defined equivalent, enforced by commit-msg hook.                                                                   | Drives changelog automation and reviewer scan speed.        |
| I2  | **Pre-commit** runs fast, local checks (format, lint-staged, typecheck-changed). **Pre-push** runs unit + build. **CI** runs everything including e2e. | Feedback loop latency matched to confidence required.       |
| I3  | **Dependency-graph enforcement** (dependency-cruiser / madge / import-linter / ArchUnit). Cross-layer imports fail the build.                          | Architecture erodes silently without machine-checked rules. |
| I4  | **Reproducible builds** — lockfile committed, Node/runtime version pinned, CI matches local.                                                           | "Works on my machine" is a process smell.                   |
| I5  | **Single command to start** (`make dev` / `pnpm dev`), **single command to test**, **single command to ship**.                                         | Onboarding friction compounds; every extra step is a tax.   |

## J. Performance & Operability

| ID  | Rule                                                                                           | Why it matters                                     |
| --- | ---------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| J1  | **Performance budgets** are explicit and measured (p50/p95 latency, bundle size, LCP, memory). | Without budgets, regressions accumulate unnoticed. |
| J2  | **Structured logging with correlation IDs** at I/O boundaries.                                 | Half of production debugging is log correlation.   |
| J3  | **Feature flags for risky rollouts**; flags have expiry dates.                                 | Flags without expiry become permanent tech debt.   |
| J4  | **Observability before optimisation** — measure, then cut.                                     | Guessed optimisations are often regressions.       |

---

## How to Use This Rubric

1. In Phase 2, walk every ID in order.
2. For each, decide **Met / Partial / Missed / N/A** and cite concrete file paths.
3. Write one checklist line per Missed/Partial in `analysis.md` using ID+short title — e.g. `- [ ] A3 — Domain imports Next.js router (src/domain/todos/...)`.
4. In Phase 3, every checklist item gets an Adopt/Adapt/Reject/Defer decision; nothing is silently dropped.
