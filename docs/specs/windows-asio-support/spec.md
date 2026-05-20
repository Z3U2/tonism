# Spec: Windows ASIO Support (opt-in)

**Status:** Forward-looking — opt-in build path for Windows-only test rigs that require ASIO. Not enabled in the default build, not exercised in CI.
**Last updated:** 2026-05-14

## Context

ASIO is Steinberg's low-latency audio API for Windows. It exists because Windows' original audio stack (MME, DirectSound) added 100+ ms of latency, making it unusable for monitoring or live performance. Steinberg published ASIO as a bypass — drivers expose a COM interface directly to apps, skipping the OS audio engine.

The competitive landscape has moved on:

- **WASAPI Exclusive Mode** (Vista+) reaches within 1–3 ms of ASIO on most modern interfaces. Available without any license. Supported by [cpal](https://docs.rs/cpal) out of the box.
- **CoreAudio** (macOS) was always low-latency by design; no ASIO equivalent was ever ported because none was needed.
- **JACK** / **PipeWire** (Linux) cover the same ground without the license.

Despite this, ASIO remains in active use for two practical reasons:

1. **Vendor driver investment** — interfaces like RME, Focusrite, MOTU shipped excellent ASIO drivers for a decade and many users have rigs configured exclusively around them. Switching their everyday DAW to WASAPI means re-tuning a setup they don't want to touch.
2. **Single-process exclusivity** — ASIO drivers are exclusive by design (one app at a time). This is a feature for studios that route everything through one DAW.

This spec documents the path for Tonism to opt into ASIO on a Windows test rig that demands it, **without making ASIO the default** — which would drag the Steinberg license into every contributor's path and into CI.

For motivation, see the conversation captured in [the conversation summary on PR #4 review](https://github.com/Z3U2/tonism/pull/4).

---

## Scope

### In scope

- A Cargo feature `asio` that, when enabled, builds `cpal` with its ASIO backend.
- Build prerequisites a Windows contributor needs (SDK, env vars, LLVM).
- CLI invocation that selects the ASIO backend at runtime.
- Steinberg license obligations that apply to any binary shipped with `asio` enabled.

### Out of scope

- **macOS / Linux ASIO** — does not exist. The feature gate is Windows-only by virtue of `asio-sys`' build script.
- **WASAPI Exclusive Mode** — separate concern, deferred to its own spec. WASAPI Exclusive covers the *latency* motivation for ~95% of use cases without the license; ASIO support is needed only when the test rig itself is ASIO-locked.
- **Bundling the Steinberg SDK** — the license forbids redistribution. Contributors register and download themselves.
- **CI builds with ASIO enabled** — CI stays on WASAPI. Validating ASIO is a local-Windows-only manual check.
- **Distributing ASIO-enabled binaries to end users** — out of MVP scope. When/if v0.2 ships an ASIO-enabled installer, an attribution/about-screen story follows.

---

## Cargo feature design

### `Cargo.toml` change

```toml
[dependencies]
# (existing nih_plug, nih_plug_egui, egui, etc. unchanged)

# Direct cpal dep so the `asio` feature can be toggled here.
# Version must match the cpal that nih_plug transitively pulls in
# (verify with `cargo tree -i cpal`).  Cargo's feature unification
# makes this the same cpal instance nih_plug uses internally.
cpal = { version = "<match-nih_plug>", optional = true }

[features]
default = []
# Enable on Windows test rigs that require ASIO.  Not portable.
asio = ["dep:cpal", "cpal/asio"]
```

Three design choices baked in:

1. **`optional = true` + `dep:cpal`** in the feature — keeps `cpal` out of the dep graph entirely when the feature is off. (Without `optional`, `cargo build` would still resolve cpal as a direct dep even on macOS/Linux where it isn't needed.)
2. **`default = []`** — every CI environment, every contributor's first `cargo build`, the macOS dev loop: all license-clean.
3. **Single feature flag, not a `cfg(target_os = "windows")` gate** — even on Windows we want the *default* build to stay license-clean. Opt-in is the contract.

### Why not enable via nih_plug feature passthrough

`nih_plug` doesn't expose an `asio` feature on its standalone backend. We could PR one upstream (and probably should, eventually), but the direct-cpal-feature approach works today because Cargo feature unification means nih_plug's transitive cpal is built with the same feature set. Less change surface, no upstream PR dependency.

---

## Build prerequisites (Windows test rig only)

### One-time setup

```powershell
# Steinberg ASIO SDK
# Register at https://www.steinberg.net/developers/, download "ASIO SDK 2.3.x".
# Unzip to a stable path, e.g.:
#   C:\dev\asio_sdk\
# Verify the path contains the subfolders: common\, host\, driver\

# LLVM — asio-sys uses bindgen, which needs libclang
winget install LLVM.LLVM

# Env vars (set in System Properties → Environment Variables for persistence,
# or in the current shell for one-off builds):
$env:CPAL_ASIO_DIR = "C:\dev\asio_sdk"
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

### Build

```powershell
cargo build --release --features asio
```

A clean ASIO-enabled build takes ~3–5 minutes longer than the default release build because `asio-sys` invokes `bindgen` over the SDK headers.

---

## Runtime invocation

```powershell
.\target\release\tonism.exe `
  --backend asio `
  --input-device "Focusrite USB ASIO" `
  --output-device "Focusrite USB ASIO" `
  --sample-rate 48000 `
  --period-size 256
```

Device names match what each ASIO driver registers under `HKLM:\SOFTWARE\ASIO\<name>`. List with:

```powershell
Get-ChildItem 'HKLM:\SOFTWARE\ASIO' | Select-Object PSChildName
```

`AudioDeviceCmdlets` and `Get-PnpDevice -Class AudioEndpoint` enumerate the MMDevice tree, which is WASAPI/WDM only — ASIO is invisible to them.

---

## License obligations

The Steinberg ASIO SDK is free in money but restrictive in distribution. The core obligations:

| # | Obligation | Practical impact for Tonism |
| --- | --- | --- |
| 1 | No redistribution of the SDK | Do not commit `C:\dev\asio_sdk\` (or any subset) into the repo. `.gitignore` should already cover stray local paths. |
| 2 | Compiled binaries are fine | A `tonism.exe` built with `--features asio` can be distributed without per-unit royalties. |
| 3 | Attribution required | Any distributed binary must display *"ASIO is a trademark and software of Steinberg Media Technologies GmbH"* somewhere user-visible — About dialog, README, or splash. Deferred until v0.2 ships an installer. |
| 4 | No reverse engineering | Of the SDK or any ASIO driver. |
| 5 | No trademark use beyond attribution | Cannot brand Tonism as "ASIO-Compatible™" etc. |

The license is not OSI-approved. Source-license compatibility:

- **MIT/Apache/BSD** repo (Tonism's anticipated direction): compatible. The redistribution restriction binds the *SDK*, not source that links against it.
- **GPL**: incompatible — well-known pain point for projects like Audacity. Tonism is not GPL.

A contributor enabling `--features asio` must accept the Steinberg license themselves at SDK-download time. Tonism's repo does not put them in legal jeopardy as long as it doesn't redistribute the SDK files.

**Read the actual license text on the download page before clicking accept** — this spec summarizes from the long-stable 2.3.x version, but Steinberg can update it.

---

## ASIO-specific gotchas

| Symptom | Cause | Fix |
| --- | --- | --- |
| `BuildStreamError::DeviceNotAvailable` | Another app already opened the ASIO driver | Close the other app. ASIO is single-process exclusive by design. |
| Stream starts then errors immediately | Sample rate mismatch with the driver's control-panel setting | Pass `--sample-rate <hz>` matching what the driver is configured for. ASIO drivers don't always retune dynamically. |
| Stream config fails on buffer size | ASIO drivers expose a fixed list of buffer sizes | Pass `--period-size` matching one of the driver's offered sizes; or omit and let cpal pick the default. |
| `cargo build` fails: `clang: command not found` or `iasiodrv.h: not found` | `LIBCLANG_PATH` not set, or `CPAL_ASIO_DIR` pointing one level off | Verify env vars resolve to existing files. `CPAL_ASIO_DIR` should contain `common\`, `host\`, `driver\`. |
| Multiple `cpal` versions in `cargo tree -i cpal` | Cargo couldn't unify versions across direct + transitive deps | Pin the direct `cpal` version to match what `nih_plug` pulls in. |

---

## Tradeoffs

### Pros

- Unblocks Windows test rigs that are ASIO-only by configuration (real constraint for users with established setups around RME/Focusrite/MOTU drivers).
- Default build path stays license-clean — opt-in is explicit.
- Cargo feature unification keeps the change small: one direct-dep addition, no upstream PR required.

### Cons

- **Contributor friction** on Windows: registering with Steinberg, installing LLVM, setting env vars. Documented but not zero.
- **CI cannot validate** ASIO behavior. Only local manual verification on a real Windows + ASIO rig confirms it works.
- **Attribution obligation** transfers to anyone shipping a binary. Acceptable but adds a v0.2 packaging-story item.
- **License risk if Tonism's source license ever shifts to GPL.** Not currently a concern.

### Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| `cpal` direct-dep version drifts from `nih_plug`'s transitive version, causing two cpal instances in the build | Document the `cargo tree -i cpal` check; pin explicitly in `Cargo.toml`. Adding a `cargo deny` rule against duplicate cpal versions would catch it in CI. |
| Steinberg updates the license restrictively | Re-read on every SDK update. The license has been stable for years; risk is low but non-zero. |
| `asio-sys` bindgen breaks on a new SDK version | Pin a known-good SDK version in this spec (currently 2.3.3). Older or newer versions may need manual workarounds. |
| Contributor accidentally commits the SDK | `.gitignore` rules + a pre-commit reading-review checklist. |

---

## Alternatives considered

### A. WASAPI Exclusive Mode (preferred default)

| | WASAPI Shared | WASAPI Exclusive | ASIO |
| --- | --- | --- | --- |
| License | none | none | Steinberg |
| Through Win audio engine | yes (+20 ms) | bypasses | bypasses |
| Typical round-trip | 20–40 ms | 3–8 ms | 2–6 ms |
| cpal support | yes (default) | yes (configurable) | feature-gated |

WASAPI Exclusive covers the *latency* motivation for ~95% of use cases. **Should be done before this spec is implemented**, as a separate tech-quality story. ASIO becomes a niche add-on after WASAPI Exclusive lands.

### B. JACK on Windows

Cross-platform pro-audio server, available on Windows via a separate installer. `cpal` has a `jack` feature flag.

- Pros: license-clean, cross-platform, very low latency.
- Cons: end users have to install JACK separately; adds a daemon to manage; tooling around it is more niche than ASIO on Windows.

Reasonable for cross-platform pro-audio setups but doesn't solve the "my existing rig is ASIO-locked and I won't reconfigure it" problem.

### C. Defer entirely

The minimum viable answer: tell users with ASIO-only rigs to either (a) configure WASAPI temporarily on their test box, or (b) accept that Tonism doesn't run on their setup for now.

Rejected because (a) is exactly the "don't make me reconfigure my rig" friction we're trying to avoid for the project owner who needs to test on their own Windows machine, and (b) blocks meaningful real-hardware verification on a platform we already claim to support.

---

## When to adopt

**Not for MVP.** AC1–AC4 verification ships on macOS (the dev machine) and Linux CI. No MVP acceptance criterion requires ASIO.

**Adopt when** the project owner (or any contributor) needs to verify Tonism on a Windows rig whose audio setup is configured exclusively around ASIO drivers and reconfiguring it for WASAPI is not acceptable. This is exactly the trigger that motivated this spec.

**Defer if** WASAPI Exclusive Mode lands first (see [Alternative A](#a-wasapi-exclusive-mode-preferred-default)) and the Windows rig owner is willing to retest under WASAPI Exclusive. In many cases the latency difference is < 2 ms and the license simplification is worth it.

---

## Implementation footprint

When this spec is actioned, the change is:

1. **`Cargo.toml`** — add the optional `cpal` direct dep and the `asio` feature gate (~5 lines).
2. **`README.md`** — a short section under "Building" explaining the Windows opt-in path. Link back to this spec.
3. **`.gitignore`** — ensure no SDK paths get committed (existing `.gitignore` already covers absolute paths since they're not relative).
4. **No source changes in `src/`** — the feature flag flows entirely through Cargo's dependency graph into cpal's backend selection. `nih_plug`'s standalone backend will see ASIO as an available cpal host because feature unification enabled it transitively.

No story has been carved for this yet; it lives here as a build-path spec until a concrete trigger materializes.

---

## References

- [Steinberg ASIO SDK download](https://www.steinberg.net/developers/) (requires registration).
- [`cpal` documentation — host selection](https://docs.rs/cpal/latest/cpal/).
- [`asio-sys`](https://docs.rs/asio-sys/) — the bindgen-driven Rust binding to the ASIO SDK headers; sits underneath `cpal`'s ASIO backend.
- [ADR-002 — Standalone runner choice](../../adr/002-standalone-runner.md) — covers cpal's role under `nih_plug_standalone`.
- [ADR-004 — GUI library: egui](../../adr/004-gui-library-egui.md) — context for the broader Windows/macOS/Linux support matrix.
- [Tonism MVP spec](../mvp/spec.md) — AC4 manual verification protocol; ASIO is not in its critical path.
