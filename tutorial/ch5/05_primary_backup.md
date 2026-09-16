# 5.5 Primary-Backup Replication

**Primary-Backup Replication** is a foundational high-availability pattern where a designated leader (Primary) handles all client write mutations, synchronously replicates state changes to standby replicas, and returns success to the client once a quorum of replicas acknowledge the write.

```
[ Client ] ---> [ Primary ] ===(Replicate)===> [ Replica 1 ]
                     |      ===(Replicate)===> [ Replica 2 ]
                     v
             [ ACK to Client ]
```

---

## 1. Replication Workflow
1. **Client Write**: Client submits a write operation `set key=val` with monotonic transaction ID.
2. **Primary State Update & Replicate**: Primary records the value into its local log and fans out a `REPLICATE` message to all followers.
3. **Replica Acknowledgements**: Each backup replica updates its local copy and replies with `ACK`.
4. **Client Confirmation**: Once all replicas confirm, the Primary returns a `SUCCESS` response to the client.
