# 3.2 Vector Shape Primitives

ProcSim supports standard 2D vector drawing primitives with custom fills, strokes, corner radii, and alignments.

---

## 1. Supported Shapes

### Rectangles (`rect`)
```rhai
#{ shape: "rect", w: 120, h: 60, radius: 8, bg: "#ffffff", border: "#3b82f6", border_width: 2 }
```

### Circles (`circle`)
```rhai
#{ shape: "circle", r: 24, bg: "#ecfdf5", border: "#10b981", border_width: 2 }
```

### Lines (`line`)
```rhai
#{ shape: "line", x1: -50, y1: 0, x2: 50, y2: 0, color: "#94a3b8", width: 2 }
```

### Polygons & Polylines (`polygon` / `polyline`)
```rhai
#{ shape: "polygon", points: [[0, 20], [20, -15], [-20, -15]], bg: "#fef3c7", border: "#f59e0b", border_width: 2 }
```

### Text Labels (`text`)
```rhai
#{ shape: "text", text: "Healthy", x: 0, y: 0, color: "#0f172a", font_size: 12, bold: true }
```

---

## 2. Layering & Rendering Order

Shapes are drawn in array index order: elements earlier in the array are rendered in the background, while later elements render on top.
