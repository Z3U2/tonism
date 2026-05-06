---
name: technical-refinement
description: Produce an implementation plan (Technical Refinement) for every user story of a given spec, grounded in the architecture standards, the spec, and a parallel exploration of the codebase. Trigger when the user asks for a "TR", "technical refinement", "implementation plan", "refine the stories technically", or names a `docs/specs/` slug with the intent of preparing dev work — even implicitly.
argument-hint: '[spec-slug — folder name under docs/specs/]'
---

# Technical Refinement

You turn the user stories of a spec into **implementation plans** ready to code. Each plan targets one story, covers every layer involved (Domain, Audio adapter, GUI adapter, Persistence, Tests), and explicitly marks each component as **🟢 new**, **🟡 modified**, or **⚪ reused** versus the current codebase.

An implementation plan is a technical contract: a developer must be able to execute it without re-reading the spec, and a reviewer must be able to audit it without re-running an exploration. Plan with the rigor of Eisenhower preparing D-Day — every detail matters, every edge case is traced. Incomplete plans become production bugs (and in a realtime audio product, those bugs are audible).

## Input

- **Argument**: a `<spec-slug>` (the folder name under `docs/specs/`, e.g. `mvp`). Each slug folder contains `spec.md`, optional `stories.md`, optional `dependencies.md`, and a `stories/` subfolder.
- If no argument is given, list folders under `docs/specs/`, ask the user which one to refine — never guess.

## Output

For **each** user story under `docs/specs/<spec-slug>/stories/`, produce one file:

`docs/specs/<spec-slug>/stories/<story-filename-without-.md>-implementation-plan.md`

Example: `mvp-02-buffer-underrun-counter.md` → `mvp-02-buffer-underrun-counter-implementation-plan.md`

**Plans are written in English** to match the rest of `docs/`.

---

## Required reading

Before writing anything, read in this order:

1. **All standards** — discover them dynamically (the set will evolve):

   ```bash
   find docs/standards -type f -name '*.md'
   ```

   Read every file returned. These are the **architectural source of truth**. Any plan that contradicts them must say so explicitly (rare, with rationale).

2. **The product architecture** — `docs/specs/product-architecture.md` for product layers, definition of success, non-goals.

3. **All ADRs** — `docs/adr/*.md`. ADRs commit decisions that the standards summarise; reading them prevents re-litigating settled questions.

4. **The spec**: `docs/specs/<spec-slug>/spec.md` — context, need, recommendations, open questions, out of scope.

5. **The dependencies file** (if present): `docs/specs/<spec-slug>/dependencies.md` — Tech Lead choices the plan must build on.

6. **The stories index** (if present): `docs/specs/<spec-slug>/stories.md` — overview, dependencies, and **Notes for the Technical Refinement** (especially 🔴 blocking questions).

7. **All stories**: every file under `docs/specs/<spec-slug>/stories/`. Read them in full — a plan based on a partial read of the story is shaky.

8. **The plan template**: [implementation-plan-template.md](implementation-plan-template.md) (in this folder).

9. **Any project-level `CLAUDE.md`** (if it exists at the repo root) — for conventions and commands.

Do not move on to exploration until all of the above is read.

---

## Phase 1 — Parallel codebase exploration

Like `write-user-stories`, but focused on **what to build/modify** rather than **how to split**. In a **single message**, spawn four `Explore` subagents in parallel. Each brief contains:

- The 2-3 sentence spec context
- The layer to explore
- The list of stories (ID + short title) — so the agent zooms into the right areas
- The question: _"For each story, which components already exist (to reuse or modify), and which new components must be created? Cite file paths and lines."_
- A request for a report structured by story, under 300 words

