# 2.1 Hierarchical Pipelines

ProcSim features an automatic deterministic layout engine that calculates coordinates for every node and connector based on topological dependencies.

---

## 1. Enabling Hierarchical Layout

Set `layout.type: hierarchical` in `graph_defn`:

```yaml
graph_defn:
  layout:
    type: hierarchical
    direction: lr      # lr (Left-to-Right) or tb (Top-to-Bottom)
    rank_sep: 240      # Distance between pipeline stages
    node_sep: 120      # Distance between parallel nodes in the same stage
```

### Layout Directions
- `lr`: Standard left-to-right flow (ideal for pipelines, streaming, request/response).
- `tb`: Top-to-bottom hierarchy (ideal for trees, master-worker, supervisors).
- `rl` and `bt`: Right-to-left and bottom-to-top inverted flows.

---

## 2. Spacing Parameters

| Parameter | Default | Description |
|---|---|---|
| `rank_sep` | `260.0` | Distance between consecutive ranks/stages along the primary flow axis. |
| `node_sep` | `140.0` | Center-to-center distance between parallel nodes in the same stage. |
| `draggable` | `false` | When `false`, locks node positions into clean aligned ranks. |

---

## 3. How Ranks Are Derived

The engine performs topological longest-path layering:
1. **Source nodes** (in-degree 0) start at **Rank 0**.
2. Downstream nodes are assigned to $\text{Rank} = \max(\text{upstream ranks}) + 1$.
3. Nodes in the same rank are centered symmetrically on the cross axis.
