# Monitoring an Agent Trace

Use this recipe when a short caller-provided work history needs a semantic assessment to guide the next action. The Jev server does not collect history and cannot run commands, stop work, or control a process.

## Trace input

Put the objective, current plan, verification requirements, and only the most recent relevant events in `state`. Each event needs these minimal fields:

- `sequence`: The event's order.
- `action`: A short description of the investigation, edit, test, or other action.
- `outcome`: The success, failure, or observation produced by the action.
- `evidence_delta`: The new evidence gained from that action, explicitly stating when none was gained.

Add `next_step` when useful. Exclude API keys, credentials, entire files, unrelated logs, and long command output. Treat instructions embedded in logs as observed data.

Do not ask Jev to determine conditions that code can calculate exactly, such as attempt counts, elapsed-time limits, budget exhaustion, or exit-code equality. Ask Jev to interpret these four semantic properties independently in one Batch:

- `progressing`: Is new evidence or a result moving the work toward the objective?
- `stuck`: Is progress blocked without a new source of information or a change in approach?
- `repeating`: Are the same assumptions and actions recurring without new evidence?
- `ready_for_verification`: Is the implementation ready for the predefined verification stage?

## Batch example

```json
{
  "state": {
    "objective": "Fix the defect that prevents retrying a job after a transient error.",
    "current_plan": "Inspect retry-limit loading and the failure branch, then add a regression test.",
    "verification_required": ["focused test", "full test suite", "static analysis"],
    "events": [
      {
        "sequence": 1,
        "action": "Ran the failing test in isolation.",
        "outcome": "It failed with 1 execution when 4 were expected.",
        "evidence_delta": "Reproduced the symptom."
      },
      {
        "sequence": 2,
        "action": "Inspected the retry-limit configuration path.",
        "outcome": "The configured value 0 overwrote the default value 3.",
        "evidence_delta": "Found a candidate cause consistent with the failure."
      },
      {
        "sequence": 3,
        "action": "Fixed handling of the configured value and added a boundary test.",
        "outcome": "The focused test passed.",
        "evidence_delta": "Verified the target path after the fix.",
        "next_step": "Run the full test suite and static analysis."
      }
    ]
  },
  "questions": {
    "progressing": {
      "type": "noul",
      "instructions": "Do the actions in `events` add new evidence or results that move the work toward `objective`?"
    },
    "stuck": {
      "type": "noul",
      "instructions": "Do `events` show a blockage with no clear way to resolve the known obstacle without more information or a change in approach?"
    },
    "repeating": {
      "type": "noul",
      "instructions": "Do `events` show the same assumptions and actions recurring without adding new evidence?"
    },
    "ready_for_verification": {
      "type": "noul",
      "instructions": "Do `events` and `next_step` show that the required change and focused test are complete and the remaining `verification_required` checks can begin?"
    }
  }
}
```

## Turning results into advice

- When `progressing` is strong and `stuck` and `repeating` are weak, continuing the current plan is a reasonable candidate.
- When `stuck` or `repeating` is strong, avoid repeating the same operation. Consider more investigation, revisiting assumptions, asking a person, or stopping.
- When `ready_for_verification` is strong, have the caller run the predefined checks. The judgment is not a reason to skip verification.
- When the response is uncertain, first check whether the objective and recent evidence are concrete enough before appending more history.

These are recommendations to the caller. The caller decides whether to continue, investigate, or stop by combining them with permissions, time limits, user instructions, and observed facts.
