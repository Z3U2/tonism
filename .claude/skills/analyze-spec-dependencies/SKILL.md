---
name: analyze-spec-dependencies
description: Identify the Tech Lead dependencies (systems/crates, patterns, test data) that must be resolved before devs can start a spec under `docs/specs/<spec-slug>/spec.md` and its user stories. Trigger this skill as soon as the user asks to "detect/analyze dependencies", "prepare the Tech Lead work", or "list what blocks the devs" for a spec — even implicitly.
argument-hint: '<spec-slug>'
---

# Analyze Spec Dependencies

You are a Tech Lead reviewing a spec (and its user stories, if they exist) to identify what _you_ — not the dev — must address before development can start.

## Boundary between Tech Lead and Devs

1. **Dependencies**: Devs must not pick or set up new crates, plugin frameworks, audio backends, or external tools. The Tech Lead chooses, sets up, and documents them in `docs/standards/`.
2. **Patterns**: Devs must not invent architectural patterns absent from `docs/standards/`. The Tech Lead picks the pattern; the Dev implements.
3. **Test data**: Devs must not design the test data strategy. The Tech Lead defines _what_ test data is needed (audio fixtures, parameter snapshots, fake audio devices, NAM profiles, …) and _how_ to obtain it.
4. The Tech Lead does **not** implement the feature — that is the Dev's job.

## Goal

Given a `<spec-slug>` (e.g. `mvp`), produce a focused list of Tech Lead deliverables that unblock the dev. The dev will read this file before starting.

## Input

A single argument: `<spec-slug>` — the folder name of a spec under `docs/specs/`. Each slug folder contains `spec.md`, optional `stories.md`, optional `dependencies.md`, and an optional `stories/` subfolder.

If no argument is given, list the folders under `docs/specs/` and ask the user which spec to analyse. Do not guess.

## Output

Write to `docs/specs/<spec-slug>/dependencies.md`, **in English**.

---

## Phase 1 — Read the spec, stories, and standards

In **a single batched message**, read with the Read tool:

1. **The spec** — `docs/specs/<spec-slug>/spec.md` (mandatory).
2. **The stories index** — `docs/specs/<spec-slug>/stories.md` (if it exists; ignore if missing).
3. **The individual stories** — every file under `docs/specs/<spec-slug>/stories/*.md` (if the folder exists). Use Glob first if the count is unknown, then read all of them.
4. **All standards** — every file under `docs/standards/*.md`. Discover them dynamically:

   ```bash
   find docs/standards -type f -name '*.md'
   ```

   Read every file returned. The standards tree is flat (no subdirectories at the time of writing); keep this discovery step in case it grows.

5. **The product architecture** — `docs/specs/product-architecture.md` (cross-cutting context: product layers, definition of success, non-goals).
6. **Relevant ADRs** — `docs/adr/*.md`. Quickly skim filenames; read in full any ADR whose topic intersects the spec (e.g. language choice, plugin framework, GUI framework).

If the spec touches an area whose standards seem incomplete and you are unsure which rules apply, spawn **one** `Explore` subagent with the question: _"Which standards under `docs/standards/` cover the following areas: <list>? Quote the rules that apply, with file paths."_ Cap the report at 200 words.

Do not skim — partial reads produce wrong dependency lists. Do not re-read the same file twice.

---

## Phase 2 — Decide for each category

For each of the four categories below, decide whether the Tech Lead has work to do _before_ the dev can start. Empty lists are normal and expected — most features need no Tech Lead work.

### 1. What standards already cover

Cite the rule and the file path. One bullet per topic. Short — it's evidence the standards were read, not a full re-summary.

### 2. Dependencies (crates / audio backends / plugin frameworks / GUI frameworks / external tools)

Only when relevant. Include local development setup (toolchain pin, build scripts, env vars, audio device permissions on macOS, plugin installation paths). **Flag dependencies the standards _imply_ without naming a concrete choice** — e.g. `architecture.md` mentions a "lock-free channel" without picking a crate, or `infrastructure.md` mentions "structured logging" without picking `tracing` vs alternatives. This project carries many `TBD` decisions; surface every one that this spec forces.

### 3. Patterns

Only when relevant. Architectural or coding patterns the Tech Lead must pick and document in `docs/standards/` before the dev implements (e.g. how the audio thread receives parameter changes from the GUI, how preset files map to domain types, how DSP blocks are composed in the signal chain, how errors translate from infrastructure to domain).

### 4. Test data strategy

Only when relevant. _What_ data is needed to test the feature (manually + automatically), and _how_ to obtain it: audio fixtures (sine sweeps, impulses, recorded guitar takes), reference NAM/IR files, fake audio devices for in-memory tests, parameter snapshots, latency-measurement signals, stress-test session inputs.

---

## Phase 3 — Write the file

Write to `docs/specs/<spec-slug>/dependencies.md` using the template below.

### Rules

- Prefer simplicity of implementation.
- Cover important topics only — not implementation details.
- Empty lists are fine. If a section has nothing, keep the heading and write a single line stating that no Tech Lead work is needed.
- Rationales **under 10 words**.
- **One alternative per topic** — the runner-up the Tech Lead considered, in `(alt: …)`.
- Bullet shape: `- <Topic>: <Choice> — <rationale ≤10 words> (alt: <alternative>)`.
- All standards paths are flat (`docs/standards/<file>.md`), not nested by area.

### Template

```markdown
# Tech Lead Dependencies — <human title of the spec>

> Derived from [spec.md](spec.md)<, and [stories.md](stories.md) if present>. To be resolved by the Tech Lead **before** devs start implementation.

## Already covered by standards

- <Topic>: <rule>, see [docs/standards/<file>.md](../../standards/<file>.md)
- …

## Dependencies

- <Topic>: <Choice> — <rationale ≤10 words> (alt: <alternative>)
- …

## Patterns

- <Topic>: <Choice> — <rationale ≤10 words> (alt: <alternative>)
- …

## Test data

- <Topic>: <Choice> — <rationale ≤10 words> (alt: <alternative>)
- …
```

If a section is empty, replace its bullet list with a single line: _"No Tech Lead dependency to resolve — devs can start."_

---

## Final handoff

After writing the file, summarise to the user in one short paragraph:

- Number of items per category.
- The top 1–2 blocking decisions for the Tech Lead.
- Any open question you could not resolve from the spec + standards alone.

---

## Anti-patterns to avoid

- **Re-stating the spec.** This file is about Tech Lead deliverables, not a feature summary.
- **Listing implementation details.** "Add an `xrun_count` field to the audio adapter" is a dev task, not a Tech Lead dependency. _"Pick the lock-free primitive for GUI ↔ audio messaging and document its usage in `architecture.md`"_ is.
- **Inventing dependencies.** If the spec + stories can be implemented with what is already in `docs/standards/` and the codebase, the lists are empty. That's the right answer.
- **Listing every standard you read.** "Already covered by standards" only mentions standards that are _directly relevant_ to this spec.
- **No alternative.** Every topic gets one runner-up so the Tech Lead can see the trade-off without re-deriving it.
- **Ignoring `TBD` markers in standards.** This project's standards explicitly defer many decisions. If the spec depends on a `TBD`, that `TBD` becomes a Tech Lead dependency for this spec.

---

**Spec slug:** $ARGUMENTS
