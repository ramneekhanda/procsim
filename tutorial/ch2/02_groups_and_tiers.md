# Groups & Architecture Tiers

Complex systems are typically structured into distinct tiers, such as **Edge & Ingress**, **Application Services**, and **Persistence**.

ProcSim allows you to group related nodes together and style visual boundary containers around them.

---

## Defining Groups

Define `groups:` at the root of your YAML:

```yaml
groups:
  ingress:
    title: "Edge & Ingress Tier"
    direction: tb        # Top-to-bottom column inside this group
    style:
      bg: "#f0f9ff"      # Soft light sky background
      border: "#0284c7"  # Sky border stroke
      border_width: 2.0
      radius: 14.0       # Rounded corners

  compute:
    title: "Service Mesh"
    direction: tb
    style:
      bg: "#faf5ff"      # Soft light purple background
      border: "#9333ea"  # Purple border stroke
      border_width: 2.0
      radius: 14.0

  storage:
    title: "Data Persistence"
    direction: tb
    style:
      bg: "#ecfdf5"      # Soft light emerald background
      border: "#059669"  # Emerald border stroke
      border_width: 2.0
      radius: 14.0
```

---

## Assigning Nodes to Groups

Assign each node to its respective tier using the `group:` property in `graph:`:

```yaml
graph:
  - name: client
    node_type: client
    group: ingress
    links: [gateway]

  - name: auth
    node_type: auth_svc
    group: compute
    links: [users_db]
```

---

## Compound Multi-Direction Layouts

The layout engine automatically computes:
1. **Macro Layout**: Arranges the groups from left to right (`lr`).
2. **Micro Layout**: Stacks the inner nodes within each group from top to bottom (`tb`).
3. **Bounding Boxes**: Renders styled container boxes behind the grouped nodes with header labels.
