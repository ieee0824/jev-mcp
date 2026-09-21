# Selecting a Next Action or Prepared Tool Call

Use this recipe when Codex has a small finite set of actions that it can actually perform. Codex defines the candidates from the current plan and permissions. Jev selects one candidate ID with Choice. Jev never invents an action, creates arguments, or executes a tool.

## Next-action selection

Describe the task, current observations, unresolved question, and recent attempts in `state`. Put only feasible actions in `criteria`, using stable IDs such as `read_more`, `run_test`, `inspect_diff`, `edit`, `ask_user`, and `finish`. Include an evidence-gathering candidate when the available state may be insufficient. Do not include `finish` unless the stated completion and verification conditions could already be satisfied.

```json
{
  "state": {
    "task": "Find why the retry test fails after the configuration change.",
    "observations": [
      "The focused test expects four attempts but observed one.",
      "The working tree changes retry-limit parsing."
    ],
    "completion_conditions": ["cause supported by evidence", "fix verified by tests"]
  },
  "instructions": "Select the single most useful feasible next action for progressing this task from the current observations.",
  "criteria": {
    "read_more": "Read the retry-limit parsing and call sites.",
    "run_test": "Run the focused retry test with diagnostics.",
    "inspect_diff": "Inspect the current diff for changes affecting retry behavior.",
    "edit": "Edit code using a cause already supported by the observations.",
    "ask_user": "Ask for information that cannot be obtained from the repository or tests.",
    "finish": "Stop because every completion condition is already supported by observed evidence."
  },
  "profile": "interactive"
}
```

Read the returned `answers.result.choice` as the candidate ID. Codex checks that the candidate is still feasible, executes it with the normal tool and permission rules, and records the actual outcome. A choice of `finish` is advisory and does not replace required verification.

## Tool selection from fully prepared calls

Prepare complete tool calls before asking Jev. Give each call an opaque candidate ID and include the exact purpose and already determined arguments in the candidate description. Jev returns only the ID; Codex maps that ID back to the locally held call and executes it. Never turn Jev output into new shell text or arguments.

```json
{
  "state": {
    "task": "Choose the next diagnostic for retry behavior.",
    "evidence": "The failure began after retry configuration changed.",
    "prepared_calls": {
      "candidate_a": {"tool": "grep", "arguments": ["retry", "src"]},
      "candidate_b": {"tool": "cargo_test", "arguments": ["retry"]},
      "candidate_c": {"tool": "git_diff", "arguments": []}
    }
  },
  "instructions": "Select the prepared call that is most likely to add discriminating evidence for the task. Return its candidate ID through Choice.",
  "criteria": {
    "candidate_a": "Inspect retry references using the prepared grep call.",
    "candidate_b": "Observe the focused behavior using the prepared test call.",
    "candidate_c": "Inspect the current changes using the prepared diff call."
  },
  "profile": "interactive"
}
```

Keep the executable call registry on the host. Recheck permissions and current state before execution. If policy says `reevaluate`, the candidates are close, or evidence is missing, gather one discriminating observation and submit an updated state instead of repeating the same choice.
