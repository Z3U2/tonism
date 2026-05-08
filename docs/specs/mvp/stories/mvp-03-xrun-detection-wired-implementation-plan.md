# Implementation plan: mvp-03 — Wire xrun detection into the audio callback

**Story**: [mvp-03-xrun-detection-wired.md](mvp-03-xrun-detection-wired.md)
**Spec**: [spec.md](../spec.md)
**Layers**: Capture · ~~Signal chain~~ · ~~Render~~ · ~~Tone state~~ · Control surface · ~~Persistence~~ · Tests
**Complexity**: 🟡 Medium

---

## ✅ Decisions taken

The TR decisions previously flagged for this story are resolved.

### D1 — Underrun detection mechanism: Option A (wall-clock budget check)

Ship the wall-clock budget check inside `Plugin::process()` as the MVP xrun signal. At callback entry: `let frame_start = std::time::Instant::now();`. At callback exit (before `ProcessStatus::Normal`): compute `budget_ns = (buffer.samples() as u128 * 1_000_000_000) / self.sample_rate as u128`; if `frame_start.elapsed().as_nanos() > (budget_ns * 9) / 10`, increment `xrun_counter` and push `AudioLogEvent::Xrun` onto the existing rtrb log bridge. Lifts the `#[allow(dead_code)]` off `audio_logger` and `log_drain`.

**Realtime safety of `Instant::now()`**: macOS uses `mach_absolute_time` (no syscall), Linux uses `clock_gettime(MONOTONIC)` via vDSO (no kernel transition), Windows uses `QueryPerformanceCounter` (no syscall). All three are RT-acceptable in practice; documented in the code comment at the call site.

**Known false-negative class**: Option A catches "process took too long" but is blind to cpal-side input drops the cpal error callback handles before `process()` is called (CoreAudio / ALSA pull-based output keeps calling us on schedule with stale/zero input). The fix is **Option B** — a one-method nih-plug fork hook surfacing `cpal::StreamError`, captured separately as a v0.2 follow-up. The fork-change spec lives at [docs/specs/tech-quality/nih-plug-cpal-xrun-hook.md](../../tech-quality/nih-plug-cpal-xrun-hook.md). When that lands, the same `XrunCounter` and rtrb log-bridge plumbing accept a second source feeding the same counter — no architectural change required.

**Why Option A now**: the spec timebox is one week; the fork PR absorbs days of upstream review and CI iteration. The MVP's success bar is "the counter is alive when we see a glitch in the manual run," not "the counter catches every kind of underrun." The manual AC4 run is the truth-source for week 1.

### D2 — Counter persistence: accumulate across stream restarts

If the audio device closes and reopens (sample-rate change, device-disconnect-reconnect on macOS), the counter accumulates rather than resets. Resetting on every stream restart would hide flapping issues. Documented in code; users who want a fresh count restart the standalone.

---

## 1. Summary

Add a wall-clock budget check to `TonismPlugin::process` ([plugin.rs:166–213](../../../../src/audio/plugin.rs)) that increments the existing `XrunCounter` and pushes an `AudioLogEvent::Xrun` onto the existing rtrb log bridge whenever a `process()` call takes more than 90 % of its per-buffer time budget. Removes the `#[allow(dead_code)]` markers on `audio_logger` and `log_drain` (lines 53, 62). The technical edge: keep `Instant::now()` calls (two per `process` invocation) RT-safe in practice, and trade a known false-negative class (cpal-side drops invisible to `process()`) for zero upstream nih-plug changes inside the 1-week MVP budget.

---

## 2. Functional scope

### Mapping acceptance criteria → components

| Criterion / Case                                                                    | Layer           | Main component                                                | Status |
| ----------------------------------------------------------------------------------- | --------------- | ------------------------------------------------------------- | ------ |
| Idle 1-min observation (buffer ≥ 256, 48 kHz): xrun stays at 0                     | Capture         | `record_xrun_if_overrun` budget check                         | 🟢     |
| Forced underrun → counter increments within GUI poll cadence                        | Capture         | Budget check + `XrunCounter::bump`                            | 🟡     |
| Each xrun produces one tracing event on the log drain                               | Capture         | `AudioLogger::log(AudioLogEvent::Xrun)`                       | ⚪     |
| Spurious early-buffer event during stream startup is documented / filtered          | Capture         | First-buffer skip flag                                        | 🟢     |
| Counter accumulates across stream restarts (per D2)                                 | Capture         | `XrunCounter` is owned by `TonismPlugin`, persists across `initialize()` | ⚪     |
| Detection path never panics, allocates, or blocks                                   | Capture         | `Instant::now()` + atomic ops + rtrb push                     | 🟢     |
| Toggle bypass / sweep gain → counter unchanged by parameter changes alone           | Capture         | Budget check is independent of params                         | 🟢     |

