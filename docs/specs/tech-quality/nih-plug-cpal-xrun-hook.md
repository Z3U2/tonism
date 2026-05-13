# Spec: nih-plug Fork — cpal Stream-Error Hook

**Status:** Forward-looking — not scheduled for MVP. Target: v0.2.
**Last updated:** 2026-05-08

## Context

The MVP xrun counter ([mvp-03](../mvp/stories/mvp-03-xrun-detection-wired.md))
ships with a wall-clock budget heuristic inside `Plugin::process()`: if the
callback runs over `0.9 × buffer_duration`, increment the counter and push
an `AudioLogEvent::Xrun` onto the existing rtrb log bridge. That is the
entire signal — it catches **"process took too long"** but is blind to a
second class of underrun: **input drops cpal handles inside its own error
callback before `Plugin::process` is ever called**.

The cpal stream-error path is documented in
`crates/nih_plug/src/wrapper/standalone/backend/cpal.rs` of
[BillyDM/nih-plug](https://github.com/BillyDM/nih-plug) (the maintained
fork — see [ADR-002](../../adr/002-standalone-runner.md)). The cpal
input/output stream's error callback only calls `unparker.unpark()` to
stop the stream; there is no channel or hook the plugin can register.
Plugins therefore have no observable signal for
`cpal::StreamError::InputUnderflow` or `OutputUnderflow`. The limitation
is documented at the call site in [`src/audio/plugin.rs`](../../../src/audio/plugin.rs)
and [`src/gui/editor.rs`](../../../src/gui/editor.rs).

This spec describes the smallest fork change that surfaces the cpal
stream-error event to plugins, closing the false-negative class above
without any other behavioural change to nih-plug.

---

## Why a Fork, Not a Workaround

Three workarounds were considered during the [mvp-03 TR](../mvp/stories/mvp-03-xrun-detection-wired-implementation-plan.md)
and rejected:

| # | Workaround | Why rejected |
|---|---|---|
| 1 | A second cpal stream on the same device just to observe errors | Two streams contend for the same device; fragile across CoreAudio / WASAPI / ALSA. |
| 2 | macOS-only `mach_audio_thread_state` polling | Platform-specific; cross-platform support is a hard constraint per [ADR-002](../../adr/002-standalone-runner.md). |
| 3 | Subscribe to nih-plug's internal logging and grep for underrun lines | Brittle; relies on log-string stability across nih-plug versions. |

The fork is the canonical path because the underlying library already
emits the event — it just doesn't expose it to plugins.

---

## Proposed Fork Change

The change touches one file in the standalone wrapper:

```text
crates/nih_plug/src/wrapper/standalone/backend/cpal.rs
```

Add an optional callback the standalone wrapper invokes from inside the
cpal error closure, exposed as a default-impl trait method on `Plugin`.
Sketch:

```rust
// Sketch only — exact API to be settled in the upstream PR review.

/// Classification of the cpal stream-error event surfaced to plugins.
/// Carries only `Copy` data — no `String`, no allocation.
#[derive(Clone, Copy, Debug)]
pub enum StreamErrorKind {
    InputUnderflow,
    OutputUnderflow,
    BackendSpecific,
}

pub trait Plugin: Default + Send + 'static {
    // ... existing items ...

    /// Called from cpal's error-callback thread when the input or output
    /// stream reports an underrun. Default: no-op.
    ///
    /// Implementations must be lock-free and non-blocking — the cpal
    /// error callback runs on a thread the host owns, not the audio
    /// thread. A canonical body pushes onto a lock-free queue and
    /// returns immediately.
    fn on_stream_error(&self, _kind: StreamErrorKind) {}
}
```

The cpal error-callback closure inside the standalone wrapper invokes
`plugin.on_stream_error(kind)` *before* unparking the stream.

**Why a separate trait method, not a `ProcessContext` field**: the cpal
error callback fires off the audio thread (cpal's error path is on its
own thread on most backends). `ProcessContext` is only valid inside
`process()`. A dedicated method on `Plugin` matches the calling thread
and keeps the contract honest.

**Why no `String` in `BackendSpecific`**: the callback path must be
allocation-free. `BackendSpecific` carries only the discriminant; if
classification is needed in the future, a `&'static str` tag can be
added without breaking the contract.

**Why default-impl**: existing nih-plug plugins do nothing extra. The
trait method is purely additive; downstream plugins that don't override
it pay zero runtime cost and zero source-compat impact.

