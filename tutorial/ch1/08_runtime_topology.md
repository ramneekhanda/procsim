# 8. Runtime Topology Changes

Every graph so far has had a fixed set of nodes, declared upfront in `graph:`. A
handler can also grow or reshape the *running* simulation itself - create new nodes,
remove them, and rewire links - all without touching the YAML.

---

## 1. `spawn_node()` - Creating Nodes at Runtime

```rhai
spawn_node(name, node_type, links);
```

Creates a new node instance of `node_type`, named `name`, linked to `links` (peers
that must already exist - no forward references to a node spawned later in the same
tick). Its `on_init` runs immediately, so `log()`/`draw()` inside it work right away.

```yaml
node_types:
  - id: coordinator
    fn: |
      fn on_timer() {
        if state.count >= 3 { return; }
        let name = "worker_" + state.count;
        spawn_node(name, "worker", []);
        link(name);
        state.count += 1;
      }
```

`link(peer)` here is the calling node (`coordinator`) adding `peer` to its *own*
`links` - self-scoped, same as declaring `links:` in YAML. `spawn_node()` itself
never links the new node back to its caller automatically; if you want a visible
connector between them, add that with `link()` (as above) or by passing your own
`node_name` into the new node's own `links` list.

---

## 2. `despawn()` - Removing Nodes

```rhai
despawn(name);
```

Removes any node by name, including the caller itself (`despawn(node_name)`). Any
connector touching the removed node is cleaned up automatically - nothing else to do.

```yaml
node_types:
  - id: worker
    attrs:
      ticks: 3
    fn: |
      fn on_init() {
        state.age = 0;
      }
      fn on_timer() {
        state.age += 1;
        if state.age >= 3 {
          despawn(node_name);
        }
      }
```

---

## 3. `link()` / `unlink()` - Rewiring Without Spawning

Both are self-scoped the same way as `link()` above - they only ever change the
*calling* node's own `links`:

```rhai
link("peer_name");     // add peer_name to my own links
unlink("peer_name");   // remove it
```

Useful for things like round-robin routing, moving a "current leader" pointer between
peers, or detaching from a node just before it gets despawned.

---

## 4. What Actually Redraws

A YAML edit (clicking **Run**) despawns and rebuilds every node from scratch - safe,
since there's no simulation state worth preserving across an edit anyway. Runtime
`spawn_node()`/`despawn()` are cheaper: only the one node that changed gets
added/removed as an entity; every other node's position, drag state, and overlay are
left completely alone. A freshly spawned node is placed to avoid landing on top of an
already-visible node, but existing nodes never get nudged to make room - they stay
exactly where they are.

**Try it**: load this chapter's example and hit **Run**. `coordinator` spawns three
`worker` nodes one at a time, linking to each as it appears; each `worker` ages for
three ticks, logs it, then despawns itself. Watch the canvas - new nodes appear next
to (not on top of) the existing ones, and despawned ones vanish cleanly, connector and
all.
