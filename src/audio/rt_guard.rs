//! A2 enforcement: cfg-gated wrapper that asserts zero heap allocation on the audio thread.

/// Wrap an audio-thread closure in [`assert_no_alloc::assert_no_alloc`]
/// when the `debug-assert-no-alloc` feature is on; pass through
/// otherwise. The no-op version compiles to a direct call so there is
/// no overhead in release builds.
///
/// Pairs with the `#[global_allocator]` declaration in each binary
/// (`src/main.rs`, `src/bin/feedback.rs`) which is also cfg-gated.
#[cfg(feature = "debug-assert-no-alloc")]
#[inline]
pub fn assert_no_alloc_audio<F: FnOnce() -> R, R>(f: F) -> R {
    assert_no_alloc::assert_no_alloc(f)
}

#[cfg(not(feature = "debug-assert-no-alloc"))]
#[inline(always)]
pub fn assert_no_alloc_audio<F: FnOnce() -> R, R>(f: F) -> R {
    f()
}
