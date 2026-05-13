# Tonism — Product Architecture

**Status:** Draft
**Last updated:** 2026-05-06

## Problem statement

As a gigging guitarist, I want to play live using digital amp simulators and NAM (Neural Amp Modeler) profiles. Existing open-source solutions fall short on three fronts: the UI is unpolished, real-time performance is inconsistent, and signal-chain customization is constrained. Tonism is a real-time guitar processing tool that closes those gaps for guitarists who play live.

## Target user

**Primary:** gigging guitarists (the author included). Users who play in front of an audience, or rehearse seriously toward that, and need a tool they can rely on under pressure.

**Not the target:** hobbyist bedroom players, producers, studio engineers, or developers looking for a DSP playground. Decisions are made for the gigging persona, not for the broadest possible audience.

## Strategic intent

Tonism is a **personal tool, shared as open source**. It is built primarily because the author wants it. The source is open because there's no reason not to publish it, and because OSS guitarists may benefit. **Adoption is not a success metric.** No KRs, no roadmap items, and no design decisions are driven by stars, downloads, or contributor count.

## Definition of success

The single sharpest success signal:

> **"I played a full song live through Tonism without a glitch."**

Concretely, that means:

| Dimension | Bar |
|---|---|
| Test condition (MVP) | Bedroom/studio session through headphones |
| Round-trip latency | < 10 ms, measured on the dev machine |
| Audio reliability | Zero buffer underruns over a continuous 5-minute session |
| Stability | Crash-free 30-minute stress session with parameter changes |
| Tone (MVP) | Signal continuity is enough; gig-acceptable tone is a v0.2 goal |
| Tone (v0.2) | At least one tone the author would actually gig with |

Quantitative targets live in [okrs/q1.md](../okrs/q1.md). The week-1 scope that satisfies the MVP bars lives in [mvp/spec.md](mvp/spec.md).

## Non-goals

The following are explicitly **not** what Tonism is trying to be — listing them keeps scope honest:

- A product chasing user adoption, stars, or contributors
- A tool for non-guitar instruments (bass, synths, vocals)
- A full DAW or recording environment
- A plugin host that competes with commercial DAWs
- A studio-grade mixing or mastering tool
- A teaching/learning tool for beginners
- A cloud-first SaaS with accounts, auth, or hosted profiles (a future cloud library, if any, would be opt-in and offline-first)

## Product layers

The product is composed of six conceptual layers. Each owns a user-facing concern and a primary failure mode the product must defend against. These layers are deliberately product-level, not code modules — the implementation mapping lives in [ADR-001](../adr/001-language-choice.md).

| Layer | Concern | Primary failure mode |
|---|---|---|
| **Capture** | Guitar audio enters the system | Buffer underruns, dropped samples, input clipping |
| **Signal chain** | Ordered processing blocks the user composes (amps, cabs, effects, NAM profiles) | Chain rebuilds that interrupt audio; non-deterministic CPU spikes |
| **Render** | Processed audio reaches the user's ears | Output clipping, channel routing errors, latency stack-up |
| **Tone state** | What the user has dialled in (parameters, loaded profiles) | Parameter zipper noise; state desync between UI and audio thread |
| **Control surface** | How the user manipulates state (UI knobs, future MIDI/footswitches) | Unresponsive controls; control changes that crash or glitch the audio thread |
| **Persistence** | Saving/loading tone state across sessions | Lost presets; corrupt state files |

Naming the failure modes per layer is what makes the OKR key results concrete: each KR defends a specific failure mode.

## Architecture decisions

| Concern | Decision | Reference |
|---|---|---|
| Language, plugin framework, audio I/O, GUI | Rust + nih-plug + cpal + egui | [ADR-001](../adr/001-language-choice.md), [ADR-004](../adr/004-gui-library-egui.md) |
| Quarterly objectives & key results | See OKRs Q1 | [okrs/q1.md](../okrs/q1.md) |
| Week-1 MVP scope and acceptance criteria | See MVP spec | [mvp/spec.md](mvp/spec.md) |
| Commit message conventions | Conventional Commits, English | [standards/commit-style.md](../standards/commit-style.md) |

## How this document evolves

- The **problem statement, target user, intent, and non-goals** are stable. They change only on a deliberate strategic shift.
- The **definition of success** evolves as bars are met (MVP → v0.2 → beyond). Past bars are kept in the changelog at the bottom of this file rather than overwritten.
- The **product layers** evolve when a new user-facing concern appears (e.g. cloud library, MIDI footswitch). Adding a layer is a product decision, not an implementation detail.