| Subagent | Layer focus | Looks for |
| -------- | ----------- | --------- |
| A        | **Audio path (Capture + Render)** | Audio backend setup (cpal or chosen alternative), buffer routing, sample-rate/buffer-size negotiation, xrun detection, latency measurement, realtime-thread entry point. Verify A2 (no alloc / no lock / no syscall on the per-buffer path) is preserved. |
| B        | **Domain (Signal chain + Tone state)** | DSP blocks, signal-chain types, parameter model, NewType wrappers (`SampleRate`, `BufferSize`, `Decibels`, …), error taxonomy, ports/traits. Pure code — no I/O imports allowed (rule A1). |
| C        | **Control surface (GUI + future MIDI)** | GUI framework setup, widget code, GUI ↔ audio messaging primitive, parameter update flow. Identify what already exists vs needs creating per story. |
| D        | **Persistence + Tests** | Preset I/O, settings store, test harnesses, audio fixtures, integration/e2e setup. Patterns to follow for new tests (Testing Trophy: e2e-thin, integration-load-bearing, unit-pure). |

Adapt the scope if the spec is purely DSP (skip C) or purely GUI (skip A). By default, run all four. Better a "nothing to do here" than a blind spot.

**Important**: these 4 reports are **shared across all stories of the spec**. Do not re-run the exploration for every plan.

> **Greenfield caveat.** If the Rust source tree does not yet exist or is very thin (most decisions per `docs/standards/` are still TBD at the time of writing), explorations will return mostly "missing". Plans then become near-fully 🟢. That is fine — **but** the plans must still respect the standards (hexagonal layering, no I/O in domain, lock-free GUI ↔ audio messaging, Testing Trophy) and explicitly cite the ADRs/standards that constrain each new component.

---

## Phase 2 — Writing the plans (one story at a time)

For each story (in dependency order — follow the index graph if present), write the plan strictly following [implementation-plan-template.md](implementation-plan-template.md). Before each plan:

1. Re-read the story (title, description, ACs, checklist).
2. Cross-check every AC and every checklist item against the 4 exploration reports.
3. For every component cited in the plan, mark its status honestly (see rule below).

If the story is flagged `⚠️ TR required` in the index, open the plan with a `## ⚠️ Decisions to validate before starting` section listing the open questions and their concrete impact on the plan. Do not guess the answer — list the options.

### If a story is complex (≥3 layers touched, ≥10 new components, or flagged TR required)

Write the plan **section by section** and wait for user feedback before moving to the next. Sections affected: `2. Functional scope`, `3. Domain & data model`, `4. Architecture`, `5. Tests`. For simple stories, write the plan in one pass.

### Before moving to the next story

- [ ] Every AC and checklist item in the story is traced to a component or test in the plan.
- [ ] No ⚪ "reused" component without a verified file path (Read before marking).
- [ ] No decision missing from the spec/standards is made without being flagged as an open question.
- [ ] Build / test / lint commands are mentioned explicitly when they apply (`cargo build`, `cargo test`, `cargo clippy`, `cargo fmt --check`).
- [ ] Realtime-audio constraints (rule A2) are restated for any plan touching the audio callback.

---

## Hard rules

### Rule 1 — Code coherence (`code-coherence`)

**Search the codebase before proposing a new component.** Mark every component cited in the plan with exactly one status:

- 🟢 **New** — to create (no equivalent file found)
- 🟡 **Existing, to modify** — file exists, behaviour to extend/change (state what)
- ⚪ **Reused as-is** — existing file already meeting the need (file path mandatory)

**CRITICAL**: before marking ⚪, open the file with Read and compare to the story's need. If the existing component does not exactly cover the need, it is 🟡, not ⚪.

### Rule 2 — Complete mapping (`complete-mapping`)

Every element of the story must appear in the plan:

- Each **acceptance scenario** (success or failure) → one test (e2e, integration, or unit) explicitly covering it.
- Each **manual checklist item** → one component or behaviour clearly assigned.
- Each **edge case** mentioned (zero buffer size, sample-rate mismatch, no audio device available, parameter at extreme value, xrun spike, …) → handled explicitly in the plan.

