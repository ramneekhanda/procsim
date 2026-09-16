# 4.5 Layered & Composed Themes

You can import a base theme preset and customize individual colors, templates, and overlay styles on top.

---

## 1. Local Overrides

```yaml
imports:
  - from: "themes:cloud"

graph_defn:
  graph_attrs:
    title: "Customized Theme"
    connection_color: "#6366f1"
```

Local definitions always take precedence over imported defaults, giving you full control over specific nodes and connectors.