**Manual checklist**:
- "Launch standalone, observe `xrun: 0` for 1 minute" → AC verification by running `cargo run --release`.
- "Force-underrun toggle → counter increments, no alloc panic, no crash" → developer-only `force-xrun` cargo feature gates a `std::thread::sleep(2 * budget)` inside `process()` for testing.
- "Off-RT log shows one event per increment" → existing `forward_event` in [log_bridge.rs:77–83](../../../../src/audio/log_bridge.rs).
- "Restore stable buffer size → counter stops climbing, existing count remains" → D2 = accumulate.
- "Toggle bypass and sweep gain during stress → counter behaviour independent of params" → per-buffer budget check is agnostic to the param values used inside the buffer.

### Out of scope (declared)

- Detecting cpal-side input drops (Option B above) — follow-up.
- Resetting the counter from the UI — manual restart for now; UI button is v0.2.
- Per-channel xrun attribution — the budget is whole-buffer; no useful per-channel signal.

---

## 3. Domain & data model

No domain change. No new types.

(`AudioLogEvent::Xrun` ([log_bridge.rs:28](../../../../src/audio/log_bridge.rs)) already exists; `tracing` mapping ([log_bridge.rs:79–82](../../../../src/audio/log_bridge.rs)) already exists.)

---

## 4. Architecture

### 4.1 Domain — pure core

No domain change.

### 4.2 Audio adapter — realtime shell

> Rule A2 reminder: **no allocation, no locking, no syscall on the per-buffer path**.

```diff
audio callback: TonismPlugin::process
├── 🟡 📡 TonismPlugin::process()                                                          src/audio/plugin.rs:166–213
+      Add wall-clock budget instrumentation:
+        - At entry (after the bypass early-return): `let frame_start = Instant::now();`
+        - At exit (before `ProcessStatus::Normal`):
+            let budget_ns = (samples * 1_000_000_000) / sr;
+            if frame_start.elapsed().as_nanos() as u64 > (budget_ns * 9) / 10 {
+                self.xrun_counter.bump();
+                self.audio_logger.log(AudioLogEvent::Xrun);
+            }
+      Drop `#[allow(dead_code)]` on lines 53 and 62 — `audio_logger` is now actively called.
+      Update the lines 50–52 comment to reflect Option A behaviour and document the false-negative class.
│   ├── ⚪ 📊 XrunCounter                                                                    src/audio/xrun.rs
+              Reused as-is. `bump()` is `Relaxed` AtomicU64 fetch_add — RT-safe.
│   ├── ⚪ 🚇 AudioLogger                                                                    src/audio/log_bridge.rs
+              Reused as-is. `log()` is non-blocking rtrb push.
│   └── 🟢 🧩 record_xrun_if_overrun()                                                       src/audio/plugin.rs (or a private free fn)
+              Extracted helper: `fn record_xrun_if_overrun(start: Instant, samples: usize, sr: f32, counter: &XrunCounter, logger: &mut AudioLogger) -> bool`.
+              Returns `true` if it bumped, for unit-testability.
+              Pulled out so the budget logic is testable without driving real audio.
└── ⚪ 🎯 SampleRate                                                                         src/domain/types.rs
```

#### Realtime constraints checklist

- [x] **No alloc**: `Instant::now()` does not allocate; atomic ops are wait-free; `rtrb::Producer::push` is non-allocating (capacity is fixed at construction).
- [x] **No lock**: no `Mutex`/`RwLock`; only atomic primitives.
- [x] **No syscall on the budget path**: `Instant::now()` resolves through `mach_absolute_time` / `clock_gettime` vDSO / `QueryPerformanceCounter` — none transition into the kernel on the supported OSes. Document in the code comment.
- [x] **Drop-order invariant** ([plugin.rs:44–48](../../../../src/audio/plugin.rs)) preserved — `audio_logger` declared before `log_drain`, no field-ordering changes.
- [x] First-buffer skip: a one-time `bool` field `first_process_call: bool` initialized to `true` in `Default`, set to `false` after the first `process()` invocation, and skipped from the budget check. Avoids cold-start false positives (allocator warm-up, first cache miss). Documented as the "spurious early-buffer event" case.

### 4.3 Control surface — GUI / MIDI

```diff
src/gui/editor.rs
└── 🟡 🪟 create()                                                                          src/gui/editor.rs:39–44
+      Update the comment block: counter now updates via the wall-clock budget heuristic;
+      true cpal underrun events remain invisible until Option B lands.
```

The xrun Label, SyncSignal, Timer, and Memo ([editor.rs:53–85](../../../../src/gui/editor.rs)) are reused as-is. Once the audio thread starts bumping the counter, the existing 60 Hz poll renders the increments without GUI code changes — pure plumbing payoff.

### 4.4 Persistence

No persistence change.

### 4.5 Composition root

No composition-root change. `XrunCounter` and `AudioLogger` are already wired in `TonismPlugin::Default` ([plugin.rs:66–81](../../../../src/audio/plugin.rs)).

The two `#[allow(dead_code)]` annotations on lines 53 (`audio_logger`) and 62 (`log_drain`) are removed.

