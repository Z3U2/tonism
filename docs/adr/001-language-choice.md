# ADR-001: Language, UI Framework & Audio Library Selection

**Status:** Proposed  
**Date:** 2026-05-06  
**Deciders:** [TBD]  
**Context:** Selecting a technology stack for a desktop audio application similar to Tonocracy (guitar amp simulation, real-time DSP, effects chain, plugin support, cloud library)

---

## Context and Problem Statement

We are building a cross-platform desktop application for real-time guitar amp simulation and effects processing, comparable to Tonocracy by Atomic Amplifiers. The application requires:

- **Real-time audio I/O** with low latency (< 10ms round-trip)
- **DSP processing pipeline** — signal chain with amp models, effects, cab IRs
- **Custom GUI** — knobs, signal flow routing, waveform displays, preset browser
- **Plugin format support** — VST3/CLAP export for DAW integration
- **Cross-platform** — Windows, macOS, Linux
- **Cloud connectivity** — preset/model sharing library

We are evaluating **Go**, **Rust**, and **C#** as the primary language, along with their respective UI and audio ecosystems.

---

## Options Evaluated

### Option A: Rust

| Aspect | Details |
|---|---|
| **Language** | Rust (systems-level, no GC, memory-safe) |
| **UI Library** | egui, iced, VIZIA, or Slint |
| **Audio Library** | cpal + custom DSP, or nih-plug (plugin framework) |
| **Plugin Framework** | nih-plug (VST3 + CLAP) |

#### UI Libraries

**egui** — Immediate-mode GUI, ~28k GitHub stars, maintained primarily by Emil Ernerfeldt (sponsored by Rerun). Very active development, latest release uses skrifa+vello_cpu for sharp text rendering. Excellent for prototyping and tools. Already integrated with nih-plug for audio plugin UIs. Limitations: basic theming, non-native look, complex layouts require workarounds. Immediate mode can be CPU-hungry with many widgets.

**iced** — Elm-architecture retained-mode GUI, ~27k GitHub stars, maintained by Héctor Ramón. Uses wgpu for rendering. Well-structured for complex apps. Has an `iced_audio` extension with knobs, sliders, and XY pads purpose-built for audio applications. Also integrated with nih-plug. Limitations: accessibility support still incomplete (open issue for 4+ years), steeper learning curve than egui, some documentation gaps.

**VIZIA** — Declarative GUI framework specifically popular in the audio plugin space. Used by multiple nih-plug projects for plugin UIs. Includes audio-specific widgets. Smaller community than egui/iced but purpose-built for the audio use case. Has nih-plug adapter with visualizer widgets.

**Slint** — Commercial/dual-licensed (free for desktop, commercial for embedded). DSL-based declarative UI with live preview. ~18k GitHub stars, backed by a company (SixtyFPS GmbH). Good accessibility support, C++/JS/Python bindings available. Limitation: AGPL or commercial license for some backends; no existing nih-plug integration.

#### Audio Libraries

**cpal** — Cross-platform audio I/O in pure Rust. ~3.7k GitHub stars, part of the RustAudio organization. Supports ALSA, PulseAudio, PipeWire, JACK, WASAPI, CoreAudio, ASIO, and WebAudio. Real-time priority thread support. Active maintenance (updated April 2026).

**rodio** — Higher-level playback library built on cpal. ~2.3k stars, 5.3M total downloads. Good for playback/decoding, but not designed for real-time DSP processing chains.

**nih-plug** — Full plugin framework for VST3 and CLAP. ~4.5k GitHub stars, maintained by Robbert van der Helm. Includes parameter smoothing, sample-accurate automation, standalone mode with JACK support, and built-in GUI adapters for egui, iced, and VIZIA. ISC licensed (but VST3 bindings are GPLv3). This is the de facto standard for Rust audio plugin development and is actively used for production plugins.

