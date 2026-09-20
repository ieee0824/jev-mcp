# Checking Evidence Sufficiency Before Submission

Use this recipe to find missing evidence before declaring a fix ready for submission. One successful run after a change is not sufficient on its own. This assessment helps prepare for review; it does not replace passing tests, code review, or user authorization.

## Dimensions to assess

Ask independent questions about at least these dimensions when they apply:

- **Reproduction:** Was the original failure reproduced before the fix, confirming the reported symptom?
- **Causality:** Do observations and diagnostics support a causal link between the failure and the changed area?
- **Post-fix verification:** Does the original reproduction succeed after the fix?
- **Regression test:** Can the test detect recurrence, such as failing when the fix is removed and passing when restored?
- **Impact coverage:** Were the affected paths and boundary conditions checked?

Question IDs only map answers. Each `instructions` field must state the target, required evidence, and condition for Yes in full.

## Procedure

1. Summarize commands, exit states, material observations, and their relationship to the change in `state`. Mark unperformed checks as unperformed instead of filling gaps with assumptions.
2. Ask one Noul question per dimension in a single Batch. If using Score, keep dimensions separate and use identical levels.
3. Do not mark the work complete when a dimension is strongly No, when a Noul value near 0.5 shows uncertainty, or when evidence cannot be tied to the dimension. Have Codex gather the missing logs, tests, comparisons, or impact-path evidence.
4. After adding evidence, reevaluate only the dimensions that changed. Codex must still inspect the final diff and actual test results.

Even if Jev rates every dimension highly, do not automatically submit, approve, or merge. Complete the repository's review and authorization requirements separately.

## Batch example

```json
{
  "state": {
    "reported_failure": "The job is not retried after a transient error.",
    "before_fix": {
      "command": "cargo test worker_retries_transient_failure",
      "result": "failed: expected 4 attempts, got 1"
    },
    "diagnosis": "The retry limit was 0, so the branch terminated after failure.",
    "change": "Restored the retry limit to 3 and added a boundary test for a configured value of 0.",
    "after_fix": {
      "focused_test": "passed",
      "full_suite": "passed"
    },
    "regression_check": "The added test failed when the fix was temporarily reverted and passed when it was reapplied.",
    "reviewed_paths": ["normal success", "transient error", "permanent error", "zero limit"]
  },
  "questions": {
    "reproduced": {
      "type": "noul",
      "instructions": "Is `before_fix` sufficient evidence that the same symptom as `reported_failure` was reproduced before the fix?"
    },
    "causal": {
      "type": "noul",
      "instructions": "Do `before_fix` and `diagnosis` support a causal link between the target of `change` and `reported_failure`?"
    },
    "fixed": {
      "type": "noul",
      "instructions": "Does `after_fix` show that the same failure was resolved after the fix and that the related tests passed?"
    },
    "regression": {
      "type": "noul",
      "instructions": "Does `regression_check` show that the added test can detect recurrence of this defect?"
    },
    "scope": {
      "type": "noul",
      "instructions": "Are `reviewed_paths` and `after_fix` sufficient evidence that the main paths and boundary conditions affected by `change` were checked?"
    }
  }
}
```