🟢 New `first_process_call: bool` field added to `TonismPlugin` and to `Default`.

### 4.6 Key technical decisions

- **0.9 × budget threshold** — leaves 10 % margin for `Instant::now()` jitter and short cache-miss spikes that are not actual xruns. Tunable; document the choice.
- **`Relaxed` ordering on the counter** — `XrunCounter::bump` already uses `Relaxed` ([xrun.rs:12](../../../../src/audio/xrun.rs)); fine because the GUI tolerates a few-frame staleness.
- **First-call skip** — robust against startup variance without a complex warm-up phase.
- **Helper function `record_xrun_if_overrun` is `pub(crate)`** — exposes it to unit tests in `src/audio/` `mod tests` while keeping it out of the public API.
- **Cargo feature `force-xrun`** — gates a `std::thread::sleep(2 * budget)` inside `process()` for the manual / integration test cases. Strictly developer-only; never on in release.

### 4.7 Justification of deviation from standards

None. The plan respects A2 (heuristic only — `Instant::now` is documented RT-safe on the three target OSes), F4 (one-way audio → GUI via `Arc<AtomicU64>`), J2 (logging from the realtime thread goes through the lock-free rtrb queue, drained off-RT). No deviation.

---

## 5. Tests

### 5.1 e2e (audio path)

The story's manual checklist is the e2e for AC2. The 5-min idle and 30-min stress integration tests live in [mvp-04](mvp-04-idle-5min-stability-test.md) and [mvp-05](mvp-05-stress-30min-test.md) — both consume the `XrunCounter` this story makes count.

### 5.2 Integration

- 🟢 `tests/xrun_force_increments.rs::process_overrun_increments_counter` — gated `#[cfg(feature = "force-xrun")]`. Drive a thin `TonismPlugin` harness through one `process()` call with the `force-xrun` feature on; assert `xrun_counter.read() > 0` and that one `AudioLogEvent::Xrun` was forwarded to tracing (use `tracing-test` or a custom subscriber to capture).
- ⚠️ Driving `TonismPlugin::process()` directly is non-trivial — `Buffer` is a nih-plug type that the standalone wrapper builds internally. The pragmatic path: extract the `record_xrun_if_overrun` helper into a free function and unit-test it directly (5.3 below); the integration test uses the helper, not the full plugin.

### 5.3 Unit (pure audio shell) — co-located

- 🟢 `record_xrun_if_overrun_bumps_when_over_budget` — co-located in `src/audio/plugin.rs` `mod tests`. Builds an `XrunCounter`, an `AudioLogger`, calls `record_xrun_if_overrun(start, samples, sr, &counter, &mut logger)` with `start` set to `Instant::now() - Duration::from_millis(50)` and a budget of 5 ms. Asserts return `true`, `counter.read() == 1`.
- 🟢 `record_xrun_if_overrun_does_not_bump_within_budget` — same with `start = Instant::now()` (zero elapsed); asserts `false`, `counter.read() == 0`.
- 🟢 `record_xrun_if_overrun_does_not_alloc` — same call sequence under `#[cfg(feature = "debug-assert-no-alloc")]`; expects no panic.
- 🟢 `audio_logger_log_drain_xrun_path` — extends the existing `drain_thread_exits_cleanly_after_one_xrun` ([log_bridge.rs:152–165](../../../../src/audio/log_bridge.rs)) with assertion that `tracing::warn!` was emitted (capture via custom subscriber).

