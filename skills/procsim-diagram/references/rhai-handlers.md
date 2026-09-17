# Rhai scripting reference

Each `node_types[].fn` (or per-instance `graph[].fn` override) is a Rhai source string
defining up to three functions. The engine (`src/systems/rhai_engine.rs`) calls whichever
exist — omit any you don't need:

- `fn on_init()` — runs once, when the node is created (graph load or `spawn_node`). Seed
  `state` here.
- `fn on_timer()` — runs each time the node's `attrs.ticks` timer fires.
- `fn on_msg(msg)` — runs once per delivered message, when a message this node was `send()`'d
  arrives. (`on_message` is also recognized as an alias — the canonical name is `on_msg`;
  prefer it in anything you write.)

## Scope variables available inside handlers

- `state` — a persistent `Dynamic` (map, usually) that round-trips across every call for
  this node instance. This is the *only* way a node remembers anything between ticks/
  messages — there is no other persistent storage. Always initialize the fields you use in
  `on_init` (or guard with `?? default`, e.g. `state.busy ?? false`) since a node created via
  `spawn_node` without an explicit `on_init` starts with an empty/default state.
- `links` — a read-only array of this node's linked peer names (`graph[].links`, kept live —
  `link()`/`unlink()` update it). Iterate it for broadcast (`for peer in links { send(peer, ...) }`)
  or index it for round-robin (`links[state.idx % links.len()]`) rather than hardcoding peer
  names, so the script still works if the graph's topology changes.
- `node_name` — this node's own instance name (`graph[].name`), useful for logging
  (`node_name + " received X"`) or for reusable scripts shared across many instances via
  `&anchor` (see `two_phase_commit.yml`'s `cohort_fn`, used by two differently-named nodes).
- Any `node_types[].params[].name` — each declared param is a read-only constant directly in
  scope by name (not nested under a `params` object).
- `msg` — only inside `on_msg(msg)`: the delivered message, a `Dynamic` map/value exactly as
  it was passed to `send()`, with one exception (see the `from` gotcha immediately below).
  Common convention: give messages a `type` field and switch on it
  (`if msg.type == "PREPARE" { ... }`).

**`msg.from` is engine-managed, not script-set — this breaks multi-hop relays if you're not
careful.** Every time `send()` is called, the engine (`send_messages` in
`src/systems/rhai_engine.rs`) unconditionally overwrites the message's `from` key with the
*calling* node's own name, discarding whatever the script put there. For a direct reply
(A sends to B, B replies with `send(msg.from, ...)`) this is exactly what you want — `msg.from`
correctly names A. But if a message passes through an intermediary before reaching its real
destination (a load balancer, a bus, any relay that does `send(next_hop, msg)` with the same
message object), `from` gets re-stamped with the *relay's* name at that hop, silently
discarding whoever the original sender was. A downstream handler that stores `msg.from`
expecting to reply to the original sender will instead reply to the relay - which, if that
relay's own `on_msg` doesn't specifically handle the reply's `type`, means the reply just
disappears with no error anywhere.

The fix: never rely on `from` surviving more than one hop. If a reply needs to reach past an
intermediary, carry that address in your own field instead - e.g. `reply_to: node_name` - since
every field *other than* `from` passes through a relay's `send(next_hop, msg)` completely
unchanged. A relay only needs to forward the message as-is (`send(target, msg)`); it never
needs to know about or touch `reply_to`.

**You don't need to worry about a handler's trailing statement "leaking" a return value.**
An earlier version of this engine called `on_msg`/`on_timer`/`on_init` via Rhai's `call_fn::<()>`,
which demanded the function return unit - and something like `state.pending.remove(oid);` as the
last statement of an `if`/`else if` branch that was also the function's own tail expression could
leak `Map::remove()`'s return value through anyway (a trailing `;` didn't fully discard it in that
specific position), throwing `ErrorMismatchOutputType` on every call that hit that branch. The
engine (`src/systems/rhai_engine.rs`) now calls these via `call_fn::<Dynamic>` instead, which
accepts any return value including unit, so this can no longer happen - write handlers naturally,
with no need to `let`-bind a call whose result you don't care about. If you ever see
`ErrorMismatchOutputType` in the console on a graph that's otherwise behaving correctly (sends
still going through), that's the signal something's reverted this fix - check
`src/systems/rhai_engine.rs`'s `call_fn_with_options::<Dynamic>` call sites and the regression
tests in that file's `tests` module before reintroducing any workaround in the YAML.

## Host functions

