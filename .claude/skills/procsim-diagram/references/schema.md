# procsim YAML schema reference

Source of truth: `src/parser/graphv2.rs` (`File`/`GraphDefinition` structs, `schemars`-derived
JSON Schema also feeds Monaco's live autocomplete in the app itself). This file is a curated
field reference for writing YAML by hand — when in doubt, grep that file or an existing
`web/static/tutorial/**/*.yml` example instead of guessing.

## Top-level shape

```yaml
theme: cyberpunk        # shorthand for imports: [{from: "theme:cyberpunk"}]
imports: [...]          # see references/imports-and-themes.md
layout: {...}           # LayoutConfig — see below
groups: [...] | {...}   # list of GroupDef, OR a map of id -> GroupDef (both accepted)
fns:                    # NOT part of the schema itself — pure YAML-anchor convenience,
  - &my_fn |             # define reusable Rhai scripts once, reference with *my_fn in
    fn on_timer() { ... } # node_types[].fn / graph[].fn. See any tutorial file.
graph_defn:
  theme: ...             # same fields also valid nested here instead of top-level
  imports: [...]
  layout: {...}
  groups: [...]
  graph_attrs: {...}
  icons: [...]
  node_templates: [...]
  node_types: [...]
  graph: [...]
```

`theme`/`imports`/`layout`/`groups` at the file root are pure shorthand — they get folded
into `graph_defn` at parse time. Pick whichever placement reads better; don't specify both.

## `node_types[]` (`NodeType`)

```yaml
node_types:
  - id: worker              # `name:` also accepted as an alias for `id`
    fn: |                   # Rhai source with on_init/on_timer/on_msg — optional;
      fn on_timer() { ... }  # a type with no fn is a silent/inert node (fine for pure sinks)
    attrs:
      ticks: 5                        # see Ticks below
      icon: worker_icon                # must match an id in top-level `icons:`
      template_ref: compute_unit       # id in top-level `node_templates:`
      template_params: { status: "IDLE" }  # values substituted into that template's {{name}}s
      template:                        # inline TemplateShape list — see "Static templates"
        - shape: rect
          ...
    params:                 # becomes live-editable controls in the app's Node Properties popup
      - name: retry_limit
        type: Integer        # Bool | Float | Integer | String | Option
        default: 3
        min: 0
        max: 10
```

**`Ticks`** (`attrs.ticks`): a plain integer (`ticks: 5`), or a range object:
```yaml
ticks: { min: 4, max: 6 }              # one random value picked once, at node creation
ticks: { min: 4, max: 6, jitter: true } # re-randomized every time the timer fires
```
Use jitter for anything that should look organic/asynchronous (heartbeats, retries) instead
of a suspiciously uniform pulse. Default when omitted is a fixed `2`.

**`params[].type`** (`ParamType`, tagged by `type`):
- `Bool { default }`
- `Float { min, max, default }`
- `Integer { min, max, default }`
- `String { default }`
- `Option` — check `graphv2.rs` near `ParamType::` if you need this; less commonly used than
  the four above.

Params become read-only Rhai constants in scope by their `name` (e.g. `retry_limit` is
directly usable inside `on_timer`/`on_msg`/`on_init` as a variable, not `params.retry_limit`).

## `graph[]` (`NodeConnection` — the actual instances)

```yaml
graph:
  - name: worker_1
    node_type: worker
    links: [db_primary, cache]   # who this node can send() to / connector endpoints
    group: compute_tier          # optional — must match a GroupDef id, see Groups below
    fn: |                        # optional per-instance script override — replaces the
      fn on_init() { ... }        # node_type's own `fn` for just this instance (icon/
                                   # template/params still come from node_type)
    pos: [120, -40]               # optional explicit pin — wins over any layout algorithm
    offset: [0, 20]               # optional nudge relative to the computed layout position
    rank: 2                       # hierarchical layout only — explicit rank override
    order: 1                      # hierarchical layout only — explicit order-within-rank
```

`links` values must name other `graph[]` entries that exist somewhere in the file (or that a
handler later creates via `spawn_node`) — no dangling/forward references to names that never
appear.

## `graph_attrs` (`GraphAttrs`)

```yaml
graph_attrs:
  title: "My Simulation"
  background: "#0f172a"          # any CSS-style color string
  connection_color: "#334155"
  text_color: "#e2e8f0"
  connector_style: step          # curved | straight | step (default: step)
  font: "https://.../MyFont.ttf" # one font for the whole graph's canvas text
  message_theme: {...}           # in-flight message bubble styling — see a theme plib for shape
  explain_theme: {...}           # explain() bubble styling — see a theme plib for shape
```
Colors accept hex or common CSS names. `connector_style`:
- `step` (default) — orthogonal right-angle routing, rounded corners, ≤3 segments.
- `curved` — bows perpendicular to the A→B line, good for organic/mesh-y graphs.
- `straight` — a single direct segment, good for dense/simple graphs where step routing
  would just add visual noise.

**Connector overlap in fan-out/fan-in or multi-tier graphs**: the connector router
(`src/systems/update_connectors.rs`) computes every edge's path independently from just its
own two endpoints — there is no crossing-avoidance or edge bundling, and it has no awareness
of any other connector or node on the canvas. This means a many-to-many pattern (e.g. two
load-balanced service instances each calling the same three downstream services — a 6-edge
bundle) will visually converge/cross near the shared targets no matter what layout you pick;
that's an inherent property of the router, not something a spacing tweak "fixes" outright.
What you *can* do, roughly in order of impact:
1. Prefer `connector_style: step` over `curved` whenever a graph has real fan-out/fan-in —
   orthogonal routing gives each source its own lane merging into a shared bus, which stays
   legible; curved bows from nearby-but-distinct source points tend to visually tangle.
2. Give it room: bump `rank_sep`/`group_sep` above their defaults so converging edges have
   space to fan out before bunching at the target, instead of crossing right at the node edge.
3. If the *real* architecture doesn't strictly require every source independently reaching
   every target, route through a shared hub/queue node instead — that turns an M×N edge count
   into M+N and is usually also a more honest model of the system (e.g. an event bus or
   message queue between producers and consumers, rather than direct point-to-point calls).
4. `graph[].rank`/`order` overrides can align sources and targets so more edges run parallel
   instead of crossing, if the default topological ordering puts them at odds.
Set the reader's expectations honestly: a genuinely dense many-to-many topology will still
look busy after all of the above — that's expected of this router, not a defect to keep
chasing.

## `layout` (`LayoutConfig`)

```yaml
layout:
  type: hierarchical    # hierarchical (default) | grid | circular | manual
  direction: lr          # lr | tb | rl | bt — hierarchical only
  rank_sep: 260           # spacing between ranks/columns (aliases: rank_spacing, rankSep, ...)
  node_sep: 140           # spacing between nodes within a rank (aliases: node_spacing, ...)
  group_sep: 320          # spacing around/between group boxes
```
- **`hierarchical`** (the real layout engine): groups (from `groups:`) collapse into single
  macro-boxes, then everything is ranked by longest-path topological order over the
  node/group links and projected along `direction`. Use for pipelines, fan-out/fan-in,
  request→response chains, tiered architectures. A group's *internal* direction defaults to
  the opposite of the graph's global direction (an `lr` pipeline stacks each group's members
  `tb`) unless overridden per-group (see Groups below).