**dasp** — DSP primitives library (formerly `sample`). ~1.1k stars. Provides sample type conversion, interpolation, and signal processing fundamentals.

#### Assessment

Rust has the **strongest audio plugin ecosystem** among the three options. nih-plug is a mature, battle-tested framework that directly solves the plugin format problem. The combination of nih-plug + VIZIA or egui is proven in production audio plugins. cpal provides excellent low-level audio I/O. The language's zero-cost abstractions and lack of GC are ideal for real-time audio where garbage collection pauses would cause audible glitches. The main risk is GUI maturity — Rust GUIs are functional but less polished than C#/XAML, and complex custom UIs (like Tonocracy's signal flow editor) require more manual work.

---

### Option B: C# (.NET)

| Aspect | Details |
|---|---|
| **Language** | C# on .NET 9/10 (managed, GC) |
| **UI Library** | Avalonia UI or .NET MAUI |
| **Audio Library** | NAudio (Windows-focused), NWaves (DSP), or SDL2/PortAudio bindings |
| **Plugin Framework** | AudioPlugSharp, VST.NET, or SharpSoundDevice |

#### UI Libraries

**Avalonia UI** — Cross-platform XAML/C# framework, ~26k GitHub stars. MIT licensed, commercially backed by AvaloniaUI Ltd. Used in production by JetBrains, Unity, Autodesk, and GitHub. Renders via Skia (pixel-identical across platforms). 70+ built-in controls, MVVM support, hot reload. Version 12.0 delivers up to 1,867% FPS improvement on complex layouts. Excellent IDE support (VS, Rider, VS Code). Very mature for complex desktop UIs — data grids, panels, docking, custom controls all well-supported.

**.NET MAUI** — Microsoft's official cross-platform framework. Wraps native controls. Poor macOS performance (uses Mac Catalyst, not native AppKit). No Linux support. Not recommended for desktop-heavy audio applications.

#### Audio Libraries

**NAudio** — The main .NET audio library, ~5.6k GitHub stars, maintained by Mark Heath since 2002. Provides WaveOut, WASAPI, ASIO output, mixing engine, format conversion, and basic effects. Comprehensive but **Windows-only** for most features. WASAPI and ASIO support is Windows-specific. No native macOS/Linux audio backend.

**NWaves** — .NET DSP library with filters, FFT, spectral analysis, and effects. Good for the DSP processing side but not a complete audio I/O solution.

**CSCore** — Advanced audio library with real-time processing, visualization, and effects. Also primarily Windows-focused.

**Cross-platform gap:** Avalonia users report needing to use LibVLC, SDL2 bindings, or PortAudio bindings for cross-platform audio. There is no single, well-maintained .NET audio library that covers Windows + macOS + Linux audio I/O natively.

#### Plugin Frameworks

**AudioPlugSharp** — C++/CLI bridge for VST3 plugins in C#. Windows-only (requires C++/CLI which is a Windows technology). WPF UI support only.

**VST.NET** — VST 2.x plugin development. Mature but limited to the legacy VST2 API. No CLAP support.

**SharpSoundDevice** — Another VST2 bridge. Requires the deprecated VST 2.4 SDK.

All C# plugin frameworks are **Windows-only** and limited to **VST2 or VST3** (no CLAP support). This is a significant limitation for cross-platform audio plugin development.

#### Assessment

C# has the **best GUI story** by far — Avalonia is mature, well-documented, and excellent for complex desktop UIs with XAML, data binding, and MVVM. However, the audio ecosystem has critical gaps: NAudio is Windows-centric, cross-platform audio I/O requires cobbling together bindings, and all plugin frameworks are Windows-only. The .NET garbage collector is also a concern for real-time audio — GC pauses can cause audible glitches, and while .NET's GC has improved dramatically, it's not deterministic. Building a Tonocracy-like app in C# would mean fighting the platform for audio I/O on macOS/Linux and likely giving up cross-platform plugin export.

---

### Option C: Go

