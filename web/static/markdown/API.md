# procsim
A distributed process simulator.

## usage
Diagram is coded in yaml which has the following schema:
```yaml
graph_defn:
  graph:
    # your graph nodes and links
  graph_attrs:
    # overall graph attributes
  node_types:
    # various types of nodes and their behaviour
```

## api
```rust
fn send(to: &str, msg: dictionary) {
    // send a message to a process
}
```
