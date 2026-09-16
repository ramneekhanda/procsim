# 4.1 Cloud Architecture Theme

ProcSim includes built-in theme presets that instantly style node cards, message packets, and connector lines with professional aesthetics.

---

## 1. Applying a Preset Theme

Use `theme: "themes:cloud"` in `graph_defn`:

```yaml
graph_defn:
  theme: "themes:cloud"
  layout:
    type: hierarchical
    direction: lr
```

---

## 2. Cloud Theme Features
- **Clean Card Containers**: Nodes are styled as white rounded cards with subtle drop borders.
- **Service Icons**: Compatible with cloud icon sets (e.g. AWS, GCP, Azure).
- **Subtle Traffic Highlighting**: Messages appear as glowing badges traveling across connection channels.