| Function | Signature | Notes |
|---|---|---|
| `log` | `log(s)` / `log(dynamic)` / `log(a, b)` | Prints to the app's log panel. The 2-arg form space-joins (`log("count:", state.n)`). |
| `send` | `send(to: String, msg: Dynamic)` | Queues a message to node `to` (must be a live node — a name in your own `links`, or one from an inbound `msg.from`). Delivered async after the connector's travel animation, arrives as `on_msg(msg)` on the target. |
| `draw` | `draw(shapes: Array)` / `draw(shape: Map)` | Replaces this node's overlay with the given shape list (see `references/schema.md`'s TemplateShape/draw table for the shape vocabulary). `draw([])` clears it; not calling `draw()` leaves the last one. Works from `on_init` too. |
| `random_chance` | `random_chance(percent: Int) -> Bool` | `true` with roughly that % probability — use for flaky/failing behavior (`if random_chance(20) { ... simulate a dropped request ... }`) instead of a manually-toggled param. |
| `random_int` | `random_int(min: Int, max: Int) -> Int` | Half-open `[min, max)`. Degenerate `max <= min` returns `min` (safe to call even when `links.len() == 0` might make `max` 0). |
| `spawn_node` | `spawn_node(name, node_type, links: Array)` / `spawn_node(name, node_type, links, fn_override: String)` | Creates a new node instance at runtime; its `on_init` runs immediately. `links` must already exist. Use the 4-arg form to give the new instance a bespoke one-off script without inventing a whole new `node_type`. |
| `despawn` | `despawn(name: String)` | Removes any node by name, including the caller (self-destructing nodes are fine). |
| `link` / `unlink` | `link(peer: String)` / `unlink(peer: String)` | Adds/removes `peer` from the **calling** node's own `links` — self-scoped by convention, not enforced. Takes effect next tick/message. |
| `update_node_params` | `update_node_params(p: Map)` | Cheaper alternative to `draw()` when the node uses `attrs.template_ref`/`template`: updates just the named `{{placeholder}}` values and re-renders, without restating every shape. No-op if the node has no template at all. |
| `explain` | `explain(key, text)` / `explain(key, text, opts)` / `explain(text)` / `explain(text, opts)` | Shows a one-time, sim-pausing narration bubble anchored to the calling node, for "here's what's happening" beats. `key` dedups — a given key only ever shows once per graph load no matter how many times it's called again. The 1-arg-text forms reuse `text` itself as the key. Use sparingly — good for a tutorial/demo walkthrough of a protocol's phases, not for routine per-message chatter (that's what `log()` is for). |

Calling an undeclared/mistyped host function, or the right function with the wrong argument
count/types, fails to compile silently from the YAML author's perspective (no red squiggly
outside the live app) — double check spelling and arity against this table for anything
non-trivial.

## Common patterns

**Broadcast / fan-out** (one sender, many peers, driven by topology not a hardcoded list):
```rhai
fn on_timer() {
  for peer in links {
    send(peer, #{ type: "PING", from: node_name });
  }
}
```

**Request/reply** (recipient replies to whoever sent it, not a fixed peer):
```rhai
fn on_msg(msg) {
  if msg.type == "REQUEST" {
    send(msg.from, #{ type: "RESPONSE", from: node_name, result: 42 });
  }
}
```

**Quorum / vote collection** (coordinator waits for N replies before acting — see
`two_phase_commit.yml` for the full worked example):
```rhai
fn on_init() { state.votes = []; }
fn on_msg(msg) {
  if msg.type == "VOTE" {
    state.votes.push(msg.vote);
    if state.votes.len() == links.len() {
      // all peers responded — decide and broadcast the outcome
    }
  }
}
```

**Round-robin routing** (load balancer):
```rhai
fn on_init() { state.idx = 0; }
fn on_msg(msg) {
  if links.len() == 0 { return; }
  send(links[state.idx % links.len()], msg);
  state.idx += 1;
}
```
A relay like this is exactly where the `msg.from` gotcha above bites: forwarding `msg` here
re-stamps `from` with this load balancer's own name, so whatever it forwards to must not
expect to reply via `msg.from` and reach the *original* caller - see that section for the
`reply_to`-field fix if the downstream handler needs to reply past this hop.

**Randomized/flaky failure simulation**:
```rhai
fn on_msg(msg) {
  if random_chance(15) {
    log(node_name + " dropped a message (simulated failure)");
    return;
  }
  send(links[0], msg);
}
```

**Visible status card via `draw()`** (make internal state changes watchable, not just logged
— reach for this whenever the description talks about a node's *state* changing, e.g.
"becomes leader", "marks itself unhealthy"):
```rhai
fn update_ui(status, color) {
  draw([
    #{ shape: "rect", w: 140, h: 55, radius: 8, y: 70, fill: "#ffffff", stroke: color, stroke_width: 2 },
    #{ shape: "text", text: node_name, y: 82, color: "#1e293b", size: 10 },
    #{ shape: "text", text: status, y: 60, color: color, size: 9 }
  ]);
}
fn on_init() { update_ui("READY", "#64748b"); }
fn on_msg(msg) { update_ui("PROCESSING", "#f59e0b"); }
```
A card like this reaches roughly `y_offset + h/2` from the node's own center (here,
`70 + 55/2 ≈ 97`) — if several such nodes sit in the same rank/group, the layout's `node_sep`
needs room for that reach on top of the node itself, or one node's card visually overlaps the
node next to it. See the "Spacing vs. `draw()` overlays" note in `references/schema.md` before
finalizing `layout.node_sep` for any graph where multiple stacked nodes carry cards like this.

**Runtime topology growth** (spawn workers on demand):
```rhai
fn on_timer() {
  if links.len() < 3 {
    let n = "worker_" + (links.len() + 1);
    spawn_node(n, "worker", []);
    link(n);
  }
}
```
