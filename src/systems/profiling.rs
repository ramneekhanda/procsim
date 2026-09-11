//! TEMPORARY per-system profiling, added to find the real bottleneck behind
//! a specific FPS target rather than continuing to guess. Not meant to ship
//! - remove this file, its registration in `main.rs`/`systems/mod.rs`, and
//! the timing wrap + `ResMut<ProfilingStats>` param added to each measured
//! system once the diagnosis is done.
//!
//! Bevy's own `Time` resource only updates once per frame (all systems in
//! the same frame see the same value), so it can't measure a single
//! system's own duration - `web_time::Instant` (wraps `performance.now()` on
//! wasm32, `std::time::Instant` natively) is used instead for real
//! sub-frame wall-clock timing.

use bevy::prelude::*;

#[derive(Resource)]
pub struct ProfilingStats {
    pub rhai_ms: f64,
    pub update_connectors_ms: f64,
    pub connector_style_ms: f64,
    pub message_path_ms: f64,
    pub overlays_ms: f64,
    pub frames: u32,
    // TEMPORARY - wall-clock start of the current reporting window, for a
    // real FPS number alongside the per-system breakdown (mirrors the
    // browser-side requestAnimationFrame counting used for wasm builds -
    // this is the native equivalent, since there's no rAF/JS to sample
    // there).
    pub window_start: web_time::Instant,
}

impl Default for ProfilingStats {
    fn default() -> Self {
        Self {
            rhai_ms: 0.0,
            update_connectors_ms: 0.0,
            connector_style_ms: 0.0,
            message_path_ms: 0.0,
            overlays_ms: 0.0,
            frames: 0,
            window_start: web_time::Instant::now(),
        }
    }
}

const REPORT_EVERY: u32 = 60;

pub fn report_profiling_stats(mut stats: ResMut<ProfilingStats>) {
    stats.frames += 1;
    if stats.frames < REPORT_EVERY {
        return;
    }
    let n = stats.frames as f64;
    let sum = stats.rhai_ms
        + stats.update_connectors_ms
        + stats.connector_style_ms
        + stats.message_path_ms
        + stats.overlays_ms;
    let elapsed = stats.window_start.elapsed().as_secs_f64();
    let fps = n / elapsed;
    crate::c_log!(
        "PROFILE avg ms/frame over {} frames ({:.1} fps): rhai={:.3} update_connectors={:.3} connector_style={:.3} message_path={:.3} overlays={:.3} | sum={:.3}",
        stats.frames,
        fps,
        stats.rhai_ms / n,
        stats.update_connectors_ms / n,
        stats.connector_style_ms / n,
        stats.message_path_ms / n,
        stats.overlays_ms / n,
        sum / n,
    );
    *stats = ProfilingStats::default();
}