| Aspect | Details |
|---|---|
| **Language** | Go (GC, goroutines, simple syntax) |
| **UI Library** | Fyne, Gio, or Wails (webview) |
| **Audio Library** | Beep/Oto, PortAudio bindings |
| **Plugin Framework** | None |

#### UI Libraries

**Fyne** — ~25k GitHub stars, material-design-inspired. Requires CGo. Criticized for not following platform HIG guidelines, poor desktop layout capabilities, and limited file path handling. Better suited for mobile than desktop. Not suitable for a complex audio application with custom knob widgets and signal flow editors.

**Gio** — Immediate-mode, ~1.5k stars. More promising than Fyne for desktop but requires significant boilerplate. Small community. No audio-specific widgets exist.

**Wails** — ~27k GitHub stars. Webview-based (HTML/CSS/JS frontend with Go backend). Could work, but adds the weight of a web runtime and makes custom audio widgets harder. Essentially Electron-lite.

#### Audio Libraries

**Beep** — ~2.1k GitHub stars, uses Oto under the hood. High-level playback with compositors and effects. Small codebase (~1K LOC core). Suitable for playback apps but not designed for low-latency real-time DSP chains.

**Oto** — Low-level cross-platform audio output. Simple but output-only — no audio input support, which is critical for an amp simulator that needs to capture guitar input.

**PortAudio bindings** — Go bindings exist (~400 stars) but are thin wrappers over the C library. Audio callbacks run in C context (not Go goroutines), and Go's GC can cause latency spikes in the audio thread. The Go runtime's goroutine scheduler is not designed for real-time audio constraints.

#### Plugin Frameworks

**None exist.** There is no Go framework for building VST3 or CLAP plugins. The Go runtime (with its GC and goroutine scheduler) makes it fundamentally challenging to meet the real-time requirements of audio plugins loaded into DAW hosts.

#### Assessment

Go is **not suitable** for this project. The language was designed for networked services and CLI tools, not real-time audio applications. The GC is non-deterministic and will cause audible glitches. There are no plugin frameworks. Audio input support is limited. GUI frameworks are immature for complex desktop applications with custom audio widgets. The only viable path would be Wails (web UI) + PortAudio (C bindings), which loses most of Go's advantages and adds significant complexity.

---

## Decision Matrix

| Criterion | Weight | Rust | C# | Go |
|---|---|---|---|---|
| Real-time audio safety (no GC pauses) | 25% | ★★★★★ | ★★☆☆☆ | ★☆☆☆☆ |
| Audio I/O library maturity | 15% | ★★★★☆ | ★★★☆☆ (Win) / ★★☆☆☆ (xplat) | ★★☆☆☆ |
| Plugin format support (VST3/CLAP) | 20% | ★★★★★ | ★★☆☆☆ (Win only) | ☆☆☆☆☆ |
| GUI framework maturity | 15% | ★★★☆☆ | ★★★★★ | ★★☆☆☆ |
| Cross-platform support | 10% | ★★★★★ | ★★★★☆ | ★★★☆☆ |
| Community & ecosystem for audio | 10% | ★★★★☆ | ★★★☆☆ | ★☆☆☆☆ |
| Developer productivity | 5% | ★★★☆☆ | ★★★★★ | ★★★★☆ |
| **Weighted Score** | | **4.25** | **3.05** | **1.35** |

---

## Decision

**Chosen option: Rust** with the following stack:

