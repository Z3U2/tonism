# Implementation plan: <spec-slug>-NN — <short title>

**Story**: [<spec-slug>-NN-<slug>.md](<spec-slug>-NN-<slug>.md)
**Spec**: [spec.md](../spec.md)
**Layers**: Capture · Signal chain · Render · Tone state · Control surface · Persistence · Tests _(strike out those that do not apply)_
**Complexity**: 🟢 Low · 🟡 Medium · 🔴 High

---

## 1. Summary

<2-3 sentences: what this story implements concretely and the main technical challenge(s). No copy-paste of the story.>

---

## 2. Functional scope

### Mapping acceptance criteria → components

| Criterion / Case               | Layer            | Main component                              | Status |
| ------------------------------ | ---------------- | ------------------------------------------- | ------ |
| <Success AC 1>                 | Domain           | `<DspBlock>` + `<SignalChainPort>`          | 🟢     |
| <Success AC 2>                 | Control surface  | `<Widget>` + `<ParamUpdateChannel>`         | 🟡     |
| <Failure AC — invalid param>   | Domain           | `<DspBlock>` (validation branch)            | 🟢     |
| <Checklist item 1>             | Audio adapter    | `<AudioBackend>` (xrun counter)             | 🟢     |

> Every line of the manual validation checklist must appear here. Every AC must have a matching test in section 5.

### Out of scope (declared)

- <Element seen in the story that is intentionally not delivered here. Why, and where it lands (other story, future PR).>

---

## 3. Domain & data model

> If the story does not change the domain types, write a single line: `No domain change. No new types.` and skip to section 4.

### New / modified types

| Change                                  | Description                                                | Status |
| --------------------------------------- | ---------------------------------------------------------- | ------ |
| 🟢 NewType `XrunCount(u64)`             | Wraps the buffer-underrun counter (rule C5)                | 🟢     |
| 🟢 Enum `BypassState { On, Off }`       | Avoids bool soup for the bypass control (rule E4)          | 🟢     |
| 🟡 Trait `SignalChainPort`              | Add `fn xrun_count(&self) -> XrunCount`                    | 🟡     |

### Errors

| Error                       | Variant of                       | Where translated to user                | Status |
| --------------------------- | -------------------------------- | --------------------------------------- | ------ |
| `AudioBackendError::NoDevice` | Infrastructure error           | GUI status bar, exit code in standalone | 🟢     |
| `ParamRangeError`           | Domain error                     | GUI tooltip / log line                  | 🟢     |

> Domain and infrastructure error taxonomies stay distinct (rule D2). Adapters translate at the boundary.

### Persistence (if relevant)

> If the story does not touch persistence, write `No persistence change.` and move on.

- File / format: <e.g. `presets/<name>.toml` — TOML via `serde`>
- Versioning strategy: <e.g. `version` field, fail-fast at the parser>
- Backfill / migration of existing files: <none / one-shot script / on-load conversion>

---

## 4. Architecture

### 4.1 Domain — pure core

> Rule A1 reminder: zero imports from audio I/O, GUI, plugin host, or filesystem crates in this layer.

#### Components

> Status legend: 🟢 New · 🟡 To modify · ⚪ Reused as-is
> Type legend: 🎯 Domain model · 🧩 Domain function · 🔌 Port (trait) · ⚙️ DSP block · 🚦 State machine

```diff
<feature entry point — domain function or trait method>
├── 🟢 ⚙️ <DspBlock>::process(&mut self, input: &[f32], output: &mut [f32])   crates/<crate>/src/domain/dsp/<block>.rs
+         Pure DSP. No alloc, no panic, deterministic. Operates on caller-provided slices.
│   ├── 🟢 🎯 <ParamSnapshot>                                                 crates/<crate>/src/domain/params/<snapshot>.rs
+              Immutable snapshot consumed by the block per buffer.
│   └── 🟢 🔌 <SignalChainPort>                                                crates/<crate>/src/domain/ports/signal_chain.rs
+              Trait the audio adapter implements; constructor-injected.
└── ⚪ 🎯 <SampleRate>, <BufferSize>                                            crates/<crate>/src/domain/types.rs
+      Reused NewType wrappers (rule C5).
```

#### Domain rules at play

- **Tell, don't ask** (C1): the block exposes `process()`, not internal state for callers to mutate.
- **Immutability by default** (C3): `&self` everywhere except the realtime per-buffer scratch state, which is `&mut self` and clearly marked.
- **Parse, don't validate** (E2): any caller-provided value is parsed into a NewType at the boundary.

---

### 4.2 Audio adapter — realtime shell

> If the story does not touch the audio path, write `No audio adapter change.` and move on.
> Rule A2 reminder: **no allocation, no locking, no syscall on the per-buffer path**.

#### Components

```diff
<audio callback or stream setup>
├── 🟡 📡 <AudioBackend>::run_callback()                                       crates/<crate>/src/adapters/audio/backend.rs:120
+      Add xrun increment when the host signals an underrun.
│   ├── ⚪ 🔌 <SignalChainPort>::process()                                     (domain)
+              Reused; called per buffer.
│   └── 🟢 📊 <XrunCounter> (atomic, wait-free)                                crates/<crate>/src/adapters/audio/xrun.rs
+              `AtomicU64` updated by the audio thread, read by the GUI.
└── ⚪ 🚇 <ParamChannel> (lock-free, GUI → audio)                                crates/<crate>/src/adapters/messaging/...
+      Reused. If the lock-free primitive is still TBD, see "Decisions to validate".
```

#### Realtime constraints checklist

