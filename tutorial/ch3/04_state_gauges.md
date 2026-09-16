# 3.4 Live State Gauges & Meters

By combining Rhai message handlers with `draw()`, nodes can visualize dynamic buffer depths, active request queues, and health metrics directly on canvas.

---

## 1. Computing Visual Geometry from State

When a node receives work or processes items, calculate shape dimensions dynamically:

```rhai
fn redraw_meter(count) {
  let color = if count >= 3 { "#ef4444" } else if count > 0 { "#f59e0b" } else { "#10b981" };
  let bar_w = if count * 20 > 90 { 90.0 } else { (count * 20) * 1.0 };
  let fill_radius = (bar_w / 2.0).min(3.0);
  let CARD_Y = 80;

  draw([
    #{ shape: "rect", w: 140, h: 60, radius: 8, y: CARD_Y, fill: "#ffffff", stroke: color, stroke_width: 2 },
    #{ shape: "text", text: "Buffer: " + count + " items", y: CARD_Y + 12, color: "#1e293b", size: 11 },
    #{ shape: "rect", w: 90, h: 8, radius: 3, y: CARD_Y - 8, fill: "#e2e8f0" },
    #{ shape: "rect", w: bar_w, h: 8, radius: fill_radius, x: (bar_w - 90.0) / 2.0, y: CARD_Y - 8, fill: color }
  ]);
}
```

---

## 2. Realistic Work Simulation

1. **Producer**: Pushes items downstream on a timer.
2. **Worker**: Enqueues incoming jobs, updates its live load bar, and drains items at a steady rate.
