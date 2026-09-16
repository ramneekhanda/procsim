# 5.6 AWS Cloud Architecture Library

In this lesson, we compose a full multi-tier production architecture on AWS using reusable Process Libraries (`plibs`), hierarchical layout tiers, and group containers.

```
[ Route 53 ] ---> [ ALB ] ---> [ ECS Cluster (Tasks 1 & 2) ] ---> [ DynamoDB ]
                                                                ---> [ SQS Queue ]
```

---

## 1. Composing AWS Libraries

```yaml
imports:
  - from: "plibs:aws/compute"
  - from: "plibs:aws/database"
  - from: "plibs:aws/messaging"
```

---

## 2. Multi-Tier Layout with Groups
- **Edge Tier**: Route 53 DNS routing.
- **Ingress Tier**: Application Load Balancer (ALB).
- **Compute Tier**: ECS Fargate container group with internal vertical stacking.
- **Persistence Tier**: DynamoDB NoSQL tables and SQS asynchronous message queues.
