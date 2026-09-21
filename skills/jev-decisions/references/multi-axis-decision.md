# Evaluating Multiple Decision Axes

Use one Batch when several independent judgments apply to the same state. This saves an API round trip while keeping each output typed and separately interpretable.

Useful axes include task complexity, reasoning requirement, tool complexity, operational risk, and the next action. The questions must be independently answerable from `state`: no question may refer to another answer or assume that another answer has already selected a route. Host-specific model routing is outside this recipe.

```json
{
  "state": {
    "task": "Diagnose and fix a retry regression in a Rust service.",
    "observations": [
      "A focused test reproduces one attempt instead of four.",
      "The diff changes retry-limit configuration parsing.",
      "No production data migration is involved."
    ],
    "available_actions": {
      "read_more": "Inspect parsing and retry call sites.",
      "run_test": "Run the focused test with diagnostics.",
      "inspect_diff": "Review the complete working diff.",
      "edit": "Change the implementation based on established evidence.",
      "ask_user": "Request unavailable requirements.",
      "finish": "End only if implementation and verification are complete."
    }
  },
  "questions": {
    "task_complexity": {
      "type": "score",
      "instructions": "Rate the task's implementation complexity using only the supplied task and observations.",
      "criteria": ["localized", "several connected components", "cross-system or architectural"]
    },
    "reasoning_requirement": {
      "type": "score",
      "instructions": "Rate how much causal reasoning is still required before a justified edit can be made.",
      "criteria": ["cause directly established", "one discriminating check remains", "multiple unresolved hypotheses"]
    },
    "tool_complexity": {
      "type": "score",
      "instructions": "Rate the complexity of the tools needed for the next useful observation.",
      "criteria": ["one read-only command", "several local commands", "external or state-changing operation"]
    },
    "risk": {
      "type": "score",
      "instructions": "Rate the risk of acting on the current evidence before gathering more information.",
      "criteria": ["low and easily reversible", "material but locally reversible", "high impact or difficult to reverse"]
    },
    "next_action": {
      "type": "choice",
      "instructions": "Select one currently available action that is most likely to advance the task using only the supplied state.",
      "criteria": {
        "read_more": "Inspect parsing and retry call sites.",
        "run_test": "Run the focused test with diagnostics.",
        "inspect_diff": "Review the complete working diff.",
        "edit": "Change the implementation based on established evidence.",
        "ask_user": "Request unavailable requirements.",
        "finish": "End only if implementation and verification are complete."
      }
    }
  },
  "profile": "interactive"
}
```

Interpret each raw distribution and each `policy.answers.<question_id>` separately. Do not derive a model or session route from these values. Codex combines the independent judgments with repository facts, user constraints, and permissions. When an axis is uncertain, collect evidence specific to that axis, update `state`, and reevaluate only the decisions that need the new evidence.