If a story element is intentionally **not implemented** in this story (e.g. depends on another story), say so explicitly with rationale and a link to the dependency story.

### Rule 3 — Standards compliance (`standards-compliance`)

The plan must be **verifiable** against `docs/standards/**`:

- **Architecture (`architecture.md`)**: hexagonal layering — no I/O imports in the domain (A1), no allocation/locking/syscall on the per-buffer realtime path (A2), one composition root, GUI ↔ audio communication only via the lock-free channel (A6 + F4).
- **Domain (`domain.md`)**: NewType wrappers for primitives (C5), `Result<T, E>` for expected failures (D1), distinct error taxonomies for domain vs infrastructure (D2), parse-don't-validate at the boundary (E2).
- **Testing (`testing.md`)**: Testing Trophy — integration as the load-bearing layer, e2e thin but real, units cover invariants. No module mocks (G2), test behaviour over implementation (G3), real dependencies over deep mocks at boundaries (G4).
- **Infrastructure (`infrastructure.md`)**: Conventional Commits in English (I1), reproducible builds (`Cargo.lock` committed, toolchain pinned), structured logging at I/O boundaries (J2 — never inline on the audio thread).

If the plan deviates from a standard, write a `Justification of deviation` section in the plan, with rationale and impact.

### Rule 4 — TBD-aware (`tbd-aware`)

This project's standards explicitly defer many decisions (plugin framework, GUI framework, crate layout, lock-free primitive, error library, logging crate, …). For each component that depends on a TBD:

- If the TBD is **already resolved** by an ADR or `dependencies.md` → cite the resolution and move on.
- If still TBD and the story can be implemented without it → say so and isolate the abstraction (a trait/port the future implementation will satisfy).
- If still TBD and the story **cannot** be implemented without it → flag `🔴 Blocking decision` in `## ⚠️ Decisions to validate before starting` and stop. Do not invent the choice.

---

## Style

Follow [implementation-plan-template.md](implementation-plan-template.md) **strictly**. Concision > grammar. Avoid code snippets unless prose would be three times longer (e.g. a port signature, an enum shape, a non-trivial DSP formula). File names are clickable links (`[file.rs](src/domain/...)`). Plans are in English; only this skill's prose is general guidance.

---

## End of skill

After producing all plans:

1. Recap to the user, in one paragraph:
   - How many plans written, under which paths.
   - The **3 biggest risks** identified across the spec (open 🔴 questions, inter-story dependencies, standards deviations, realtime-audio risks).
   - Commands to run before dev starts (e.g. "ensure `cargo test` is green on `main` before starting story 02").
2. If, while writing the plans, you found that a story is mis-sized (actually >1 day or not manually testable), **say so** — this is the moment to push for a re-split before code starts.

---

## Anti-patterns to avoid

- **Plan that paraphrases the story**: if a section reads like a copy-paste of the ACs, it is dead weight. The plan adds the _how_, not the _what_.
- **⚪ component without reading the file**: top source of TR bugs. Read before marking.
- **Everything is 🟢 new** _without justification_: in a greenfield repo this can be honest, but cross-check that no in-flight branch or sibling story already adds the component. If everything is 🟢 because the codebase exploration was skipped, re-run Phase 1.
- **Ignoring the standards**: if the plan puts a `cpal` import in `domain/`, that is not a review nit — it is a violation of rule A1 ("Domain has zero imports from audio I/O"). Catch it at plan time.
- **Inventing an open decision**: if the spec, the dependencies file, or the index flags a 🔴 question, do not decide — list options and their impact on the plan.
- **Forgetting realtime constraints**: any plan touching the audio callback that does not restate "no allocation, no locking, no syscall on the per-buffer path" is incomplete.
- **Heap allocation in DSP**: a plan that reaches for `Vec::push` inside the audio callback fails rule A2. Pre-allocate in the shell, hand a slice to the core.
