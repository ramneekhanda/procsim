# 3.1 `draw()` Basics & Node Coordinates

Node types can dynamically render vector shapes, text labels, and UI indicators directly onto the canvas using the built-in `draw()` host function.

---

## 1. The `draw()` Host API

In any Rhai handler (`on_init`, `on_timer`, or `on_msg`), call `draw()` with an array of shape maps:

```rhai
draw([
  #{ shape: "rect", w: 140, h: 50, radius: 8, bg: "#ffffff", border: "#6366f1", border_width: 2 },
  #{ shape: "text", text: "ACTIVE", y: 0, color: "#1e1b4b", font_size: 14 }
]);
```

---

## 2. Coordinate System

- Coordinates are **node-local** (origin `(0, 0)` is the center of the node).
- **Y-Up**: $+Y$ points **upward**, $-Y$ points **downward**.
- **X-Right**: $+X$ points **right**, $-X$ points **left**.
- Visual overlays move and scale automatically when the node is panned, zoomed, or dragged.

---

## 3. Dynamic Updates & Replacement

- Each call to `draw([...])` **replaces** the previous overlay on that node.
- If a handler does not call `draw()`, the existing overlay remains intact.
- Calling `draw([])` clears the overlay completely.
