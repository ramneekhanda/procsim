# Imports, themes, and stdlib presets

A graph can pull in reusable `node_types`/`icons`/`node_templates`/`layout`/`groups` from a
preset instead of redefining them inline. Prefer this over hand-rolling a theme's color
palette or an AWS service's icon/type by hand — it's less code and matches the app's own
built-in Learn/Examples content, so it composes cleanly if the user later imports another
plib alongside it.

## Syntax

```yaml
imports:
  - from: "theme:cyberpunk"          # bare shorthand form also works: `theme: cyberpunk`
  - from: "plibs:aws/compute"
  - from: "plibs:aws/database"
  - from: "stdlib:load_balancer"
    import: [round_robin_lb]         # optional: only pull these node_type ids
    as: my_lb                        # optional: rename the (single) imported type's id
  - from: "https://example.com/shared-types.yml"  # remote URL — resolved by the frontend,
                                                    # not fetchable from a hand-authored file
                                                    # you're producing offline; prefer a
                                                    # built-in preset unless the user already
                                                    # has a URL they want to use
```
**Merge precedence**: local definitions in the current file always win. `graph_attrs` fields
only fill in from an import if still at default; `node_types`/`icons`/`node_templates`/
`groups` dedupe by id (a locally-declared type with a blank `fn` still inherits the imported
one's script — useful for overriding just one field); `layout` only fills in if the base file
has none; `graph` instances merge by `name`. Imports are recursive (depth-capped) — a theme
or AWS plib can itself import further presets, though none of the built-ins currently do.

## Theme presets (`theme:<name>` or bare `<name>`)

Each theme is a `node_templates` + `graph_attrs.message_theme`/`explain_theme` bundle —
importing one gives your custom node types a consistent visual language for free (still need
your own `node_types[].attrs.template_ref` to actually opt a type into a theme's cards,
unless the theme's own doc says otherwise — check `plibs/themes/<name>.yml` for exact
template ids it defines).

| Name(s) | Vibe |
|---|---|
| `cyberpunk` | Neon on dark, glitchy HUD-style cards |
| `cloud` / `cloud_cards` | Clean SaaS/cloud-console card look |
| `datacenter` / `rack` | Server-rack styled boxes |
| `minimal` / `capsule` | Flat, understated pill/capsule shapes |
| `synthwave` | 80s retro-futurist gradient palette |
| `nordic` / `nord` | Muted cool-toned Nord color scheme |
| `dracula` | Dark, high-contrast Dracula palette |
| `matrix` | Green-on-black terminal aesthetic |
| `solarized_light` / `solarized` | Warm light Solarized palette |

Pick a theme based on the *tone* of what's being simulated if the user hints at one
("cyberpunk", "retro", "clean/professional", "dark mode") — otherwise leave it unset (plain
default styling) rather than guessing one that wasn't asked for.

## AWS service library (`plibs:aws/<module>` or `aws:<module>`)

Provides real AWS icons + ready-made node types for cloud-architecture diagrams. Modules:
`compute`, `networking`, `database`, `messaging`, or `all` (everything, if the user's
architecture spans multiple categories and importing 2-3 separate modules gets noisy).

| Module | `node_types` provided |
|---|---|
| `compute` | `lambda_func`, `ec2_instance`, `ecs_service` |
| `networking` | `api_gateway`, `alb`, `cloudfront` |
| `database` | `dynamodb`, `s3_bucket`, `rds_aurora` |
| `messaging` | `sqs_queue`, `sns_topic`, `eventbridge` |

Use these `id`s directly as `graph[].node_type` once imported (see
`web/static/tutorial/ch5/01_intro_to_plibs.yml` — imports `compute`+`database`, then wires a
custom `client` type to `lambda_func` → `dynamodb`). Don't redefine an AWS icon/type by hand
when one of these already covers it — check `plibs/aws/*.yml` in this repo if you need to see
an exact node type's own `fn:` (some are inert sinks with no script, others queue/process).

## stdlib behavior presets (`stdlib:<name>`)

Small, complete node-type implementations for common resilience/routing patterns — use these
instead of writing the same logic from scratch, then override with a per-instance `fn:` in
`graph[]` only if the described behavior differs from the stock one:

- `stdlib:load_balancer` (aliases: `load_balancer`, `lb`) → `round_robin_lb` (cycles through
  `links` in order) and `weighted_lb` (currently also round-robin — same rotation logic,
  named separately so a graph can later evolve it to true weighting without a type-id churn).
- `stdlib:circuit_breaker` (alias: `circuit_breaker`) → `circuit_breaker`: tracks
  `state.status` (`CLOSED`/`OPEN`) and `state.failures` against a `state.threshold` of 3,
  drops messages while `OPEN` instead of forwarding.
- `stdlib:cache` (aliases: `cache`, `lru_cache`) → `lru_cache`: a simple hit/miss cache keyed
  by the raw message value (`state.store`), forwards on miss.

**Note**: these three stdlib node types use `fn on_message(msg)` in their own source (an
older naming convention) rather than `on_msg` — this still works because the engine
recognizes both names, but write new handlers of your own using `on_msg` (the canonical name
documented everywhere else in this skill) rather than copying `on_message` from these.
