# 5.4 Two-Phase Commit (2PC)

**Two-Phase Commit (2PC)** is a distributed consensus algorithm that ensures atomic transaction execution across multiple independent databases or microservices.

```
                   ┌---> [ Database 1 ]
[ Coordinator ] ---┤
                   └---> [ Database 2 ]
```

---

## The Protocol Phases

### Phase 1: Prepare (Voting)
1. **Coordinator** generates a transaction ID (`tx_id`) and broadcasts a `PREPARE` message to all cohort databases.
2. Each **Cohort** checks its local locks and state, then replies with `VOTE_COMMIT` or `VOTE_ABORT`.

### Phase 2: Commit (Decision)
1. If **all** cohorts voted `VOTE_COMMIT`, the coordinator broadcasts `GLOBAL_COMMIT`.
2. If **any** cohort voted `VOTE_ABORT` or timed out, the coordinator broadcasts `GLOBAL_ABORT`.
3. Cohorts apply the final decision locally.
