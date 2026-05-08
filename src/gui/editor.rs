use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use nih_plug::prelude::Editor;
use vizia_plug::vizia::prelude::*;
use vizia_plug::widgets::*;
use vizia_plug::{ViziaState, ViziaTheming, create_vizia_editor};

use crate::audio::params::TonismParams;
use crate::audio::xrun::XrunCounter;

/// Default window size in logical pixels: width × height.
pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (400, 300))
}

/// How often the xrun label polls the atomic counter (~60 Hz).
const XRUN_POLL_INTERVAL: Duration = Duration::from_millis(16);

/// Build the vizia editor for Tonism.
///
/// Draws four [`ParamSlider`] rows (input_gain, output_gain, bypass,
/// test_signal) plus a live xrun counter label and a static latency placeholder.
///
/// # xrun binding pattern
///
/// The xrun counter is an `Arc<AtomicU64>` updated on the audio thread.
/// A `SyncSignal<u64>` is created inside the editor closure (on the UI thread)
/// and a 60 Hz `Timer` polls the atomic, calling `signal.set(new_value)` when
/// it changes.  A `Memo` on the signal drives the `Label` text.
///
/// This is the stop-condition fallback documented in the Phase 4 spec:
/// a Timer-driven poll rather than a custom `Lens` impl on `Arc<AtomicU64>`,
/// which would require a non-trivial derive or blanket impl.
///
/// # xrun observability note
///
/// The cpal standalone wrapper does NOT forward xrun/underflow events back to
/// `Plugin::process`.  The cpal error callback only calls `unparker.unpark()`
/// to stop the stream (see `wrapper/standalone/backend/cpal.rs`); there is no
/// channel or hook the plugin can register.
/// XXX: xrun events are not observable from Plugin::process; the counter stays
/// at 0 in MVP.  Tracked for future work.
pub fn create(params: Arc<TonismParams>, xrun_counter: XrunCounter) -> Option<Box<dyn Editor>> {
    let state = default_state();

    create_vizia_editor(state, ViziaTheming::Custom, move |cx, _gui_context| {
        // Create a reactive signal on the UI thread.
        // `SyncSignal::new` can be called anywhere; when called on the UI thread
        // (which we are — this closure runs on the GUI thread) the signal is
        // scoped to the current reactive scope.
        let xrun_signal: SyncSignal<u64> = SyncSignal::new(0u64);

        // Clone the Arc so we can move it into the timer closure without
        // moving the whole XrunCounter.
        let xrun_arc = xrun_counter.0.clone();

        // Timer fires at ~60 Hz and updates the signal when the count changes.
        let timer = cx.add_timer(XRUN_POLL_INTERVAL, None, move |_cx, action| {
            if matches!(action, TimerAction::Tick(_)) {
                let current = xrun_arc.load(Ordering::Relaxed);
                xrun_signal.set_if_changed(current);
            }
        });
        cx.start_timer(timer);

        VStack::new(cx, |cx| {
            Label::new(cx, "Input Gain");
            ParamSlider::new(cx, &params.input_gain);

            Label::new(cx, "Output Gain");
            ParamSlider::new(cx, &params.output_gain);

            Label::new(cx, "Bypass");
            ParamSlider::new(cx, &params.bypass);

            Label::new(cx, "Test Signal");
            ParamSlider::new(cx, &params.test_signal);

            // Live xrun counter: Memo re-evaluates each time xrun_signal changes.
            Label::new(
                cx,
                Memo::new(move |_| format!("xrun: {}", xrun_signal.get())),
            );

            // Latency label: static placeholder — algorithm is dev work post-Phase 4.
            Label::new(cx, "latency: -- ms");
        });
    })
}
