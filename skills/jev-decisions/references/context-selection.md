# Selecting Context to Retain

Use this recipe to decide which tool results and observations from a long work history should remain, be shortened, or leave the active working context. This recipe never deletes the original conversation, logs, or files.

## Pin required information first

The caller must retain these items without asking Jev to select them:

- The user's objective, constraints, permissions, and outstanding requests.
- The selected design and its rationale or material tradeoffs.
- Reproduction steps and the conditions used to compare results.
- Current unresolved items, known risks, and remaining verification.

The caller may summarize them as `pinned_context`, but Jev does not decide whether they remain.

## Forming candidates

Split the rest of the history into small candidates whose source and meaning remain clear. Include current successes and also observations that show recurrence conditions, rejected hypotheses, or earlier failures. A past failure that conflicts with a current success may reveal an environment difference or instability.

Ask one Noul question per candidate about whether it is needed for the next action or unresolved decision, and put the questions in one Batch. To compare candidate importance on ordered levels, use one Score per candidate with identical criteria. Because several candidates may all be needed, Choice is usually inappropriate.

## Batch example

```json
{
  "state": {
    "next_decision": "Decide whether the retry fix is ready for submission.",
    "pinned_context": {
      "user_constraint": "Do not change the public API format.",
      "design_reason": "Preserve the default retry limit and honor only explicit configuration.",
      "reproduction": "After a transient error, 4 executions were expected and 1 was observed.",
      "unresolved": "The full test suite and static analysis have not run."
    },
    "candidates": {
      "focused_success": "After the fix, the focused retry test passed.",
      "past_failure": "Before the fix, the same test failed with 1 execution when 4 were expected.",
      "unrelated_format": "A documentation formatting test failed because of a newline difference but passed on rerun.",
      "discarded_api_hypothesis": "The API returned 202 and accepted the job registration request, so the API hypothesis was rejected."
    }
  },
  "questions": {
    "keep_focused_success": {
      "type": "noul",
      "instructions": "Should `candidates.focused_success` be retained as evidence of the target behavior after the fix for `next_decision`?"
    },
    "keep_past_failure": {
      "type": "noul",
      "instructions": "Should `candidates.past_failure` remain in the working context for `next_decision` to preserve the reproduction condition and before-and-after difference?"
    },
    "keep_unrelated_format": {
      "type": "noul",
      "instructions": "Should `candidates.unrelated_format` remain in the working context because it is needed for `next_decision` about submitting the retry fix?"
    },
    "keep_discarded_api_hypothesis": {
      "type": "noul",
      "instructions": "Should `candidates.discarded_api_hypothesis` remain in the working context for `next_decision` to preserve the causal explanation and avoid repeating the same rejected investigation?"
    }
  }
}
```

## Applying the result

- Retain strongly relevant candidates with their source and observation intact.
- For uncertain candidates, have the caller decide whether a shorter summary can preserve their meaning.
- Remove low-relevance candidates only from the active working context. This does not authorize deleting the original conversation, logs, or files.
- Retain `pinned_context` regardless of the evaluation.

After selection, Codex must confirm that evidence needed to refute the current explanation and relevant past failures are still available. Never change audit-record or user-data retention based only on Jev's assessment.
