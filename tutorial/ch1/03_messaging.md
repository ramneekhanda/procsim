# 3. Nodes Talking To Each Other

A node's `links` list names the other nodes it's allowed to message. Sending is one
call:

```
send(to: &str, msg: dictionary);
```

`msg` is any Rhai map (`#{ ... }`) - put whatever fields your handlers need in it. Two
fields are special, read by the canvas itself rather than your script:

- **`display`** - a short string shown on the message's icon while it travels.
  Cosmetic only - it's never a substitute for real data. Put whatever a receiving
  node actually needs to branch on or process in its own key(s) (`type`, `id`,
  `payload`, ...); don't make a receiver parse `msg.display` to recover a value,
  even when the display text happens to look like the data.
- **`icon`** - an id from the graph's `icons:` registry (chapter 4), for a custom
  message icon instead of the default.

The receiving node's `on_msg(msg)` handler runs when the message arrives - `msg` is
exactly the map you sent, plus a `from` field the engine fills in with the sender's
name (see `msg.from` used to reply, below).

```yaml
graph_defn:
  graph:
    - name: ping
      node_type: ping
      links: [pong]
    - name: pong
      node_type: pong
      links: [ping]
  node_types:
    - id: ping
      fn: |
        fn on_timer() {
          send("pong", #{ type: "PING", display: "PING" });
        }
        fn on_msg(msg) {
          log("ping got: " + msg.type);
        }
      attrs:
        ticks: 4
    - id: pong
      fn: |
        fn on_msg(msg) {
          log("pong got: " + msg.type);
          send("ping", #{ type: "PONG", display: "PONG" });
        }
```

Notice `pong` has no `on_timer` at all - it only ever reacts to `on_msg`, which is a
perfectly normal shape for a node type (a passive server, say).

**Try it**: load this chapter's example, run it, and watch a message icon travel back
and forth between the two nodes on the canvas while the Logs panel shows both sides
of the conversation.

A message takes a few real seconds to arrive after `send()` is called (it animates
along the connector) - `on_msg` fires once it visually reaches the destination, not
the instant `send()` is called.
