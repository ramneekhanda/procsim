# Manual Coordinates & Precise Overrides

While automatic layouts handle 95% of diagrams out-of-the-box, ProcSim gives you complete control to fine-tune individual node placements and offsets.

---

## Relative Offsets (`offset: [dx, dy]`)

You can shift any node relative to its calculated layout position:

```yaml
graph:
  - name: observer
    node_type: observer
    offset: [0, -60]   # Nudge 60px downwards
```

---

## Absolute Coordinates (`pos: [x, y]`)

To pin a node to exact world coordinates:

```yaml
graph:
  - name: external_service
    node_type: cloud
    pos: [350, 180]   # Exact (x, y) coordinates
```

---

## Manual Mode (`type: manual`)

If you want 100% free-form canvas positioning without any automatic rank or group calculation:

```yaml
layout:
  type: manual
  draggable: true    # Allow free-form dragging on canvas
```

In manual mode with `draggable: true`, you can freely drag nodes around the canvas to design custom layouts!