### 5.4 AC coverage table

| AC / Checklist item                                            | Test                                                            |
| -------------------------------------------------------------- | --------------------------------------------------------------- |
| 1-min idle, counter stays 0                                    | **manual** + (covered by [mvp-04](mvp-04-idle-5min-stability-test.md))      |
| Forced underrun → counter increments                           | unit `record_xrun_if_overrun_bumps_when_over_budget`            |
| Each increment produces one tracing event                      | integration `audio_logger_log_drain_xrun_path`                  |
| Spurious early-buffer event filtered                           | unit (a test sets `first_process_call = true` and asserts no bump even when over budget) |
| Counter accumulates / resets per D2                            | reading review (counter is owned by `TonismPlugin`, lives across `initialize()`) |
| Detection path never panics / allocates / blocks               | unit `record_xrun_if_overrun_does_not_alloc`                    |
| Counter independent of bypass / gain sweeps                    | reading review (budget check is post-process; sees only elapsed time) |

---

## 6. Dependencies and execution order

- **Prerequisite stories**: none — relies only on already-existing `XrunCounter` and `AudioLogger`.
- **Stories unblocked**: [mvp-04](mvp-04-idle-5min-stability-test.md) and [mvp-05](mvp-05-stress-30min-test.md) — both want a counter that actually counts when their tests assert "xrun == 0" and "xrun within tolerance".
- **Commands to run** (local + CI):
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test xrun`
  - `cargo test --features debug-assert-no-alloc xrun`
  - `cargo test --features force-xrun xrun_force_increments`
  - **Manual**: `cargo run --release` for 1 min, observe `xrun: 0`; rerun with `--features force-xrun`, observe increments.

---

## 7. Risks and open questions

- 🟢 **D1 (mechanism) and D2 (counter persistence) resolved** — see "Decisions taken" above. The known false-negative class (cpal-side input drops invisible to `process()`) has a documented v0.2 follow-up at [docs/specs/tech-quality/nih-plug-cpal-xrun-hook.md](../../tech-quality/nih-plug-cpal-xrun-hook.md).
- 🟡 **`Instant::now()` portability claim** — RT-safe on the three target OSes by reading `mach_absolute_time` / vDSO `clock_gettime` / `QueryPerformanceCounter`. Worth a one-line code comment citing the source, in case a future contributor questions the syscall claim.
- 🟡 **0.9 budget threshold may need tuning** — first run on the dev machine is the calibration moment. Document expected behaviour: AC2 should pass at 256-frame buffer with no false positives.
- 🟢 **All plumbing already exists** — `XrunCounter`, `AudioLogger`, log drain thread, GUI poll. Story is mostly removing two `#[allow(dead_code)]` flags and adding ~20 lines of instrumentation.

---

## 8. References

- Similar implementations to follow: existing budget-style instrumentation patterns are absent in this repo, but `Instant::now` + atomic counter is the canonical real-time pattern (cpal's own `StreamInstant` uses the same primitive). The GUI poll → atomic read pattern is documented in [editor.rs:39–66](../../../../src/gui/editor.rs).
- Directly applicable standards: [architecture.md](../../../standards/architecture.md) (A2, F4), [infrastructure.md](../../../standards/infrastructure.md) (J2 — log via lock-free queue), [testing.md](../../../standards/testing.md) (G3, G4, G5, G7).
- Related ADRs: [ADR-002](../../../adr/002-standalone-runner.md) (nih-plug ownership of the audio loop — explains why we cannot directly hook cpal stream errors).
- Source spec section: [acceptance criteria AC2](../spec.md#acceptance-criteria); [dependencies — xrun counter](../dependencies.md#patterns); the 2 `#[allow(dead_code)]` lines this story removes: [plugin.rs:53, 62](../../../../src/audio/plugin.rs).
