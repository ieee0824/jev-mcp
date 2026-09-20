# Ranking Incident Hypotheses

Use this recipe to select the next diagnostic from several possible causes. Codex generates the hypotheses, Jev ranks them against the current evidence, and Codex performs diagnostics and determines the final cause.

## Procedure

1. From logs, diffs, and reproduction results, have Codex form hypotheses that can be distinguished from one another. For each hypothesis, identify the expected observation and a diagnostic that could refute it.
2. When several hypotheses may be true at once, put one Noul question per hypothesis in a single Batch. If ordered diagnostic priority is more useful, ask one Score question per hypothesis with identical levels. Use Choice only after confirming that the categories are mutually exclusive.
3. Select the next diagnostic by combining the returned probabilities or scores with the diagnostic's cost and safety. Do not establish a cause from ranking alone.
4. Run a test or observation that distinguishes the leading hypothesis, then use the observed result to reject or support it. Base the final conclusion on the diagnostic evidence.
5. Return to evidence gathering and generate a new hypothesis set when all Noul values are low, results are close or uncertain, the hypotheses cannot be distinguished with current evidence, or a diagnostic contradicts the leading hypothesis. These are signals that the true cause may be absent from the candidate set.

When thresholds are useful, use values validated for the project. Never use a fixed threshold as proof of causality.

## Batch example

This example evaluates each hypothesis independently against the same evidence. Each `instructions` field states the complete hypothesis and condition rather than relying on the question ID.

```json
{
  "state": {
    "change": "Changed the worker retry limit from 3 to 0.",
    "failure": "There was 1 execution after a transient error; 4 were expected.",
    "observations": [
      "The API accepted the job and returned 202.",
      "The worker started the first execution."
    ]
  },
  "questions": {
    "retry_limit": {
      "type": "noul",
      "instructions": "Using only the current evidence, including `observations`, assess whether the retry limit becoming 0 because of `change` could have caused `failure`."
    },
    "api_registration": {
      "type": "noul",
      "instructions": "Using only the current evidence, including `observations`, assess whether the API failing to register the job correctly could have caused `failure`."
    },
    "worker_crash": {
      "type": "noul",
      "instructions": "Using only the current evidence, including `observations`, assess whether the worker crashing after the first execution could have caused `failure`."
    }
  }
}
```

Even if `retry_limit` ranks first, verify causality by restoring the previous limit or running a focused retry-branch test. Low scores for other hypotheses do not prove the leading one.