---

## Plugin-Side Adoption (Tonism)

Once the fork hook lands:

1. `TonismPlugin::on_stream_error` matches on `StreamErrorKind` and
   pushes onto the existing `rtrb` log bridge — the same path
   [mvp-03](../mvp/stories/mvp-03-xrun-detection-wired.md) wires for
   the budget-heuristic case.
2. Extend [`AudioLogEvent`](../../../src/audio/log_bridge.rs) with two
   variants: `Xrun` (existing, used by the budget heuristic) and
   `OutputXrun` (new). Both feed the same `XrunCounter`; the variant
   is distinguished only in the off-RT `tracing` output for diagnosis.
3. The wall-clock budget check from mvp-03 stays as a complement: it
   catches "process too slow" before cpal does. The two signals are
   logically distinct but feed the same counter — the user-visible
   xrun count is the union of both classes of event.
4. Behind a `cargo` feature `nih-plug-xrun-hook`, the plugin compiles
   with the upgraded behaviour; without the feature, the wall-clock
   heuristic stands alone. Lets the upstream PR ride at its own pace
   without blocking Tonism builds.

---

## Upstream Path

1. Open an issue on [BillyDM/nih-plug](https://github.com/BillyDM/nih-plug)
   describing the gap and proposing the trait shape. Get sign-off on the
   API before the patch.
2. Submit the PR. Tests: extend the existing standalone-mode test
   harness to simulate a cpal `StreamError` and assert the trait method
   fires.
3. Until merged, route through the [`Z3U2/nih-plug`](https://github.com/Z3U2/nih-plug)
   fork already in [`Cargo.toml`](../../../Cargo.toml) `[patch]` (used
   today for the macOS/Windows VST3 import fix). Add the xrun-hook
   commit on top of the existing `fix-macos-windows-vst3-import`
   branch — or split it into a separate branch for cleaner upstream
   review.
4. Once upstream merges, drop the `[patch]` entry and the
   `nih-plug-xrun-hook` cargo feature; the trait method becomes
   always-on.

---

## Tradeoffs

**Pros**

- Closes the false-negative class flagged in the
  [mvp-03 implementation plan](../mvp/stories/mvp-03-xrun-detection-wired-implementation-plan.md) —
  input underruns become observable.
- Reuses every piece of MVP plumbing — `XrunCounter`, the rtrb log
  bridge, the GUI poll. Only the *source* of the increment changes.
- Trivially small upstream surface (one trait method, one callback).
  A reviewer can read the diff in a minute.

**Cons**

- Fork-maintenance burden until the PR merges upstream. Mitigated by
  the `Z3U2/nih-plug` fork already existing for the VST3 import fix —
  the maintenance overhead is incremental, not new.
- The new method runs off the audio thread (cpal's error callback
  thread), so the rule [A2](../../standards/architecture.md) commitment
  is preserved but the lock-free push must still avoid `String`
  allocation in the error variant.
- A breaking change to the `Plugin` trait if not introduced with a
  default impl — mitigated by the default impl in the sketch.

---

## When to Adopt

**Not for MVP.** AC2 ships with the wall-clock heuristic. The manual
hardware run is the truth-source for AC4.

**v0.2 trigger.** When either (a) a real-world AC4 failure is traced
to an input drop the heuristic missed, or (b) the v0.2 multi-block
chain pushes audio-thread CPU close enough to the budget that the
false-negative class becomes routinely tripped. Either signal makes
the upstream PR a priority over feature work.

---

## References

- [mvp-03 implementation plan §⚠️ Decisions](../mvp/stories/mvp-03-xrun-detection-wired-implementation-plan.md)
- [mvp-03 user story](../mvp/stories/mvp-03-xrun-detection-wired.md)
- [BillyDM/nih-plug — standalone backend cpal source](https://github.com/BillyDM/nih-plug/tree/master)
- [cpal `StreamError` docs](https://docs.rs/cpal/latest/cpal/enum.StreamError.html)
- [`Z3U2/nih-plug` — existing fork (VST3 import fix)](https://github.com/Z3U2/nih-plug)
- [ADR-002 — Standalone runner](../../adr/002-standalone-runner.md)
- [docs/standards/architecture.md — A2 (no alloc/lock/syscall on per-buffer path)](../../standards/architecture.md)
