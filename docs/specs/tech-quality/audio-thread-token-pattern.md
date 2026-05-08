# Spec: Audio-Thread Token Pattern

**Status:** Forward-looking — not scheduled for MVP. Target: v0.2.
**Last updated:** 2026-05-08

## Context

Rust's borrow checker cannot express "this value may only be touched from one
specific OS thread." The audio callback has exactly this constraint: certain
types hold non-`Sync` state that must never cross thread boundaries. Today the
invariant is informal — a `// SAFETY` comment in `src/audio/log_bridge.rs`
stating "only the audio thread calls this." As the codebase grows, informal
comments scale poorly. The token pattern upgrades the invariant to a type-level
guarantee enforced at every call site.

---

## The Pattern

Define a zero-sized, non-`Send`, non-`Sync` marker type:

```rust
// Sketch only — not for compilation in this commit.

/// Proof that the current execution context is the audio thread.
/// Constructed once during audio-backend setup; never leaves the callback closure.
pub struct AudioThreadToken {
    _not_send: std::marker::PhantomData<*const ()>,
}

// SAFETY: constructed once on the audio thread and moved into the callback
// closure. PhantomData<*const ()> makes AudioThreadToken: !Send + !Sync.
impl AudioThreadToken {
    pub(crate) unsafe fn new() -> Self {
        AudioThreadToken { _not_send: std::marker::PhantomData }
    }
}
```

The token is constructed once during audio-backend setup and moved into the
callback closure; `*const ()` makes it `!Send`, so it cannot escape. Audio-side
methods that must run only on the audio thread accept it as an explicit
parameter:

```rust
// Before (current MVP — unsafe impl Send + UnsafeCell):
pub fn log(&self, event: AudioLogEvent) { ... }

// With token pattern:
pub fn log(&self, _token: &AudioThreadToken, event: AudioLogEvent) { ... }
```

Callers without a `&AudioThreadToken` cannot invoke these methods; no `unsafe`
block is required inside `log_bridge`.

---

## What It Would Let Us Do

**Drop `UnsafeCell` + `unsafe impl Send` from `AudioLogger`** while keeping
`&self` ergonomics: the borrow checker enforces thread-confinement through the
token rather than through exclusive access.

**Uniform treatment of lock-free primitives.** v0.2's multi-block signal chain
will add parameter-snapshot readers, coefficient buffers, and scratch pads —
all holding non-`Sync` state. One token, one pattern; no new `unsafe impl Send`
per type.

**Make [A2](../../standards/architecture.md) more enforceable.** A function
without `&AudioThreadToken` is statically safe to call from any thread. A
function that takes the token is marked for A2 review at the signature level,
not buried in a comment.

---

## Tradeoffs

**Pros**

- Zero runtime cost: `PhantomData` is a ZST — no size, no indirection.
- The invariant is type-level; it cannot be silenced by forgetting to update a comment.
- Future-proof for the v0.2 multi-block signal chain: one token, N types, one
  pattern to teach new contributors.

**Cons**

- Every audio-side method gains a `_token: &AudioThreadToken` parameter;
  threading it through deep call stacks adds noise.
- Types constructed off the audio thread and then moved onto it still need
  `unsafe impl Send` with a SAFETY comment — the token removes the operational
  unsafety, not the construction-time `unsafe`.
- Adds a new concept to the codebase; the current `&mut self` fix is simpler.

**Comparison to the current MVP `&mut self` fix**

`Plugin::process` already holds `&mut self`, giving exclusive access. For one
logger that is sufficient and obvious. The token pays off when the audio side
reaches three or more independently-confined types.

---

## When to Adopt

**Not now.** MVP has one audio-side type; `&mut self` resolves the soundness
concern at zero architectural cost.

**v0.2 trigger.** When the multi-block signal chain lands (see
[product-architecture.md — product layers](../product-architecture.md#product-layers))
and three or more audio-side types each need thread-confinement, introduce
`AudioThreadToken` in a single PR. A fourth `// SAFETY: only called from the
audio thread` comment is the concrete signal to act.

---

## References

- PR #1 thread T11 discussion:
  <https://github.com/Z3U2/tonism/pull/1#discussion_r3208953651>
- [docs/standards/architecture.md — A2](../../standards/architecture.md)
- [`PhantomData` and thread safety — Rustonomicon](https://doc.rust-lang.org/nomicon/phantom-data.html)
