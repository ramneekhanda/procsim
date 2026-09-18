---
name: procsim-diagram
description: Build or edit procsim graph YAML files (this repo's distributed-systems/process simulator format — node_types with Rhai on_init/on_timer/on_msg handlers, graph instances, layout, groups, themes, plibs imports) from a natural-language description of a system, protocol, or process. Use this whenever the user asks to "simulate", "diagram", "model", or "visualize" a distributed system, protocol (2PC, Raft, gossip, leader election, ring, pub/sub, load balancing, circuit breaker, cache, saga, etc.), architecture (microservices, AWS setup, client/server pipeline), or any process made of discrete actors passing messages/ticking on timers — in this repo specifically, not generic diagramming. Also use it to extend, debug, or restyle an existing procsim YAML file (adding nodes, fixing a Rhai handler, changing layout/theme). Trigger even if the user doesn't say "procsim" or "YAML" explicitly, e.g. "can you show me how a leader election would work" or "make a graph of a 3-tier web app with a cache".
---

# procsim diagram builder

procsim graphs are not static diagrams — every node is a small Rhai state machine that
ticks on a timer and reacts to messages, and the "diagram" is the *emergent visual behavior*
of that simulation running. When translating a description into a graph, think in terms of
**actors, messages, and state transitions**, not boxes and arrows.

## Workflow

1. **Identify the actors.** Each distinct role becomes a `node_type` (e.g. `coordinator`,
   `worker`, `load_balancer`) with instances listed under `graph`. Don't create a `node_type`
   per instance unless instances genuinely behave differently — reuse one type across many
   nodes and let `links`/`params` differentiate them.
2. **Decide what drives activity.** Something has to originate messages — usually one node
   type ticks on a timer (`on_timer`) and everything downstream reacts (`on_msg`). Passive
   nodes (e.g. a database or cache) often need only `on_msg`, with a very long `ticks` (or
   none) since they don't need to matter on a schedule.
3. **Map the message flow to `links` + `send()`.** `links` in `graph:` declares which nodes
   *can* receive from a given node (also used for connector drawing) — a handler should only
   `send()` to names in its own `links` (or a name it stores from an inbound message, e.g.
   `msg.from`, for replies). Read `references/rhai-handlers.md` before writing handlers.
4. **Pick a layout that matches the topology.** Hierarchical/directional flows (pipelines,
   fan-out/fan-in) → `layout: {type: hierarchical, direction: lr|tb}`. A ring or peer group
   → `circular`. A quick uniform mesh → `grid`. Full field reference in
   `references/schema.md`.
5. **Group related nodes** (a tier, a set of replicas/cohorts) with `groups:` so the layout
   visually clusters them and optionally draws a labeled box around them.
6. **Reach for `plibs` before writing custom node types from scratch** — theme imports
   (`imports: [{from: "theme:cyberpunk"}]` etc.) and AWS/service icon+type libraries
   (`plibs:aws/compute`, `.../database`, `.../networking`, `.../messaging`, `.../all`) and
   stdlib patterns (`stdlib:load_balancer`, `stdlib:circuit_breaker`, `stdlib:cache`) already
   exist — see `references/imports-and-themes.md`. Only hand-write a `fn:` when the described
   behavior isn't one of those.
7. **Give it a visual identity, not just logic.** A bare node (no `draw()`/`template_ref`)
   only shows an icon + name label — fine for simple demos, but if the user describes
   *states* ("becomes the leader", "marks itself as down", "shows queue depth"), make that
   visible with a `draw()`-based status card (pattern in `references/rhai-handlers.md`) or a
   reusable `node_templates` entry (pattern in `references/schema.md`) so state changes are
   watchable, not just logged.
8. **Write the file, then sanity-check it against the schema** (field names, enum spellings,
   Rhai syntax) using `references/schema.md` and the real examples under
   `web/static/tutorial/**/*.yml` in this repo — grep those for a pattern before inventing
   syntax, since there's no local YAML validator/linter to run outside the app itself.

## Minimal skeleton

Every graph is one YAML document shaped like this (top-level `fns`/`imports`/`layout`/
`groups` are shorthands that get merged into `graph_defn` — see `references/schema.md` for
the equivalent nested form):

```yaml
fns:
  - &some_fn |
    fn on_init() { state.x = 0; }
    fn on_timer() { state.x += 1; send(links[0], #{ val: state.x, display: "val=" + state.x }); }
    fn on_msg(msg) { log(node_name + " got " + msg.val); }

graph_defn:
  graph_attrs:
    title: "My Simulation"
  layout:
    type: hierarchical
    direction: lr
  node_types:
    - id: my_type
      fn: *some_fn
      attrs: { ticks: 3 }
  graph:
    - name: node_a
      node_type: my_type
      links: [node_b]
    - name: node_b
      node_type: my_type
      links: []
```

Rules that are easy to get wrong:
- `graph[].links` must only reference names that exist elsewhere in `graph:` (or are
  `spawn_node`'d later at runtime) — no forward-declared/typo'd peers.
- A node with no `fn:` and no `node_type.fn` is legal (silent/inert node) — useful for a pure
  sink like a database that's only ever `send()`'d to and logged, never programmed.
- `ticks` can be a fixed number, `{min, max}` (one random value at load), or
  `{min, max, jitter: true}` (re-randomized every fire) — use jitter for anything meant to
  look organic/asynchronous (retries, heartbeats) rather than a suspiciously uniform pulse.
- Rhai host functions available inside handlers: `log(...)`, `send(to, msg)`, `draw(shapes)`,
  `random_chance(pct)`, `random_int(min, max)`, `spawn_node(...)`, `despawn(name)`,
  `link(peer)`/`unlink(peer)`, `update_node_params(map)`, `explain(key, text)`. Full
  signatures, scope variables (`state`, `links`, `node_name`, params), and worked patterns
  (retry/backoff, quorum voting, broadcast, request/reply) are in
  `references/rhai-handlers.md` — read it before writing non-trivial handlers, since a
  function called with the wrong arity/type just silently fails to compile rather than
  giving a friendly error.
- If `msg` in `send(to, msg)` is a map, always set a `display: String` field on it — that's
  what renders on the traveling message icon. Skip it and the icon falls back to Rhai's raw
  map stringification, which is illegible on canvas. Never set `msg.from` yourself — the
  engine always overwrites it with the sender's own name on every `send()` call, so a
  script-set value is silently discarded (see `references/rhai-handlers.md`'s `send` entry).

## Reference files

- `references/schema.md` — full YAML field reference: `GraphDefinition`, `node_types`,
  `graph` instances, `params`, `layout` (all 4 types + spacing knobs), `groups`, `icons`,
  `node_templates`, `graph_attrs` (theme/color fields), connector styling.
- `references/rhai-handlers.md` — Rhai scripting: scope variables, every host function's
  exact signature, `state` semantics, and common behavioral patterns (broadcast/quorum,
  request-reply, retry, leader election, token passing) with short code snippets.
- `references/imports-and-themes.md` — the `imports:`/`theme:` mechanism, full list of
  built-in theme and AWS presets, stdlib behavior presets, and merge precedence rules for
  writing a graph that composes presets instead of redefining everything inline.

Read the specific reference file(s) relevant to the current step rather than all three
up front — e.g. skip `imports-and-themes.md` entirely for a graph with no imports/theme.
