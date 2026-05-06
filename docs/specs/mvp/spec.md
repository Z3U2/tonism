# Tonism — MVP Spec

**Version:** v0.1
**Timebox:** 1 week — 2026-05-06 → 2026-05-13
**Status:** Draft
**Owner:** author (solo)

## Goal

A standalone real-time guitar processor that lets the author play a full song through it, in a bedroom session through headphones, without a perceived audio glitch.

That is the entire MVP. Anything that does not directly contribute to that goal is out of scope.

## Why this is the MVP

The success signal is "I played a full song live without a glitch" ([product-architecture.md](../product-architecture.md#definition-of-success)). Tone refinement, profile loading, plugin export, and preset management all *assume* a working real-time signal path. If the path doesn't work, none of those features matter; if the path does work, every later feature is a known increment on a proven foundation.

The MVP therefore exists to validate one thing only: **the real-time path is glitch-free.**

## Acceptance criteria

The MVP is accepted when all four criteria below are met. They are 1:1 with [OKR Q1 KR1.1–KR1.4](../../okrs/q1.md#objective-1--prove-a-glitch-free-real-time-guitar-signal-path).

| # | Criterion | How verified |
|---|---|---|
| AC1 | Round-trip latency < 10 ms on the dev machine | Measured value displayed in-app and recorded with the measurement method in a brief note |
| AC2 | Zero buffer underruns during a continuous 5-minute session | In-app xrun counter visible during the session; counter reads 0 at the end |
| AC3 | Crash-free 30-minute stress session with parameter changes throughout | Process runs for 30 minutes; author varies controls during the session; no crash, no audio dropout escalating to xruns |
| AC4 | Author plays a full song end-to-end without a perceived glitch | Subjective, binary; author calls it after the run |

If any one criterion fails, the MVP is not complete. Partial credit is not a thing here.

## In scope (must-have)

Only the minimum required to satisfy the four acceptance criteria:

- **Real-time audio capture + playback path.** Guitar in, processed audio out, low buffer size.
- **One processing block in the chain.** Any DSP that proves the path is alive — even a single gain stage is sufficient. This is intentionally loose: the path is what's being proven, not the DSP.
- **Bypass toggle.** A way to A/B that the chain is doing something. Without this, AC4 ("without a glitch") is uncheckable — the author needs to confirm it's actually being processed.
- **Latency measurement.** A method (round-trip impulse, or equivalent) to produce the AC1 number, and a place in the UI to show it.
- **Buffer-underrun counter.** A live counter, visible during the session, that increments on every xrun. Required for AC2.
- **Minimal control surface.** Input gain and output gain knobs. Enough to set levels and run the session.

## Explicitly out of scope (week 1)

These are good ideas. They are not week-1 ideas.

- NAM profile loading (see [OKR KR2.1](../../okrs/q1.md#objective-2--reach-a-tone-the-author-would-actually-gig-with) — Q1 later)
- Multiple amps, effects, cab IRs
- Preset save/load
- Plugin (VST3/CLAP) export — standalone only
- Cloud library / sharing
- Signal-chain editor UI (drag-and-drop blocks)
- Themed or polished UI
- MIDI / footswitch control
- Cross-platform packaging (build on the dev machine; ship later)

## Verification protocol

Run in this order on the dev machine, headphones connected, a guitar plugged in.

1. **Latency check (AC1).** Run the latency measurement. Record the number. Record the method (buffer size, sample rate, audio backend, OS). Pass = number < 10 ms.
2. **Stability check (AC2).** Start a session. Play continuously or leave silent for 5 minutes. Watch the xrun counter. Pass = counter at 0 at the end.
3. **Stress check (AC3).** Start a session. Over 30 minutes, periodically adjust input gain, output gain, and toggle bypass. Pass = no crash, xrun counter does not climb beyond what AC2 already validated.
4. **Success signal (AC4).** Play a full song through Tonism. Pass = author confirms no perceived glitch.

If AC1–AC3 pass but AC4 fails, the MVP has a *perceived* glitch the instruments missed. Investigate before declaring v0.1 done.

## What "done" unblocks

Closing the MVP unblocks O2 (tone work) and the rest of Q1. It does **not** unblock plugin export, cloud library, or any community-facing work — those are not Q1 priorities.

## Risks and how we handle them on a 1-week budget

| Risk | Mitigation |
|---|---|
| Latency target too aggressive on the dev OS/audio backend | If < 10 ms is unreachable in week 1, document the achievable number and the bottleneck. Do not silently relax AC1 |
| The "one DSP block" turns into scope creep (multiple amps, EQ tweaking) | Hold the line: any block that produces audible output passes. Tone is v0.2 |
| Author finds a glitch on AC4 that the counters didn't catch | Treat as a real defect. The MVP is not done. Diagnose the gap between counter coverage and perception |
| Week runs long | Cut scope, not criteria. The acceptance criteria are the contract |

## Changelog

- 2026-05-06: initial draft.
