# 3.3 Reusable Node Templates

Instead of manually repeating shape arrays in every script, you can declare declarative templates under `node_templates` in `graph_defn`.

---

## 1. Defining `node_templates`

```yaml
graph_defn:
  node_templates:
    - id: server_card
      template:
        - shape: rect
          w: 130
          h: 55
          radius: 6
          bg: "#ffffff"
          border: "#0284c7"
          border_width: 2
        - shape: text
          text: "SERVICE"
          y: 12
          color: "#0369a1"
          font_size: 10
```

---

## 2. Referencing Templates in Node Types

Attach the template to any node type via `attrs.template_ref`:

```yaml
  node_types:
    - id: api_server
      attrs:
        template_ref: server_card
        ticks: 4
```

Any node using this type will automatically render with the template's shapes. Handlers can still call `draw()` at runtime to add dynamic badges or overlay progress meters on top.

---

## 3. The Built-in `node` Template

You don't have to define your own template just to get a themed look. Every built-in theme preset (`themes:cloud`, `themes:cyberpunk`, `themes:matrix`, etc. — see Chapter 4) already ships a template literally named `node`, alongside its own specialized one (`cloud_card`, `cyber_hud`, ...). Point a node type's `template_ref` at `node` and it renders correctly under **any** theme, because every theme's `node` template answers to the same field contract:

| Field | Meaning | Auto-filled? |
|---|---|---|
| `node_name` | The node's own name | always, from the node itself |
| `icon` | The node type's `attrs.icon` | if `attrs.icon` is set |
| `role_tag` | A short subtitle/badge | defaults to `"NODE"` |
| `accent_color` | Border/stripe highlight | defaults to a theme-neutral blue |
| `status_text` / `status_bg` / `status_color` | The status pill | all have sensible defaults |

Because every field has a fallback, `template_ref: node` renders something reasonable even with zero `template_params` set — swap the theme import and the same node type just gets re-skinned, no compile errors. The theme's *other*, specialized templates (`cyber_hud`, `minimal_pill`, ...) don't make this promise — they're meant to showcase that one theme's look, not survive a theme swap.

---

## 4. Updating Fields at Runtime — `update_node_params()`

`draw()` replaces a node's *entire* overlay every time it's called — fine for a one-off custom look, but wasteful if all you want is to flip a status color on tick. `update_node_params(#{...})` instead re-renders the node's existing template with just the fields you name changed; everything else (body, icon, name, layout) stays exactly as it was:

```rhai
fn on_timer() {
    state.busy = !(state.busy ?? false);
    if state.busy {
        update_node_params(#{ status_text: "BUSY", status_color: "#dc2626" });
    } else {
        update_node_params(#{ status_text: "IDLE", status_color: "#94a3b8" });
    }
}
```

Watch `worker_1`/`worker_2` below — their status pill flips between IDLE and BUSY on every tick without the rest of the card being redrawn.

A couple of rules worth knowing:
- Updates **persist** across calls, so a later `update_node_params(#{status_text: "DONE"})` doesn't need to repeat `status_color` too if it hasn't changed.
- A node type with no `template_ref`/`template` at all has nothing to update — calling it there (or with an empty `#{}`) logs a `WARN:` line to the Logs panel instead of doing nothing silently or crashing.

---

## 5. Per-Instance Scripts — Overriding `fn` for One Node

A script belongs to a *node type* — every node using `node_type: worker` normally runs the exact same `fn`. That's usually what you want, but it gets in the way when you're reusing a shared or imported type (say, a library's `lambda_func`) and just want *one* instance to behave differently, without copy-pasting its whole definition (icon, `template_ref`, colors, ...) just to change its script.

**Static nodes** — a `graph:` entry can carry its own `fn:`, which replaces the type's script for that one instance only:

```yaml
graph:
  - name: worker_override
    node_type: worker        # still gets worker's icon, template, ticks, etc.
    fn: |                    # but runs THIS script instead of worker's own
      fn on_init() {
        update_node_params(#{ status_text: "OVERRIDDEN", status_color: "#7c3aed" });
      }
    links: []
```

**Dynamically spawned nodes** — `spawn_node()` has a 4-argument form for the same thing at runtime:

```rhai
// 3-arg form: spawns with node_type's own default script
spawn_node("worker_3", "worker", []);

// 4-arg form: spawns with worker's icon/template, but this script instead
spawn_node("worker_3", "worker", [], "
  fn on_init() { update_node_params(#{ status_text: \"SPAWNED\" }); }
");
```

Both forms compile the override once (the same way a node type's own `fn` is compiled) and leave every other instance of that type untouched. Note the script string is itself Rhai source text, so quotes inside it need escaping (`\"`) and it can't contain a raw, un-escaped newline inside a `"..."` literal — write it as one line, or build it with string concatenation if it needs to be longer.
