//! Microbenchmarks for the node-state round-trip in `systems::rhai_engine`'s
//! tick loop - the `clone`-based variants here reproduce the *old* code
//! (`scope.set_value("globals", state.clone())` / `scope.get_value(...)`),
//! the `move`-based variants reproduce the *current* code
//! (`scope.push_dynamic("globals", std::mem::take(state))` /
//! `scope.remove::<Dynamic>("globals")`) - see the comment on that call site
//! for the reasoning. Run with `cargo bench`.
//!
//! Two state shapes are benchmarked side by side:
//! - `small_state`: the actual 4-scalar-field shape of the Two-Phase Commit
//!   example's `coordinator` node (see `web/static/examples/two_phase_commit.yml`).
//! - `large_state`: a heavier shape (a string field plus a 20-element array)
//!   representative of a node tracking more history/detail - meant to show
//!   whether the clone-vs-move gap widens as state gets bigger, since a
//!   `Dynamic` clone is a deep clone (it recurses into every string/array/map
//!   it contains) while a move is always O(1) regardless of what's inside.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rhai::{CallFnOptions, Dynamic, Engine, Scope, AST};

fn engine_and_ast() -> (Engine, AST) {
    let mut engine = Engine::new();
    // Matches `initialize_engine`/`parse_graph2` in the real app - the default
    // in-function expression-nesting limit (32) is too low for realistic
    // handlers.
    engine.set_max_expr_depths(256, 256);
    let ast = engine
        .compile(
            r#"
            fn on_timer() {
                globals.tx_id += 1;
                globals.responses = 0;
                globals.yes = 0;
            }
        "#,
        )
        .expect("compile on_timer");
    (engine, ast)
}

/// The Two-Phase Commit `coordinator` node's actual state shape.
fn small_state() -> Dynamic {
    let mut map = rhai::Map::new();
    map.insert("phase".into(), "idle".into());
    map.insert("tx_id".into(), Dynamic::from(0_i64));
    map.insert("responses".into(), Dynamic::from(0_i64));
    map.insert("yes".into(), Dynamic::from(0_i64));
    Dynamic::from_map(map)
}

/// A heavier state shape - a string field plus a small array - to see
/// whether the clone/move gap widens with state size.
fn large_state() -> Dynamic {
    let mut map = rhai::Map::new();
    map.insert("phase".into(), "idle".into());
    map.insert("tx_id".into(), Dynamic::from(0_i64));
    map.insert("responses".into(), Dynamic::from(0_i64));
    map.insert("yes".into(), Dynamic::from(0_i64));
    map.insert(
        "description".into(),
        Dynamic::from("a moderately long status string to simulate real node state".to_string()),
    );
    let history: rhai::Array = (0..20).map(|i| Dynamic::from(i as i64)).collect();
    map.insert("history".into(), Dynamic::from_array(history));
    Dynamic::from_map(map)
}

/// The *old* code path: clone in, clone out.
fn call_via_clone(engine: &Engine, ast: &AST, scope: &mut Scope, state: &mut Dynamic) {
    let options = CallFnOptions::new().eval_ast(false).rewind_scope(false);
    let init_size = scope.len();
    scope.set_value("globals", state.clone());
    let _: Result<(), Box<rhai::EvalAltResult>> =
        engine.call_fn_with_options(options, scope, ast, "on_timer", ());
    *state = scope.get_value("globals").unwrap();
    scope.rewind(init_size);
}

/// The *current* code path: move in, move out.
fn call_via_move(engine: &Engine, ast: &AST, scope: &mut Scope, state: &mut Dynamic) {
    let options = CallFnOptions::new().eval_ast(false).rewind_scope(false);
    let init_size = scope.len();
    scope.push_dynamic("globals", std::mem::take(state));
    let _: Result<(), Box<rhai::EvalAltResult>> =
        engine.call_fn_with_options(options, scope, ast, "on_timer", ());
    *state = scope.remove::<Dynamic>("globals").unwrap_or_default();
    scope.rewind(init_size);
}

/// Isolates *just* the round-trip mechanism (no function call at all), to
/// measure the clone/move cost on its own, separate from script execution.
fn roundtrip_only_clone(scope: &mut Scope, state: &mut Dynamic) {
    let init_size = scope.len();
    scope.set_value("globals", state.clone());
    *state = scope.get_value("globals").unwrap();
    scope.rewind(init_size);
}

fn roundtrip_only_move(scope: &mut Scope, state: &mut Dynamic) {
    let init_size = scope.len();
    scope.push_dynamic("globals", std::mem::take(state));
    *state = scope.remove::<Dynamic>("globals").unwrap_or_default();
    scope.rewind(init_size);
}

fn bench_full_tick(c: &mut Criterion) {
    let (engine, ast) = engine_and_ast();
    let mut group = c.benchmark_group("full_tick_call");

    for (label, make_state) in [
        ("small_state", small_state as fn() -> Dynamic),
        ("large_state", large_state as fn() -> Dynamic),
    ] {
        group.bench_with_input(BenchmarkId::new("clone", label), &(), |b, _| {
            let mut scope = Scope::new();
            let mut state = make_state();
            b.iter(|| call_via_clone(black_box(&engine), black_box(&ast), &mut scope, &mut state));
        });
        group.bench_with_input(BenchmarkId::new("move", label), &(), |b, _| {
            let mut scope = Scope::new();
            let mut state = make_state();
            b.iter(|| call_via_move(black_box(&engine), black_box(&ast), &mut scope, &mut state));
        });
    }
    group.finish();
}

fn bench_roundtrip_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip_only");

    for (label, make_state) in [
        ("small_state", small_state as fn() -> Dynamic),
        ("large_state", large_state as fn() -> Dynamic),
    ] {
        group.bench_with_input(BenchmarkId::new("clone", label), &(), |b, _| {
            let mut scope = Scope::new();
            let mut state = make_state();
            b.iter(|| roundtrip_only_clone(&mut scope, &mut state));
        });
        group.bench_with_input(BenchmarkId::new("move", label), &(), |b, _| {
            let mut scope = Scope::new();
            let mut state = make_state();
            b.iter(|| roundtrip_only_move(&mut scope, &mut state));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_full_tick, bench_roundtrip_only);
criterion_main!(benches);
