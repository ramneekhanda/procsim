# procsim
A distributed process simulator.

procsim allows you to define a graph with nodes and edges and program each node of the graph using a scripting language (rhai).
Each node gets three callbacks:
```
- `on_init` which is called when the node is created
- `on_tick` which is called every defined tick for the node
- `on_msg` which is called when a message is received by the node
```
Each node upon those events can do following things:
```
- send a message to another node
- change its state
- log something
```
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
fn send(to: &str, msg: dictionary);
fn log(msg: &str);
```
