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
   simulating flaky/failing behavior without a manually-toggled param) to talk back to
   the engine; `send_messages` fans sent messages out to the `Messages` component on
   whichever `NodeConnector` links the two node names, becoming an in-flight `Message`
   with a 3s timer.
4. `state` (the Rhai `globals` map) round-trips through the node's persistent `Dynamic` field
   across calls — this is how a node keeps memory between ticks/messages.

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
- `update_connectors.rs` draws bezier-curve connector paths (via `bevy_prototype_lyon` +
  `lyon_algorithms`) between linked nodes and keeps them glued to node positions as they're
  dragged.
- `update_message.rs` walks each connector's path (`lyon_algorithms::walk`) to animate an
  in-flight message icon/label along the curve based on its timer's progress, then moves it
  from `msg_inflight` to `msg_delivered` once the timer finishes (picked up by
  `rhai_engine::execute_rhai_engine` next frame to trigger `on_msg`).
- `zoom_panel.rs` / `drag.rs` handle Ctrl/Cmd-scroll zoom and node dragging.
- `browser_resize.rs` (wasm-only, `#[cfg(target_arch = "wasm32")]` in `systems/mod.rs`) keeps
  the Bevy canvas sized to its parent `<div>`.

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
