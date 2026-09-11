# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`procsim` (Cargo package name `dsa`) is a browser-based visual simulator for distributed
systems / process graphs. A Rust/Bevy (ECS) app compiles to WebAssembly and renders into a
`<canvas>` embedded in a SvelteKit app. Users write a YAML document (edited live in a Monaco
editor with schema-driven autocomplete) that defines a graph of nodes; each node type carries
a Rhai script with `on_init` / `on_timer` / `on_msg` handlers. The engine ticks node timers,
runs the Rhai handlers, and animates messages travelling along the connectors between nodes.

Two halves that must both be understood together:
- `src/` — the Rust/Bevy simulation engine, compiled to `wasm32-unknown-unknown` and exposed to
  JS via `wasm-bindgen`.
- `web/` — a SvelteKit + Monaco + dockview frontend that hosts the compiled wasm module, the
  YAML code editor, and the log/inspector panels.

## Build & run commands

All top-level orchestration goes through `cargo-make` (`Makefile.toml`), which drives both the
Rust→wasm build and the npm/SvelteKit build. Run these from the repo root.

```bash
# one-time
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
cargo install cargo-make

# fast local iteration: dev-profile wasm build + `vite dev` with hot reload
cargo make run-dev

# production-style: release-profile wasm build (cargo-make "production" profile),
# then builds the static site and serves it via `cargo server`
cargo make -p production run

# just build both halves without serving (used in CI)
cargo make -p production build-all

# rust-only checks
cargo build
cargo test
cargo check --target wasm32-unknown-unknown
cargo bench     # microbenchmarks, e.g. benches/rhai_state_roundtrip.rs - native-only,
                 # doesn't touch wasm/bevy, HTML report at target/criterion/report/index.html

# frontend-only checks (from web/)
npm run check        # svelte-check + svelte-kit sync
npm run dev           # vite dev server alone (needs wasm already built once, see below)
npm run build
```

Default (no `-p production`) cargo-make profile is "development": faster/unoptimized wasm
build (`[env.development]` in `Makefile.toml`). Passing `-p production` switches to the
`[env]` defaults: `lto`, `opt-level = 'z'`, single codegen unit, stripped debuginfo — this is
what CI (`.github/workflows/rust.yml`) uses to build and deploy to GitHub Pages.

Key generated artifacts (all under the gitignored `package/`):
- `package/wasm/dsa.js` / `dsa_bg.wasm` — wasm-bindgen output.
- `package/release/` — the final static site (SvelteKit adapter-static output +
  `assets/fonts`), what gets deployed.

`web/src/routes/dsa.js` is a **checked-in symlink** to `../../../package/wasm/dsa.js`. This
means the frontend cannot resolve its wasm import until the Rust side has been built at least
once (`cargo make build-wasm-module` or any of the `run*`/`build-all` tasks) — a bare
`npm run dev` in `web/` on a clean checkout will fail to load the module.