- [ ] No `Vec::push`, `Box::new`, or other heap allocation inside the per-buffer path.
- [ ] No `Mutex`, `RwLock`, or other blocking primitive.
- [ ] No filesystem, network, or `println!` calls.
- [ ] All scratch buffers pre-allocated in the shell on stream start; passed as slices.
- [ ] If logging is needed, it goes through the lock-free queue drained by a non-realtime thread (rule J2).

---

### 4.3 Control surface — GUI / MIDI

> If the story is purely backend (no UI), write `No control-surface change.` and move on.

#### Components

> Status legend: 🟢 / 🟡 / ⚪
> Type legend: 🪟 Window/panel · 🎛️ Widget · 🪝 State binding · 🌍 String / i18n key

```diff
<panel concerned>
├── 🟡 🪟 <MainWindow>                                                          crates/<crate>/src/adapters/gui/main_window.rs
+      Add the new readout / knob / toggle to the layout.
│   ├── 🟢 🎛️ <XrunReadout>                                                    crates/<crate>/src/adapters/gui/widgets/xrun_readout.rs
+          Reads the atomic counter; renders text. No mutation of audio state.
│   └── 🟢 🪝 use_param_binding!(...)                                           crates/<crate>/src/adapters/gui/bindings.rs
+          Wires GUI state → ParamChannel; one-way (rule F4).
└── ⚪ 🚇 <ParamChannel> (GUI side)                                              (shared with audio adapter)
+      Reused.
```

#### Routing / navigation

- 🟢 New panel / route `<name>` registered in `<router_or_app_init>` (or the equivalent for the chosen GUI framework).
- 🟡 Status-bar entry to surface counters / errors.

---

### 4.4 Persistence

> If the story does not touch persistence, write `No persistence change.` and move on.

- 🟢/🟡 Reader/writer pair: `crates/<crate>/src/adapters/persistence/<file>.rs` — parses preset/setting at the boundary, hands a domain type to the core.
- 🟢/🟡 File-format version handling.
- 🟢/🟡 Test fixtures: paths under `tests/fixtures/<area>/`.

---

### 4.5 Composition root

- 🟡 Update the composition root (per rule A5 — single place at startup) to wire the new component(s).
- ⚪ DI shape unchanged otherwise.

---

### 4.6 Key technical decisions

- **<Decision 1>**: <choice taken> — <one-sentence rationale, ideally a reference to a standard, ADR, or sibling story>.
- **<Decision 2>**: …

### 4.7 Justification of deviation from standards (if applicable)

> Section present only if the plan deviates from `docs/standards/**`.

- **Standard concerned**: <file + rule ID>
- **Deviation**: <what we do differently>
- **Rationale**: <why, and measured impact>
- **Resolution path**: <never / when story X lands / tracked in ADR Y>

---

## 5. Tests

### 5.1 e2e (audio path) — _Testing Trophy: thin but real_

> Required only when the story affects the realtime path or end-to-end behaviour. Otherwise: `No e2e impact.`

- 🟢 `<feature>_full_audio_path_no_xrun_5min` — drive a known input through the chain via the e2e harness; assert the xrun counter stays at 0.
- 🟢 `<feature>_param_change_under_load` — toggle bypass / change gain during the run; assert no audible glitch and no crash (via the harness's glitch detector).

### 5.2 Integration (domain + adapter) — _load-bearing layer_

- 🟢 `<feature>_chain_with_in_memory_audio_device` — instantiate the signal chain with a fake (in-memory) audio device, push a buffer, assert the output.
- 🟢 `<feature>_param_channel_round_trip` — write a parameter change from the GUI side, read it on the audio side, assert it lands.
- 🟡 `<existing-test>` — extend with the new field/branch.

### 5.3 Unit (pure domain) — _cheap & fast, boundary-value heavy_

- 🟢 `<DspBlock>::process` — table-driven tests for: zero buffer, single sample, max buffer size, parameter at min/max, NaN/Inf at the input (rule G7).
- 🟢 `<ParamSnapshot>::try_from` — invalid range → `Err(ParamRangeError)`.

### 5.4 AC coverage table

| AC / Checklist item     | Test                                              |
| ----------------------- | ------------------------------------------------- |
| <Success AC 1>          | integration `<feature>_chain_with_in_memory_…`    |
| <Failure AC — bad param>| unit `<ParamSnapshot>::try_from_invalid_range`    |
| <Checklist 1>           | e2e `<feature>_full_audio_path_no_xrun_5min`      |

> No AC and no checklist item should remain without a test.

---

## 6. Dependencies and execution order

- **Prerequisite stories**: <spec-slug-NN> must be delivered first. Why: <reason>.
- **Stories unblocked**: <spec-slug-MM> can start once this lands.
- **Commands to run** (local + CI):
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo bench` (only if performance-budget tests are gated — see [infrastructure.md](../../../standards/infrastructure.md))

---

## 7. Risks and open questions

- 🔴 <Blocking question not resolved by the spec — who decides, options, impact on the plan>.
- 🟡 <Risk to watch — e.g. perf regression on the dev OS audio backend, to benchmark in review>.
- 🟢 <Good news — e.g. no GUI work needed, all reused>.

---

## 8. References

- Similar implementations to follow: [`<existing-block>`](crates/<crate>/src/domain/dsp/...), [`<existing-adapter>`](crates/<crate>/src/adapters/...).
- Directly applicable standards: [architecture.md](../../../standards/architecture.md), [domain.md](../../../standards/domain.md), [testing.md](../../../standards/testing.md), [infrastructure.md](../../../standards/infrastructure.md).
- Related ADRs: [`docs/adr/...`](../../../adr/).
- Product layers and success bars: [product-architecture.md](../../product-architecture.md).
