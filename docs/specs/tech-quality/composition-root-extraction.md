# Spec: Composition Root — Extract Inline DSP and Infrastructure

**Status:** Extracts 1–5 landed. Extract 6 decided, not yet implemented.
**Last updated:** 2026-06-04

## Context

[A5](../../standards/architecture.md) designates `src/cpal_direct.rs` as the
single composition root. Its job is to **construct** components and **wire**
them — not to implement signal processing or infrastructure plumbing inline.

Today `build_streams()` is ~190 lines because it does both: it constructs
domain blocks and audio infrastructure, but it also contains inline DSP
arithmetic (sine generation, per-sample smoothed gain, channel deinterleave)
and infrastructure logic (ring sizing, pre-fill, A2 guard) directly inside
the callback closures. This violates the spirit of
[A1](../../standards/architecture.md) (domain purity) and
[A5](../../standards/architecture.md) (composition root wires, doesn't
implement), and makes the signal processing untestable in isolation.

The goal: after extraction, each callback closure should read as a pipeline
of `.process()` calls on domain blocks, with infrastructure concerns handled
by dedicated audio-layer types. The composition root wires; everything else
lives where it belongs.

---

## Extractions

### 1. `TestOscillator` → `domain/blocks/`

**What:** The 440 Hz sine generator — phase accumulator (`phase`, `phase_inc`),
`phase.sin()`, and the TAU import — is a pure DSP concept inline in the input
callback (lines 236, 247–248, 259 of `cpal_direct.rs`).

**Where:** New `domain/blocks/test_oscillator.rs` implementing `Process`. The
composition root should not import `std::f32::consts::TAU`.

**Shape:**

```rust
pub struct TestOscillator {
    frequency: f32,
    phase: f32,
    phase_inc: f32,
}

impl TestOscillator {
    pub fn new(frequency: f32) -> Self { ... }
    pub fn next_sample(&mut self) -> f32 { ... }
}

impl Process for TestOscillator {
    fn prepare(&mut self, sr: SampleRate, _max_block: usize) {
        self.phase_inc = TAU * self.frequency / sr.value();
    }
    fn reset(&mut self) { self.phase = 0.0; }
    fn process(&mut self, buf: &mut [f32]) {
        for sample in buf.iter_mut() {
            *sample = self.phase.sin();
            self.phase = (self.phase + self.phase_inc) % TAU;
        }
    }
}
```

The input callback becomes: `if test_signal { osc.next_sample() } else { data[i] }`.

### 2. Inline smoothed gain → evolve `Gain` to accept per-frame input

**What:** Both callbacks manually do the per-sample
`Decibels::new(smoother.next()) → GainLinear → multiply` loop. This is the
same work `Gain::process` does, but for smoothed parameters it's hand-rolled
inline.

**Decision:** No `SmoothedGain` block. Smoothing is per-parameter, not
per-block — creating `SmoothedGain` would mean needing `SmoothedFilter`,
`SmoothedCompressor`, etc. as future blocks land. Instead:

- **`Gain`** evolves to accept a changing `db` value (set per-buffer or
  per-frame from the outside, rather than reading a fixed field).
- **`LinearSmoother`** stays in domain (pure math, already correctly placed).
- **`SmoothedFloatParam`** stays in params infra (atomic read + smoother).
- **The composition root** wires them: reads the smoothed value, sets it on
  the block, calls `process()`. This is the composition root's job.

The pattern generalises to every future effect: dumb blocks that take values
and process buffers, smart wiring that orchestrates per-frame parameter
feeding. No parallel `Smoothed*` hierarchy needed.

**Status:** Design decided. Implementation deferred — requires a `Gain` API
change (current `Gain.db` is a fixed field set at construction).

### 3. Channel deinterleave/reinterleave → `domain/buffer.rs`

**What:** The channel-0 extract-process-write-back loop (lines 298–304) is a
pure buffer operation with no I/O or device dependency.

**Where:** New `domain/buffer.rs` (or utilities in `domain/types.rs`):

```rust
pub fn deinterleave_channel(interleaved: &[f32], channel: usize, channels: usize, out: &mut [f32]);
pub fn interleave_channel(out: &mut [f32], channel: usize, channels: usize, source: &[f32]);
```

These are reusable for any future per-channel processing (EQ, compression)
and independently testable with simple buffer assertions.

### 4. Ring buffer construction → `audio/ring.rs`

**What:** The latency-based ring sizing, `LATENCY_MS` constant, pre-fill
loop, and the `CAPTURE_LEN` assertion (lines 189–202) are audio
infrastructure concerns — they're about the transport between callbacks,
not domain processing.

**Where:** New `audio/ring.rs`:

```rust
pub struct AudioRing {
    pub producer: rtrb::Producer<f32>,
    pub consumer: rtrb::Consumer<f32>,
    pub latency_frames: usize,
}

impl AudioRing {
    pub fn new(sample_rate: u32, channels: u16, latency_ms: f32) -> Self { ... }
}
```

