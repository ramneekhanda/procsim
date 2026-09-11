//! Native-only "Examples" picker. There's no browser/Monaco editor in a
//! native `cargo run` (see `ingest_code::load_native_demo_on_startup`'s doc
//! comment), so there's also nowhere to click an example link the way the
//! SvelteKit frontend's Examples menu works. This is a small `bevy_egui`
//! window (the same UI toolkit `ui::graph_properties_viewer` already uses)
//! listing every example under `web/static/examples/`, embedded at compile
//! time via `include_str!` so the native binary is self-contained and
//! doesn't depend on the process's working directory. Clicking one runs it
//! through the exact same `compile_code()` path the browser uses.

use bevy_egui::{egui, EguiContexts};

use crate::systems::ingest_code::compile_code;

/// (menu label, embedded YAML text) - mirrors `menubar.svelte`'s Examples
/// menu. Kept as one flat list here (native has no room for the browser
/// menu's categories) since this is a developer convenience, not
/// user-facing polish.
const EXAMPLES: &[(&str, &str)] = &[
    ("Node Lifecycle", include_str!("../../web/static/examples/lifecycle.yml")),
    ("Ping Pong", include_str!("../../web/static/examples/pingpong.yml")),
    (
        "Multiclient Ping Pong",
        include_str!("../../web/static/examples/multiclient_pingpong.yml"),
    ),
    ("Load Balancer", include_str!("../../web/static/examples/load_balancer.yml")),
    ("Circuit Breaker", include_str!("../../web/static/examples/circuit_breaker.yml")),
    ("Two-Phase Commit", include_str!("../../web/static/examples/two_phase_commit.yml")),
    ("Leader Election", include_str!("../../web/static/examples/leader_election.yml")),
    ("Service Discovery", include_str!("../../web/static/examples/service_discovery.yml")),
    ("Blockchain (Proof of Work)", include_str!("../../web/static/examples/blockchain.yml")),
    (
        "Blockchain (Network Partition & Fork)",
        include_str!("../../web/static/examples/blockchain_fork.yml"),
    ),
    (
        "Cell Division (runtime spawn)",
        include_str!("../../web/static/examples/cell_division.yml"),
    ),
    (
        "Autoscaling Load Balancer",
        include_str!("../../web/static/examples/autoscaling_lb.yml"),
    ),
    (
        "Random Mesh (200 nodes)",
        include_str!("../../web/static/examples/random_mesh_200.yml"),
    ),
];

/// Same list `load_native_demo_on_startup` seeds `E_CODE` with at `Startup`
/// - kept as a named constant so the two stay in sync by construction rather
/// than by remembering to update both places.
pub const DEFAULT_EXAMPLE: &str = EXAMPLES[5].1; // "Two-Phase Commit"

pub fn examples_picker(mut contexts: EguiContexts) {
    // Starts collapsed, same reasoning as ui::graph_properties_viewer's
    // Graph Properties window - a native window shouldn't open with a panel
    // already covering the canvas.
    egui::Window::new("Examples")
        .collapsible(true)
        .default_open(false)
        .show(contexts.ctx_mut(), |ui| {
            for (label, yaml) in EXAMPLES.iter() {
                if ui.button(*label).clicked() {
                    let result = compile_code(yaml.to_string());
                    if !result.result() {
                        eprintln!("failed to load example {label}: {}", result.error_log());
                    }
                }
            }
        });
}