There is no dedicated Rust test suite beyond `cargo test`/`Makefile.toml`'s `test` task
(no `#[test]` functions currently exist in `src/`); correctness is mostly exercised by loading
example graphs through the running app. Example/sample graph YAML lives in `examples/config/`
(loaded from the frontend's "examples" menu, fetched at runtime from `examples/`).
`benches/` (native-only, `criterion`, see `Cargo.toml`'s `[[bench]]` entry) exists
separately for performance, not correctness — currently just
`rhai_state_roundtrip.rs` (see the state round-trip note in the tick-loop section below).

## Architecture

### Graph definition & YAML schema (`src/parser/graphv2.rs`)

This is the single source of truth for the on-disk/YAML data model and is intentionally the
most important file to read first:
- `File { graph_defn: GraphDefinition }` is the YAML root.
- `GraphDefinition` has `node_types` (reusable type definitions, each with an optional Rhai
  `fn` script, `attrs` (a tick interval and an on-canvas `icon` id), and typed `params` —
  Bool/Float/Integer/String/Option), `graph` (the actual node instances as
  `NodeConnection { name, node_type, links }`), `graph_attrs` (colors/title), and `icons`
  (a named `IconDef { id, url }` registry — `attrs.icon` and a `send()` payload's own
  `icon` key both reference an entry here by `id`; `parse_graph2` validates `attrs.icon`
  against this list the same way it validates `graph[].node_type` against `node_types`).
  `attrs.ticks` is a `Ticks` enum (untagged, so plain `ticks: 5` still works): either a
  fixed seconds value, or `{min, max}` resolved to one random value at load time
  (`resolve_ticks_secs`), or `{min, max, jitter: true}` which re-resolves to a fresh
  random value every time the timer fires (handled in `rhai_engine.rs`'s tick loop via
  `Timer::set_duration`, safe to call because `Timer::tick()` only reads whatever
  duration is currently set, verified against `bevy_time`'s source).
- `parse_graph2()` deserializes the YAML, **compiles each node type's Rhai script into an
  `AST`** (`compile_ast`), then instantiates a `Node` per graph entry (looking up its
  `NodeType`, creating a per-node Rhai `Scope` and a `Timer` from `attrs.ticks`), and runs
  `on_init` once per node via `init_scope` to seed each node's persistent `state`
  (`Dynamic`, stored under the `globals` scope variable).
- `schemars` derives (`JsonSchema` on nearly everything) are what generate the JSON Schema
  served to Monaco (`get_code_schema` in `src/systems/ingest_code.rs`) for live YAML
  autocomplete/validation in the editor — changing these structs changes the editor's schema.

### Runtime tick loop (`src/systems/rhai_engine.rs`)

Every frame, `execute_rhai_engine`:
1. Ticks each node's `Timer`; when it fires and the node has a compiled script, calls
   `on_timer(context)` with the node's params pushed into scope and its `state` bound as
   `globals`.
2. Drains each connector's `msg_delivered` queue and calls `on_msg(msg)` on the receiving
   node for each delivered message.
3. Rhai scripts call the registered host functions `log(s)` (→ `stdlib::rhai_lib::rhai_log` →
   `log_dsa_event!`), `send(to, msg)` (pushes into a thread-local message store), and
   `random_chance(percent)` (returns `true` with roughly that % probability, for scripts
   simulating flaky/failing behavior without a manually-toggled param), and
   `draw(shapes)` (see below) to talk back to the engine; `send_messages` fans sent
   messages out to the `Messages` component on whichever `NodeConnector` links the two
   node names, becoming an in-flight `Message` with a 3s timer.
4. `state` (the Rhai `globals` map) round-trips through the node's persistent `Dynamic` field
   across calls — this is how a node keeps memory between ticks/messages. Both call sites
   (`on_timer` and `on_msg`) move `state` into/out of the `Scope` rather than cloning it -
   `scope.push_dynamic("globals", std::mem::take(&mut node.state))` going in,
   `node.state = scope.remove::<Dynamic>("globals").unwrap_or_default()` coming out. This
   matters because `Dynamic::clone` is a *deep* clone (it recurses into every
   string/array/map a node's state contains), while a move is O(1) regardless of what's
   inside - `benches/rhai_state_roundtrip.rs` (`cargo bench`) measures the old clone-based
   path against this one side by side and shows the move-based round-trip alone running
   3.6-4.7x faster (more for bigger state), a 26-39% reduction in total per-tick call cost.
   Safe to do because the entry doesn't need to survive the call either way - the next line,
   `scope.rewind(init_size)`, discards it regardless.
5. The Rhai compile/eval engines (`parse_graph2` and `initialize_engine`) both raise
   `set_max_expr_depths` to 256 — the default in-function limit of 32 is too low for
   realistic handlers (e.g. a map literal containing an inline `if`).

### Custom node overlays (`draw()` — `src/parser/draw.rs`, `src/systems/node_overlay.rs`)

A handler can call `draw([ #{shape:"rect", ...}, #{shape:"text", ...}, ... ])` to paint a
custom overlay on its node. Flow mirrors `send()`: `draw()` writes the shape list into a
per-`execute_rhai_engine` thread-local (`draw_store`), which is drained right after each
handler into `Node::overlay` + `Node::overlay_dirty`. `node_overlay::render_node_overlays`
then despawn-and-respawns that node's `NodeOverlayShape` child entities (lyon shapes +
`Text2d`, parented to the node so they track drag/pan/zoom). `draw()` *replaces* the
overlay; not calling it leaves the last one; `draw([])` clears. Supported shapes: `rect`
(optional `radius`), `circle`, `line`, `polygon`/`polyline`, `text`; coords are node-local,
y-up, `#rrggbb` colors, capped at `MAX_SHAPES_PER_NODE`. Rhai functions can't see
script-level vars, so a helper called from a handler must take what it needs as params.

### Runtime topology changes (`spawn_node()`/`despawn()`/`link()`/`unlink()`)

A handler can grow or reshape the running graph without touching the YAML/editor at
all: `spawn_node(name, node_type, links)` creates a new node instance (its `on_init`
runs immediately, through the live engine, so `draw()`/`log()` in it work); `despawn(name)`
removes any node by name, including the caller; `link(peer)`/`unlink(peer)` add/remove
`peer` from the *calling* node's own `links` (self-scoped by convention, not enforcement).
(Named `spawn_node` rather than `spawn` because Rhai reserves the bare word `spawn`,
presumably for a future async/threading feature - it's a parse error under any name.)

Mechanism: same thread-local-queue-drained-after-the-handler pattern as `send`/`draw`.
`link`/`unlink` apply immediately to the calling `Node` (safe - no reallocation of
`node_instances` involved); `spawn_node`/`despawn` are attributed to the caller and
queued, then applied once at the very end of `execute_rhai_engine`, after every node's
`&mut Node` borrow from `node_map` has been dropped (`node_instances.push`/`retain`
would otherwise invalidate those borrows mid-iteration). Since `links` was pushed as a
Rhai *constant* in `init_scope`, updating it from the host can't use `Scope::set_value`
(panics on a read-only entry) - it uses `Scope::remove` + `push_constant` instead;
`remove` doesn't check access mode because that check lives in the interpreter's eval
path (assignment/non-pure-method-call sites), not in the `Scope` struct itself.

Rendering reacts via two new events distinct from the YAML-edit `GraphChange` (which
still does a full despawn/respawn of every node - deliberately, since a YAML edit may
have changed anything and there's no state worth preserving across it): `NodeAdded`/
`NodeRemoved` (`resources/graph_def.rs`) tell `node_system::create_nodes` to spawn/despawn
only that one entity, leaving every other node's position, drag state and overlay
untouched. `update_connectors` was rewritten from "despawn all connectors whenever any
node is added" to a real add/remove diff against the currently-declared edges (now
read from live `node_instances[].links`, not the YAML-shaped `graph_defn.graph`, so
`link()`/`unlink()` only have one copy to update) - this also fixed a pre-existing bug
where adding a single node via a YAML edit destroyed every other connector's in-flight
`Messages` queue. `node_pulse::pulse_on_tick` uses `try_insert` (not `insert`) for the
same reason: a node's last `NodeTicked` (pulse animation) and its own same-tick
`despawn()` race to be the command that lands first when the schedule flushes.
`MAX_NODES` (200) caps a runaway `spawn_node()` loop. See
`web/static/examples/cell_division.yml` for a full spawn/link/unlink/despawn demo.

### Narration bubbles (`explain()` — `src/systems/explain_bubble.rs`, `src/systems/radial_blur.rs`, `src/resources/narration.rs`)

A handler can call `explain(key, text)` to show a one-time, pausing, animated narration
bubble anchored to the calling node — for first-occurrence "here's what's happening" beats
in a simulation (see `web/static/examples/two_phase_commit.yml`). `key` is an
author-chosen dedup id (e.g. `"first-prepare"`): `PendingExplain.seen: HashSet<String>`
means each key only ever shows once per graph load, no matter how many times the handler
calls `explain()` with it again (e.g. on every tx). Same thread-local-queue pattern as
`send`/`draw`: `explain()` pushes an `ExplainEntry { node_name, key, text }` into
`PendingExplain.queue`, drained one at a time by `show_next_explain`.

- `show_next_explain` (runs every frame while idle): pops the queue, calls
  `sim_time.pause()` (`Time<Virtual>::pause()` — freezes node timers, in-flight message
  animation, and any `Time`-driven tween all at once), and spawns the bubble: a body +
  pointer-triangle (aimed at the anchor node, built via `node_overlay::spawn_shape` — the
  same shape-drawing code `draw()` overlays use), word-wrapped text, a decorative Continue
  button, and — layered on top of it — a fully transparent `SpriteBundle` "hit region"
  carrying the actual `On::<Pointer<Click>>::run(dismiss_explain_bubble)` handler. The hit
  region is necessary because `bevy_mod_picking` here is built
  `features = ["backend_sprite"]` only — lyon `Mesh2d` shapes (the button's visual) are
  never pickable in this app, so anything meant to be clicked needs an invisible `Sprite`
  on top of it (`update_connectors.rs`'s connector hover uses the identical trick — see
  that section below).
- Bubble pop-in is a hand-rolled `BubblePop` component + `animate_bubble_pop` system driven
  by `Time<Real>`, **not** `bevy_tweening::Animator` — `Animator` reads the generic `Time`
  resource (which mirrors `Time<Virtual>`), so a tween started the same frame the sim
  pauses would see `delta == 0` forever and never animate. Anything that must keep moving
  while the sim is paused (bubble pop-in, the backdrop, the blur fade below) has to be
  driven by `Time<Real>` explicitly.
- `dismiss_explain_bubble` (the Continue click handler) despawns the bubble and unpauses
  `sim_time` only once `PendingExplain.queue` is empty — otherwise it immediately shows the
  next queued entry, still paused.
- `GraphChange` (any YAML edit) calls `PendingExplain::reset()` (clears `seen` and `queue`)
  and unpauses, so a fresh graph load never starts frozen on a stale bubble.

**Radial blur (`radial_blur.rs`)**: while any bubble is up, a `RadialBlurSettings.intensity`
uniform fades in/out (`animate_radial_blur`, `Time<Real>`-driven, same pause-safety reason
as above) and drives a custom WGSL post-process (`radial_blur.wgsl`, embedded via
`load_internal_asset!` so there's no wasm asset-path to resolve) — a `ViewNode` spliced into
the `Core2d` render graph between `Node2d::Tonemapping` and `Node2d::EndMainPassPostProcessing`,
multi-sampling the already-rendered frame toward its center and averaging (deliberately
color-only, no depth texture — this app's WebGL2 target already can't do real depth-of-field,
see the "Disabling depth of field" log line from `bevy_core_pipeline`).

Because that post-process runs on the whole `Core2d` pass, the bubble would get blurred too
if it were part of the same render — so it isn't. A second camera (`BubbleCamera` in
`main.rs`'s `setup_camera`, marked by `components::camera::BubbleCamera`) composites on top:
`order: 1`, `clear_color: ClearColorConfig::None`, renders only `RenderLayers::layer(1)`, and
carries no `RadialBlurSettings` (the post-process plugin only touches views that have the
component). Every bubble/backdrop entity is tagged `RenderLayers::layer(1)` so only this
camera draws them, crisp, over the (possibly blurred) world. `sync_bubble_camera` copies the
main camera's `Transform`/`OrthographicProjection` onto it every frame, since `bevy_pancam`'s
`PanCam` only drives the entity it's attached to (the main camera) and the bubble camera has
no other way to track pan/zoom.

**Gotcha this already caused once**: any query meant to uniquely match "the" camera by
`With<Camera2d>` alone now matches both cameras and `.single()`/`.single_mut()` on it will
panic — which, since an uncaught wasm panic halts the whole Bevy app loop, looks exactly
like a freeze/deadlock from the user's perspective (this broke node dragging — `drag.rs`'s
projection lookup — the first time `BubbleCamera` was added; fixed by adding
`Without<BubbleCamera>`). Any future second-camera-agnostic query needs the same filter.

### Connectors (`src/systems/update_connectors.rs`, `src/components/node_connector.rs`)

A `NodeConnector` is drawn as a cubic bezier between two node centers, inset so it
visibly stops just outside each icon's edge rather than running into it. The curve bows
*perpendicular* to the A→B line (magnitude proportional to distance, clamped in
`bow_amount`) rather than toward a fixed diagonal offset — a fixed offset could bow the
"wrong" way depending on how two nodes happened to be arranged; deriving it from the line
itself keeps the shape consistent no matter the layout. `connector_geometry`/
`connector_stroke` are shared by both the initial spawn (`generate_line`) and the
per-frame retrace loop so newly-created and moving connectors can't drift out of sync
with each other.

**Hover** works the same way the explain-bubble's Continue button does: a connector's
own lyon `Mesh2d` is never pickable (`bevy_mod_picking` here only hit-tests `Sprite`s), so
each connector gets an invisible `Sprite` child (`ConnectorHitRegion`, sized/rotated to
cover its chord) carrying the real `On::<Pointer<Over>>`/`<Out>` handlers, which reach
back to the connector via `Parent` and set `NodeConnector.hovered`.

**A real z-fighting bug this hit-region tripped over**: `background_grid.rs` spawns one
giant 4000x4000 `Sprite` at `z = -1.0` covering the entire visible world. The hit-region
was originally placed at that same z. `bevy_mod_picking`'s sprite backend sorts candidate
hits by z (descending) and the first opaque hit blocks every tied-or-lower one from being
considered at all — so on that exact tie, the always-covering grid sprite won essentially
every time and no connector hover could ever register, regardless of any other code being
correct. Fixed by moving hit-regions to `HIT_REGION_Z = -0.5` (above the grid, still below
node icons at z = 0, 1, 2, ... so a node always wins over an overlapping connector
hit-region near its edge). Any future full-screen or large sprite needs to stay clear of
this z if it's meant to be "behind everything, but still not block picking."

**Live styling** (`update_connector_style`, blends the user-configured `connection_color`
toward an indigo `ACCENT_COLOR` rather than replacing it, so a custom base color still
shows through):
- **Traffic**: width/accent scale with `Messages.msg_inflight.len()` — a connector
  currently carrying messages reads visibly thicker/brighter than an idle one.
- **Delivery flash**: `NodeConnector.flash` is set to `1.0` in `update_message.rs` when a
  message lands (moves from `msg_inflight` to `msg_delivered`) and decays back to `0` over
  `FLASH_DECAY_SECS`, giving a brief highlight on the edge itself, not just the arriving
  message icon.
- **Selection adjacency**: connectors touching the currently-`SelectedNodeMarker`'d node
  get a persistent highlight bump, computed by checking `conn.id1`/`id2` against the
  selected node's name each frame.

### Ingestion / state machine (`src/systems/ingest_code.rs`, `main.rs`)

- `compile_code` and `get_code_schema` are the two `#[wasm_bindgen]` entry points the SvelteKit
  frontend calls directly (see `web/src/routes/+page.svelte`). `compile_code` just validates +
  stashes the YAML string in a `lazy_static` `Mutex<String>` (`E_CODE`); it does not mutate the
  live ECS world directly (wasm/JS calls happen outside the Bevy schedule).
  `ingest_codechange` (a normal Bevy system) polls that global each frame, and on change
  re-parses it and swaps in the new `GraphDefinitionRes`, flipping `LoadingState` to `Loading`.
- `LoadingState` (`Loading` / `Ready`, in `resources/common_assets.rs`) gates almost every other
  system via `run_if(resource_equals(...))` in `main.rs` — asset loading (`resource_loader`)
  only runs while `Loading`; simulation/rendering systems only run while `Ready`. When all
  fonts/icons finish loading, `resource_loader::load_assets` flips state back to `Ready` and
  fires a `GraphChange` event, which `node_system::create_nodes` uses to despawn and redraw all
  node entities from scratch.
- Node/connector visuals are rebuilt reactively from `GraphDefinitionRes`, not incrementally
  patched — `create_nodes` and `update_connectors` both do despawn-and-respawn-all on relevant
  changes rather than diffing.

### Rendering/interaction systems (`src/systems/`)

- `node_system.rs` spawns node entities (icon + label sprites, tween-animated into a random
  position, `bevy_mod_picking` click/drag handlers).
- `update_connectors.rs` — see the dedicated section above.
- `update_message.rs` walks each connector's path (`lyon_algorithms::walk`) to animate an
  in-flight message icon/label along the curve based on its timer's progress, then moves it
  from `msg_inflight` to `msg_delivered` once the timer finishes (picked up by
  `rhai_engine::execute_rhai_engine` next frame to trigger `on_msg`).
- `zoom_panel.rs` / `drag.rs` handle Ctrl/Cmd-scroll zoom and node dragging.
- `explain_bubble.rs` / `radial_blur.rs` implement the `explain()` narration-bubble system —
  see the dedicated section above.
- `browser_resize.rs` (wasm-only, `#[cfg(target_arch = "wasm32")]` in `systems/mod.rs`) keeps
  the Bevy canvas sized to its parent `<div>`.
- `ui.rs`'s `graph_properties_viewer` is the app's one `bevy_egui` window ("Graph
  Properties" — sim-speed slider, background/text/connection color pickers, and per-node
  param editing when a node is selected). `.collapsible(true).default_open(false)` so it
  loads collapsed to just its title bar instead of covering the canvas on every load.

### wasm/JS bridge (`src/wasm/`)

`c_log!` forwards to `console.log`. `log_dsa_event!` is the channel Rhai's `log()` calls ride
on: it dispatches a DOM `CustomEvent("dsa-log-event")` on `#dsa-log-event-listener`, which
`+page.svelte` listens for and feeds into the log panel (`panels.ts`) — this is the only path
from inside a running simulation back into the frontend's UI log, so grep for
`log_dsa_event`/`dsa-log-event` when tracing "why doesn't my log show up" issues.

### Frontend (`web/src/routes/`)

- `+page.svelte` is the app shell: loads the wasm module (`init()` from `./dsa` — the
  symlinked wasm-bindgen output), wires up the Menubar, dockview panel layout (`panels.ts`),
  and the log event listener described above.
- `Monaco.svelte` wraps `monaco-editor` + `monaco-yaml`, configured with the JSON Schema
  fetched from `get_code_schema()` so the YAML editor validates/autocompletes against the
  Rust-side `File`/`GraphDefinition` structs.
- `panels.ts` builds the `dockview-core` layout (editor, canvas, log table via
  `tabulator-tables`).
- Styling is Tailwind + daisyUI (`tailwind.config.js`, `app.css`).
