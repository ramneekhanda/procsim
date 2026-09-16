# 5.3 Load Balancer with Round-Robin

A **Load Balancer** distributes incoming client traffic across a pool of available backend workers to maximize throughput and prevent overloading any single instance.

```
                  ┌---> [ Worker 1 ]
[ Client ] ---> [ LB ] ---> [ Worker 2 ]
                  └---> [ Worker 3 ]
```

---

## 1. Round-Robin Dispatching

The load balancer maintains an index pointer into its worker list:

```rhai
fn on_msg(msg) {
  if msg.type == "REQ" {
    let target = links[state.idx % links.len()];
    state.idx += 1;
    send(target, msg);
  }
}
```

---

## 2. Worker Processing & Response Relay

Workers process the request and reply to the load balancer (`msg.from`), which relays the response back to the original client (`msg.reply_to`).
