# 1.7 Interactive Narration (`explain`)

Simulations can pause execution and pop up instructional speech bubbles anchored directly to relevant nodes. This lets authors explain step-by-step state transitions, error recoveries, and consensus milestones.

---

## 1. Calling `explain(key, text)`

Rhai scripts trigger narration bubbles via `explain(key, markdown_text)`:

```rhai
explain(
  "step_1_init",
  "**Primary Node**: Initialized leadership and waiting for incoming client transactions."
);
```

### Key Rules
- **Automatic Simulation Pause**: Whenever a bubble appears, the simulation timer pauses automatically (`Time<Virtual>` freezes).
- **Deduplication by `key`**: Each unique `key` string only ever triggers once per graph session. Calling `explain("step_1_init", ...)` repeatedly on every tick will not spam the user.
- **Dismiss to Resume**: Clicking the **Continue** button or background backdrop dismisses the current bubble and resumes the simulation.
- **Queueing**: If multiple nodes trigger bubbles simultaneously, they are queued and displayed in sequence.

---

## 2. Formatting Narration Text

Narration bubbles support Markdown formatting, including:
- Bold labels: `**Leader**`
- Inline code: `` `state.term = 1` ``
- Lists and bullet points for multi-step descriptions.