The `CAPTURE_LEN` assertion belongs here too since `CAPTURE_LEN` already
lives in `audio::latency`. The `LATENCY_MS` constant moves with it.

### 5. `assert_no_alloc_audio` → `audio/rt_guard.rs`

**What:** The cfg-gated A2 enforcement wrapper (lines 67–85) is an
audio-thread infrastructure mechanism. It has nothing to do with composition.

**Where:** `audio/rt_guard.rs` — a one-function module re-exported from
`audio::rt_guard::assert_no_alloc_audio`.

### 6. `err_fn` and `device_label` → `device.rs`

**What:** Both are cpal-specific utilities (lines 528–539). `err_fn` is the
stream-error callback; `device_label` extracts a human-readable name.

**Where:** `src/device.rs`, alongside the existing device enumeration and
`resolve_initial_config`. `device_label` is already mirrored in
`scripts/check_buffer_size.rs` — centralizing removes the duplication.

---

## What Stays in the Composition Root

- **`build_streams()`** — but slimmed to: create `AudioRing`, create domain
  blocks (`TestOscillator`, `Gain`, `LatencyMeter`), wire params, compose
  callbacks as pipelines of `.process()` calls, build cpal streams, return
  `AudioStreams`. The per-frame smoothed gain read → set → process wiring
  stays here — that's composition, not DSP.
- **`run_gui()` / `run()`** — the two entry points sequencing
  init → build → run.
- **`AudioStreams`** — the ownership bundle returned by `build_streams`.
- **`parse_cli()`** — CLI parsing is a shell concern; the composition root
  *is* the shell.
- **`find_device_index()`** — GUI-wiring glue. Small enough to stay, though
  it could move to `gui/` since only the GUI path uses it.
- **`spawn_ramp_thread()`** — debug/test orchestration, appropriate for the
  composition root.

---

## Expected Result

After extraction, the input callback body should read roughly:

```rust
move |data: &[f32], _| {
    assert_no_alloc_audio(|| {
        for frame in data.chunks(channels) {
            let sample = if test_signal.value() {
                oscillator.next_sample()
            } else {
                frame[0] // simplified; real version iterates channels
            };
            let scaled = if bypass.value() { sample } else { input_gain.apply(sample) };
            if producer.push(scaled).is_err() { fell_behind = true; }
        }
        if fell_behind { input_xrun.bump(); }
    });
};
```

And the output callback:

```rust
move |data: &mut [f32], _| {
    assert_no_alloc_audio(|| {
        // drain ring
        for sample in data.iter_mut() {
            *sample = consumer.pop().unwrap_or_else(|_| { fell_behind = true; 0.0 });
        }
        if !bypass.value() {
            deinterleave_channel(data, 0, channels, &mut ch0_scratch);
            latency_meter.process(&mut ch0_scratch[..n_frames]);
            interleave_channel(data, 0, channels, &ch0_scratch);
            gain_block.process(data);
            output_gain.apply_to_buffer(data, channels);
        }
        if fell_behind { output_xrun.bump(); }
    });
};
```

The composition root drops from ~540 to ~350 lines, and every extracted
piece gains independent testability.

---

## Implementation Status

| # | Extraction | Status | Commit |
|---|-----------|--------|--------|
| 1 | `assert_no_alloc_audio` → `audio::rt_guard` | ✅ Done | `c0c8a1c` |
| 2 | `err_fn` + `device_label` → `device` | ✅ Done | `60da087` |
| 3 | Ring construction → `audio::ring` | ✅ Done | `c6a41cb` |
| 4 | Deinterleave/interleave → `domain::buffer` | ✅ Done | `b6178e3` |
| 5 | `TestOscillator` → `domain::blocks` | ✅ Done | `77852d2` |
| 6 | Evolve `Gain` for per-frame input | Design decided | — |

Extracts 1–5 landed on `refactor/composition-root-extractions`. Composition
root went from 540 → 482 lines. 11 new unit tests added (6 buffer + 5 oscillator).

---

## Tradeoffs

**Pros**

- Each extracted piece is independently unit-testable — today none of the
  inline DSP in the callbacks can be tested without running real cpal streams.
- The composition root reads as wiring, not implementation — matches the
  intent of [A5](../../standards/architecture.md).
- Future signal-chain blocks (EQ, compressor) follow the same `Process`
  pattern — the callbacks just grow by one `.process()` call.
- `deinterleave_channel` is reusable for any future per-channel processing.

**Cons**

- More files, more indirection. The original 540-line file was greppable and
  self-contained. After extraction, understanding the full signal path
  requires reading 4–5 files. Mitigated by the Excalidraw diagram and the
  module-level doc comments.

---

## References

- [A1 — Domain imports nothing from I/O](../../standards/architecture.md)
- [A2 — No alloc/lock/syscall on the audio thread](../../standards/architecture.md)
- [A5 — One composition root](../../standards/architecture.md)
- [ADR-007 — Composition root location](../../adr/007-composition-root.md)
- [Composition root diagram](../../composition-root.excalidraw)
- [Code review notes](2026-06-13-async-review-changes.md)