| Layer | Choice | Rationale |
|---|---|---|
| **Language** | Rust | No GC, memory-safe, zero-cost abstractions, ideal for real-time audio |
| **Plugin Framework** | nih-plug | Production-proven VST3 + CLAP framework, standalone mode, parameter system |
| **Audio I/O** | cpal (via nih-plug's CPAL backend for standalone) | Cross-platform, ASIO/JACK/WASAPI/CoreAudio support |
| **DSP** | Custom + dasp | DSP primitives from dasp, custom amp modeling and effects |
| **GUI (plugin)** | VIZIA or egui (via nih-plug adapters) | Audio-specific widgets, proven in plugin context |
| **GUI (standalone)** | iced or egui (via nih-plug standalone) | More room for complex UI in standalone mode |

### Recommended Architecture

```
┌─────────────────────────────────────┐
│           nih-plug core             │
│  (Plugin trait + Parameter system)  │
├──────────┬──────────────────────────┤
│  DSP     │  GUI Layer               │
│  Engine  │  (VIZIA / egui / iced)   │
│          │                          │
│  • Amp   │  • Signal flow editor    │
│  • FX    │  • Preset browser        │
│  • IR    │  • Knobs & meters        │
│  • NAM   │  • Cloud library         │
├──────────┴──────────────────────────┤
│        Audio I/O (cpal)             │
│  WASAPI│CoreAudio│ALSA│JACK│ASIO    │
└─────────────────────────────────────┘
│                                     │
├─→ VST3 bundle (nih_export_vst3!)    │
├─→ CLAP bundle (nih_export_clap!)    │
└─→ Standalone (nih_export_standalone) │
```

---

## Consequences

### Positive

- **Single codebase** produces VST3, CLAP, and standalone binaries
- **No GC pauses** — deterministic real-time audio performance
- **Active ecosystem** — nih-plug, cpal, and RustAudio org are all actively maintained
- **Memory safety** without runtime overhead
- **Cross-platform** from day one (Windows, macOS, Linux)

### Negative

- **Steeper learning curve** than C# — Rust's ownership model has a ramp-up period
- **GUI limitations** — Rust GUIs are less polished than Avalonia/XAML. Building a complex signal flow editor will require more custom widget work
- **Smaller talent pool** — fewer developers know Rust than C#
- **Compile times** — Rust compile times are longer, though incremental builds help

### Risks and Mitigations

| Risk | Mitigation |
|---|---|
| GUI too limited for complex UI | Start with `nih_plug_egui` (per [ADR-004](004-gui-library-egui.md)); fallback to webview-based UI if needed |
| nih-plug maintainer burnout | Framework is open source (ISC); could fork. Also consider Clack as backup |
| VST3 GPLv3 licensing constraint | Use CLAP as primary format (MIT licensed); or create custom VST3 bindings |
| Team unfamiliarity with Rust | Budget 2–4 weeks ramp-up; leverage nih-plug examples and cookiecutter template |

---

## Alternatives Considered but Rejected

**C++ with JUCE** — The industry standard, but not one of the candidate languages. Mentioned for context: JUCE dominates audio plugin development but has a dual GPL/commercial license and C++ memory safety concerns.

**Hybrid: Rust DSP + C# GUI** — Theoretically possible via FFI, but adds significant complexity in the interop layer, especially for real-time parameter communication between the GUI and audio threads. Not recommended unless the GUI requirements become extreme.

---

## References

- [nih-plug](https://github.com/robbert-vdh/nih-plug) — Rust VST3/CLAP framework
- [cpal](https://github.com/RustAudio/cpal) — Cross-platform audio I/O
- [egui](https://github.com/emilk/egui) — Immediate mode GUI (~28k stars)
- [iced](https://github.com/iced-rs/iced) — Elm-architecture GUI (~27k stars)
- [VIZIA](https://github.com/vizia/vizia) — Declarative GUI for audio
- [Avalonia UI](https://github.com/AvaloniaUI/Avalonia) — C# cross-platform GUI (~26k stars)
- [NAudio](https://github.com/naudio/NAudio) — .NET audio library (~5.6k stars)
- [Fyne](https://github.com/fyne-io/fyne) — Go GUI toolkit (~25k stars)
- [Tonocracy](https://tonocracy.com/) — Reference application
- [2025 Survey of Rust GUI Libraries](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html)
- [Tritium: Rust GUI Observations (Feb 2026)](https://tritium.legal/blog/desktop)