# 5.1 Introduction to Process Libraries (plibs)

Process Libraries (`plibs`) are modular, reusable packages containing node types, Rhai handler scripts, templates, and icon registries that can be shared across simulations.

---

## 1. Importing Libraries

Use the top-level `imports:` block:

```yaml
imports:
  - from: "plibs:aws/compute"
  - from: "plibs:aws/database"
```

---

## 2. Using Imported Types in Your Graph

Once imported, all node types from the library become immediately available in `graph`:

```yaml
graph:
  - name: my_lambda
    node_type: lambda_func
    links: [my_dynamo]
  - name: my_dynamo
    node_type: dynamodb
    links: []
```

You can seamlessly connect custom local nodes with imported library nodes.
