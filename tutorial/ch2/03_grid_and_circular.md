# Grid & Circular Layouts

In addition to hierarchical pipelines, ProcSim supports geometric layout models suited for worker pools, ring topologies, and mesh networks.

---

## Grid Layout (`type: grid`)

Grid layout arranges all nodes into a balanced matrix of rows and columns:

```yaml
layout:
  type: grid
  node_sep: 140   # Spacing between rows and columns
```

This is ideal for:
- Worker server farms and compute pools
- Matrix routing networks
- Visual dashboards displaying multiple concurrent nodes

---

## Circular Layout (`type: circular`)

Circular layout arranges nodes evenly along an ellipse:

```yaml
layout:
  type: circular
```

This is ideal for:
- Token-ring networks
- Consensus leader election rings (Raft / Paxos)
- Decentralized peer-to-peer rings (Chord / DHT)
- Round-robin load balancing rings

Click **▶ Load this example** above to see a circular token-passing ring in action!