- **`circular`** — even ellipse placement by index. Use for rings, peer-to-peer meshes,
  token-passing, gossip protocols — anything without a clear directional flow.
- **`grid`** — `sqrt(n)`-column matrix. Use for a flat pool of near-identical peers (worker
  pool, shard set) with no meaningful ordering.
- **`manual`** — nothing auto-placed; every node needs its own `pos`. Rare — only reach for
  this when the user describes exact spatial arrangement.
- Any layout: a `graph[].pos` always wins outright; `.offset` nudges the computed position
  instead of replacing it.

**Spacing vs. `draw()` overlays — a trap worth naming explicitly**: `node_sep`/`rank_sep`
place nodes purely from graph structure — the layout engine has no idea how big any node's
`draw()` overlay actually is. A status card that floats above its node (the common
`y: 70-90, h: 50-60` pattern in `references/rhai-handlers.md`) can reach ~100+ units from the
node's own center. Default `node_sep` (140) or anything you pick without accounting for that
reach will let one node's card visually overlap the node stacked next to it. Rule of thumb:
if nodes in the same rank/group carry overlay cards, set `node_sep` to comfortably exceed
(card height + offset from center), not just "enough room for an icon + label" — 200-260 is a
safe range for cards in the 50-60px-tall range; bump further for bigger cards. This bit a real
generated graph (a 12-node pipeline with ~55px cards under `node_sep: 120` — cards visibly
covered the neighboring node) — treat overlay size as a real input to spacing, not an
afterthought to fix after the fact.

