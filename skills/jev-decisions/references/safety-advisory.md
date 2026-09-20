# Advisory Assessment of Risky Operations

Use this recipe when an operation is ambiguous and an additional assessment could clarify which risks need attention before or after existing permission checks. Jev's response is an advisory signal. It does not replace OS or MCP-client permissions, sandboxing, user confirmation, or approval procedures. Jev cannot authorize, deny, or execute an operation.

## Separate pre-operation intent from post-operation observation

Before the operation, put only the planned intent, target, data scope, external boundary, expected impact, and recovery method in `state`. Assess these dimensions independently in one Batch:

- Whether the operation is read-only and changes no state.
- Whether every change is fully and realistically reversible.
- Whether it transmits data or a message to an external party.
- Whether it deletes or irrecoverably overwrites files, history, or resources.

After the operation, do not reuse the plan as though it were observed fact. Build a new `state` from the operation actually executed, its exit state, changed targets, external response, and observed recoverability. Assess divergence from intent, unexpected changes, and unverified impact independently.

## Pre-operation example

```json
{
  "state": {
    "phase": "before",
    "intent": "Create a GitHub pull request to share the working branch diff.",
    "target": "The current repository's working branch and pull request body.",
    "planned_actions": ["Push the branch", "Create a pull request through the GitHub API"],
    "data_scope": "Committed diff, branch name, and pull request body.",
    "recovery": "The pull request can be closed; pushed commits remain on the remote."
  },
  "questions": {
    "read_only": {
      "type": "noul",
      "instructions": "Is this a read-only operation that changes neither local state nor external-service state?"
    },
    "reversible": {
      "type": "noul",
      "instructions": "Can every planned state change be fully and realistically reversed by the stated `recovery` method?"
    },
    "external_send": {
      "type": "noul",
      "instructions": "Do `planned_actions` transmit the information in `data_scope` to the external GitHub service?"
    },
    "deletion": {
      "type": "noul",
      "instructions": "Do `planned_actions` delete or irrecoverably overwrite files, history, branches, or external resources?"
    }
  }
}
```

A high `external_send` result in this example is neither permission nor prohibition. The caller must use the existing approval process to confirm that the user authorized the push and PR creation and that the outbound content matches the intended scope.

## Post-operation example

```json
{
  "state": {
    "phase": "after",
    "intended": ["Push the working branch", "Create one pull request"],
    "observed": {
      "command_status": "success",
      "changed_targets": ["remote working branch", "pull request #24"],
      "external_response": "Received the pull request URL.",
      "deletions": [],
      "unexpected": []
    }
  },
  "questions": {
    "matches_intent": {
      "type": "noul",
      "instructions": "Do `observed.changed_targets` and `external_response` show that only the operations within `intended` were performed?"
    },
    "unexpected_change": {
      "type": "noul",
      "instructions": "Does `observed` show any unplanned external transmission, state change, deletion, or hard-to-reverse impact?"
    },
    "needs_follow_up": {
      "type": "noul",
      "instructions": "Does `observed` contain any unverified impact that requires further checking or recovery before considering the result complete?"
    }
  }
}
```

## Uncertainty and errors

When probabilities are close, a Noul value is near 0.5, target information is missing, or the API returns an error, never default to automatic authorization. Before an operation, investigate the target and impact and return to any required permission or user-confirmation step. After an operation, perform read-only checks; if an unexpected change is possible, stop further mutation until the facts are known.

A low-risk assessment does not authorize an operation that requires explicit permission. A high-risk assessment does not authorize deletion or rollback without factual verification. The caller makes the final decision from actual permissions, user instructions, sandbox boundaries, and approval results.
