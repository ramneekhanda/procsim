# 5.2 Token Ring Mutual Exclusion

In a **Token Ring** network, nodes form a logical unidirectional ring. A single unique message (the **Token**) circulates continuously. Only the node holding the token is permitted to enter the critical section or transmit data.

```
       [ Node 1 ] ----> [ Node 2 ]
           ^                 |
           |                 v
       [ Node 4 ] <---- [ Node 3 ]
```

---

## 1. Protocol Rules
1. **Ring Topology**: Each node $i$ links only to node $(i + 1) \pmod N$.
2. **Token Circulation**: Node 1 initiates the token at startup.
3. **Execution**: When a node receives the token:
   - It acquires mutual exclusion.
   - It performs its critical section work and logs its state.
   - It renders a visual badge (`HOLDING TOKEN`).
   - It passes the token downstream to its successor.