## `groups` (`GroupDef`) — clustering + labeled boxes

Two equivalent forms — a list with explicit `id`, or a map keyed by id:
```yaml
groups:
  - id: cohort_tier
    title: "Participant Databases"
    direction: tb          # this group's own internal layout direction (GroupLayoutConfig)
    sep: 170                # this group's own internal member spacing
    nodes: [db_cohort_1, db_cohort_2]   # alternative to setting `group:` on each graph[] entry
    style:
      box: true             # false = use this group purely for layout, don't render a box
      bg: "#f0f9ff"
      border: "#0284c7"
      border_width: 2.0
      radius: 14.0
      padding: 12.0
```
```yaml
groups:
  ingress:                  # map form — key becomes the id
    title: "Edge & Ingress Tier"
    direction: tb
    style: { bg: "#f0f9ff", border: "#0284c7", border_width: 2.0, radius: 14.0 }
```
Membership comes from either `GroupDef.nodes: [...]` OR each member's own `graph[].group: id`
field — pick one style per file, don't mix for the same group. Group boxes only render under
`layout.type: hierarchical`; other layout types ignore `group_boxes` but still respect
`group:` for nothing (so grouping is effectively hierarchical-only — mention this if a user
asks for grouped circular/grid layout).

## `icons` (`IconDef`)

```yaml
icons:
  - id: worker_icon
    url: https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.1/assets/72x72/1f4bb.png
```
Referenced by `node_types[].attrs.icon`. Twemoji CDN (as above) is a convenient source for
generic pictograms; AWS plibs (`references/imports-and-themes.md`) already define their own
icon sets for cloud-service nodes — prefer importing those over redefining AWS icons by hand.

## `node_templates` (`NodeTemplateDef`) — reusable static overlays

```yaml
node_templates:
  - id: compute_unit
    template:                      # list of TemplateShape (see below)
      - shape: rect
        w: 130
        h: 55
        radius: 8
        bg: "#ffffff"
        border: "#6366f1"
        border_width: 2
      - shape: text
        text: "{{status_text}}"    # {{name}} placeholders filled from attrs.template_params
        y: -8
        color: "{{status_color}}"
        font_size: 9
```
A node type opts in via `attrs.template_ref: compute_unit` + `attrs.template_params: {...}`.
Templates are purely static string substitution (no `state`/branching) — they're a default
look, not a replacement for `draw()`. A script can still update the *substituted values* at
runtime with `update_node_params(#{ status_text: "BUSY" })` (cheaper than a full `draw()`
call — only touches what changed) — see `references/rhai-handlers.md`.

### `TemplateShape` / `draw()` shape fields (shared vocabulary)
Tagged by `shape:` — `rect` (+ `radius` for rounded corners; `roundedrect` is `rect` with a
sane default radius baked in), `circle` (`r` for radius), `line` (`x1,y1,x2,y2`),
`polygon`/`polyline` (`points`), `text` (`text`, `size`/`font_size`, `bold`). Common
color/size keys accept a couple of natural aliases so you don't have to hunt for the exact
canonical name: `fill` also accepts `bg`/`color`; `stroke` also accepts `border`/
`border_color`; `stroke_width` also accepts `border_width`/`width`; a text shape's `size`
also accepts `font_size`. All coordinates are node-local (origin at node center, y-up).

## Full worked examples in this repo

Don't reinvent syntax — grep `web/static/tutorial/**/*.yml` for the closest existing pattern
first:
- `ch1/04_params_and_icons.yml` — params + icons + jittered ticks
- `ch1/08_runtime_topology.yml` — spawn_node/link/despawn at runtime
- `ch2/01_hierarchical_layouts.yml`, `ch2/02_groups_and_tiers.yml`,
  `ch2/03_grid_and_circular.yml`, `ch2/04_manual_and_offsets.yml` — one per layout type
- `ch3/01_draw_basics.yml`, `ch3/02_shapes.yml`, `ch3/03_node_templates.yml`,
  `ch3/04_state_gauges.yml` — custom visuals
- `ch4/*.yml` — theming patterns
- `ch5/01_intro_to_plibs.yml` through `ch5/06_aws_cloud_architecture.yml` — imports, AWS,
  load balancer/token ring/2PC/primary-backup protocol patterns
