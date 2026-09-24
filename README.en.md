# jev-mcp

[日本語](README.md) | [English](README.en.md)

[![CI](https://github.com/ieee0824/jev-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/ieee0824/jev-mcp/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A lightweight MCP server for using **Jev**, the decision model from [TypeSafe AI](https://docs.typesafe.ai/introduction), with Codex. It is written in Rust and communicates with MCP clients over standard input and output (stdio).

Codex handles requirements, code investigation, editing, and test execution. During that work, it can delegate classification, relevance checks, candidate selection, and evaluation to Jev. Codex acts as System 2 for investigation and execution; Jev acts as System 1 for fast judgment over a closed decision space.

```text
Codex: investigate and plan
  ↓ relevant evidence and a focused question
Jev: classify, select, or evaluate
  ↓ typed answer, probability distribution, and deterministic handling guidance
Codex: edit, test, and take the next action
```

## Features

The server provides four MCP tools.

| Tool | Purpose | Main result |
| --- | --- | --- |
| `jev.noul` | Judge whether a condition holds | Yes probability `noul` from 0 to 1 |
| `jev.choice` | Select one named option | `choice`, `probabilities`, and `confidence` |
| `jev.score` | Rate one dimension on ordered levels | `score`, `legend`, `probabilities`, and `confidence` |
| `jev.batch` | Evaluate several questions against shared state | An answer for each question ID |

Example uses include:

- deciding whether a file is relevant to the current defect;
- assessing whether a test failure is related to a recent change;
- selecting which subsystem to investigate first;
- assessing whether a change needs more review or testing; and
- evaluating whether a supplied work trace shows progress or repeated attempts.

The server does not read repository files, collect agent history, or execute tools. The caller supplies evidence as `state` and defines the finite candidate set. Jev is not used as a free-form planner or command generator.

## Requirements

- Rust 1.88 or later
- A TypeSafe API key
- An MCP client with stdio support, such as Codex

Get an API key from the [TypeSafe console](https://console.typesafe.ai/). Evaluations call the TypeSafe API and may incur usage charges.

## Build

```sh
cargo build --release --locked
```

The executable is created at `target/release/jev-mcp`.

With no arguments, the executable runs as a stdio MCP server and waits for MCP messages on standard input. An MCP client normally launches it. The `eval validate` and `eval run` subcommands provide the evaluation CLI described below.

## Connect from Codex

Add the following configuration to `~/.codex/config.toml`. Replace `command` with the absolute path to the built executable. Update the existing entry if `[mcp_servers.jev]` is already present.

```toml
[mcp_servers.jev]
command = "/absolute/path/to/jev-mcp/target/release/jev-mcp"
tool_timeout_sec = 70

[mcp_servers.jev.env]
TYPESAFE_API_KEY = "your_typesafe_api_key"
```

Replace `your_typesafe_api_key` with the actual key, save the file, and restart Codex. Do not commit a configuration file containing the key.

To inherit environment variables instead, use:

```toml
[mcp_servers.jev]
command = "/absolute/path/to/jev-mcp/target/release/jev-mcp"
env_vars = [
  "TYPESAFE_API_KEY",
  "TYPESAFE_MODEL",
  "JEV_PROFILE",
  "JEV_POLICY_ACCEPT_THRESHOLD",
  "JEV_POLICY_VERIFY_THRESHOLD",
  "JEV_POLICY_OPEN_CHOICES",
  "JEV_TELEMETRY",
]
tool_timeout_sec = 70
```

In this case, set `TYPESAFE_API_KEY` in the environment of the process that launches Codex. GUI applications may not inherit variables from a terminal session.

The server reads environment variables at startup. It **does not load `.env` files automatically**. See the [official Codex MCP documentation](https://developers.openai.com/codex/mcp/) for more configuration details.

### Environment variables

| Name | Description |
| --- | --- |
| `TYPESAFE_API_KEY` | Bearer token for API authentication. Tools can still be listed when it is missing or empty, but evaluation calls return an error. |
| `TYPESAFE_MODEL` | Default model. Defaults to `jev-latest`; the `model` argument can override it per call. |
| `JEV_PROFILE` | Default execution profile: `reliable` (default) or `interactive`. The `profile` argument can override it per call. |
| `JEV_POLICY_ACCEPT_THRESHOLD` | Minimum confidence or certainty for an `accept` disposition. Defaults to `0.90`. |
| `JEV_POLICY_VERIFY_THRESHOLD` | Minimum value for a `verify` disposition. Defaults to `0.60` and must not exceed the accept threshold. |
| `JEV_POLICY_OPEN_CHOICES` | Comma-separated Choice names that always produce `reevaluate`. Defaults cover `unknown`, `other`, `unclear`, insufficient-information, and none-of-the-above names. Set an empty string to disable this rule. |
| `JEV_TELEMETRY` | Set to `1` or `true` to write input-free call statistics as JSONL to stderr. Disabled by default. |

### Call telemetry

With `JEV_TELEMETRY=1`, the server writes one JSON object per tool call to stderr. Each JSONL record has `schema_version: 1` and `event: "jev_call"`, plus the tool name, question count, execution profile, elapsed time, attempt count, retry count, timeout count, requested and resolved models, token counts, and the structured error `kind` on failure. Stable event names and schema versions keep the format suitable for a future aggregation CLI.

Telemetry never records `state`, `instructions`, `criteria`, probability distributions, or API keys. It does not write telemetry to stdout, which is reserved for MCP traffic.

## Using the tools

### Using the Codex Skill

The repository includes the [jev-decisions Skill](skills/jev-decisions/SKILL.md). It teaches Codex how to choose a suitable judgment, batch questions, interpret results, and gather more evidence.

The Skill instructions, references, JSON examples, and UI metadata are written in English for reuse. Requests to the Skill and evidence sent to Jev may use other languages.

Copy `skills/jev-decisions` into your personal Skill directory, normally `~/.codex/skills/`. If `CODEX_HOME` is set, use its `skills/` directory. Then invoke it like this:

```text
$jev-decisions Evaluate whether this test failure is related to the diff and narrow down what to inspect next.
```

The Skill and MCP server are separate components. Configure the MCP connection and API key before using the Skill.

### Common input and output

- `state`: Information to evaluate. It may be a string, object, or array.
- `instructions`: The question or condition to judge. It may be a string, object, or array.
- `model`: Optional model override. The server default is used when omitted.
- `profile`: Optional execution profile, `reliable` or `interactive`. It is MCP execution metadata and is not sent as judgment evidence to the API.
- `criteria`: A Choice option map, an ordered Score level array, or optional Yes/No descriptions for Noul.

Unknown arguments are rejected. Input shapes follow the [TypeSafe API reference](https://docs.typesafe.ai/api).

On success, the server preserves the API's `model`, `answers`, and `usage` fields and adds its own `policy` assessment. The same object is returned through MCP `structuredContent` and as JSON text in `content`.

Single-tool answers appear under `answers.result`. `jev.batch` preserves the question IDs supplied by the caller.

The JSON snippets below are tool argument objects. The MCP client wraps them in a `tools/call` message.

### Yes/No judgment: `jev.noul`

```json
{
  "state": "The test passed.",
  "instructions": "Does the state explicitly say that the test passed?"
}
```

The returned `noul` value is the probability of Yes.

- Near 1: strong Yes
- Near 0: strong No
- Near 0.5: uncertain

Noul has no separate `confidence` field. Optional `criteria.true` and `criteria.false` values can describe the Yes and No conditions.

### Selecting an option: `jev.choice`

`criteria` is a map containing 1 to 255 named options. Descriptions may be strings, objects, or arrays. Use `null` when an option needs no description.

```json
{
  "state": "The job retry test failed after the worker retry count changed.",
  "instructions": "Which subsystem should be investigated first?",
  "criteria": {
    "worker": "Job execution and retry handling",
    "api": "HTTP request handling",
    "unknown": "The evidence is insufficient"
  }
}
```

Example response values below are illustrative, not measured results.

```json
{
  "model": "jev-example",
  "answers": {
    "result": {
      "type": "choice",
      "choice": "worker",
      "probabilities": {
        "worker": 0.9,
        "api": 0.05,
        "unknown": 0.05
      },
      "confidence": 0.75
    }
  },
  "usage": { "input_tokens": 200, "output_tokens": 30 },
  "policy": {
    "version": 1,
    "answers": {
      "result": { "disposition": "verify", "reason": "moderate_confidence" }
    }
  }
}
```

Include an `unknown`, `other`, or equivalent option when the candidate set may not cover every case.

### Scoring an ordered dimension: `jev.score`

Provide 2 to 10 descriptions in `criteria`, ordered from the lowest level to the highest. Each level may be a string, object, or array.

```json
{
  "state": "CSV export fails, but JSON export remains available.",
  "instructions": "Rate the severity of the reported defect.",
  "criteria": [
    "Cosmetic only, with no functional impact",
    "Some functionality is unavailable, but a workaround exists",
    "The main workflow is blocked and no workaround exists"
  ]
}
```

`score` is the probability-weighted average of the zero-based level indices. With three levels it ranges from 0 to 2 and may be fractional. It is not always a probability between 0 and 1.

The answer also includes a `legend` mapping indices to descriptions, a `probabilities` distribution over levels, and a distribution-derived `confidence` value.

### Evaluating several questions: `jev.batch`

Use `jev.batch` for independent questions against the same `state`. A single API request may mix Noul, Choice, and Score questions. `questions` must contain at least one entry.

```json
{
  "state": {
    "diff": "Changed the worker retry limit from 3 to 0.",
    "test_failure": "worker_retries_transient_failure: expected 3 attempts, got 1"
  },
  "questions": {
    "related": {
      "type": "noul",
      "instructions": "Is `test_failure` related to `diff`?"
    },
    "subsystem": {
      "type": "choice",
      "instructions": "Which subsystem should be investigated first?",
      "criteria": {
        "worker": "Background execution and retry handling",
        "api": "HTTP request handling",
        "unknown": "The evidence is insufficient"
      }
    },
    "impact": {
      "type": "score",
      "instructions": "Rate the impact of `diff` on retry behavior.",
      "criteria": [
        "Retry behavior is unchanged",
        "Retries fail under some conditions but still occur",
        "Retries are completely disabled"
      ]
    }
  }
}
```

The answers appear under `answers.related`, `answers.subsystem`, and `answers.impact`.

Questions in a batch cannot refer to one another's answers. When one answer is needed for a later judgment, add it to a new `state` and make another call.

## Latency profiles

`reliable` is the default profile and preserves the original reliability-oriented behavior. It allows 30 seconds per attempt and 60 seconds total, retrying HTTP 429 and 529 responses at most twice. It suits evaluations where completing the judgment matters more than coding-loop latency.

`interactive` is intended for frequent calls from a coding loop. It allows about 1.5 seconds per attempt and about 3 seconds total, retrying a timeout or HTTP 429/529 response at most once. If Jev cannot answer within that budget, the server returns an explicit structured error such as `timeout`; it never fabricates a successful judgment.

Set the server default with `JEV_PROFILE` and override it per call with `profile`. The profile controls MCP-side execution and is not included in the TypeSafe API request body. Configure the MCP client's tool timeout to exceed the selected profile's total budget.

## Policy and uncertainty

Choice and Score `confidence` values describe their answer distributions. They do not guarantee correctness.

Separately from Jev's inference result, the server applies a deterministic policy that labels each answer `accept`, `verify`, or `reevaluate`. Choice and Score use `confidence`; Noul uses the distance of the Yes probability from 0.5. A configured open choice produces `reevaluate` even when confidence is high.

```text
confidence >= 0.90
    → accept the Jev judgment

0.60 <= confidence < 0.90
    → have Codex verify the result

confidence < 0.60
    → gather more evidence and reevaluate
```

These are configurable defaults. The policy never rewrites raw answers or probability distributions and does not execute tools, repeat calls, or make permission decisions. Tune thresholds against evaluation data for the specific use case. The caller decides the next action from the result's consequences and observed evidence.

Because confidence measures distribution concentration rather than correctness, do not immediately repeat an identical request after `reevaluate` or an uncertain distribution. Have Codex collect discriminating code, logs, or test results, add them to `state`, and then reevaluate.

See the [TypeSafe confidence documentation](https://docs.typesafe.ai/confidence) for more detail.

## Selecting the next action or a tool candidate

The host can define a finite set of feasible next actions such as read more, run a test, inspect the diff, edit, ask the user, or finish, then pass their IDs to `jev.choice`. Tool selection follows the same pattern: the host prepares complete calls, assigns candidate IDs, and asks Jev to return only one ID. A returned string is never treated as a generated command or argument list.

Use `jev.batch` to assess task complexity, reasoning needs, tool complexity, risk, and next action independently against the same state. Questions in the batch remain independent and cannot consume earlier answers. Reusable instructions are available in the Skill's [next-action/tool-selection recipe](skills/jev-decisions/references/action-selection.md) and [multi-axis recipe](skills/jev-decisions/references/multi-axis-decision.md). These patterns use the existing `jev.choice` and `jev.batch` tools, so the server does not add a redundant action-selection tool.

## Validating evaluation cases

Labeled evaluation cases can be stored as JSONL. Each line contains a unique `id`, a `state`, one `question`, and an `expected` value of the matching type. The same state and question constraints apply as in MCP calls.

```json
{"id":"relevance","state":"The changed function appears in the failure stack trace.","question":{"type":"noul","instructions":"Is the failure related to the change?"},"expected":{"type":"noul","min":0.7,"max":1.0}}
{"id":"owner","state":{"change":"retry worker","failure":"retry test"},"question":{"type":"choice","instructions":"Which area should be inspected first?","criteria":{"worker":"background processing","api":"HTTP handling","unknown":"insufficient information"}},"expected":{"type":"choice","allowed":["worker"]}}
{"id":"risk","state":["Changed the retry limit from 3 to 0"],"question":{"type":"score","instructions":"What is the risk of this behavior change?","criteria":["low","medium","high"]},"expected":{"type":"score","min":1.5,"max":2.0}}
```

- Noul `expected` uses `0 <= min <= max <= 1`.
- Choice `allowed` contains one or more acceptable option names.
- Score `expected` uses `0 <= min <= max <= number of levels - 1`.

The following command validates the entire file offline. It requires neither an API key nor network access. It exits nonzero for an empty file, blank line, duplicate ID, input constraint violation, or mismatched question and expected types.

```sh
cargo run --locked -- eval validate tests/fixtures/eval-valid.jsonl
```

Set the API key and use `run` to evaluate validated cases sequentially against the real TypeSafe API. This command **may incur paid API usage**.

```sh
TYPESAFE_API_KEY="your_typesafe_api_key" cargo run --locked -- eval run tests/fixtures/eval-valid.jsonl > eval-results.jsonl
```

stdout contains one JSON record per case followed by a `type: "summary"` record. Case records include the ID, `pass` / `fail` / `error` status, measured value, resolved model, elapsed time, and token usage. An expectation mismatch is `fail`; communication and API failures are `error`. Processing continues after an error. API keys and submitted `state` values are not written to the output.

The repository includes 12 synthetic Japanese coding-judgment cases in [`eval/ja-coding-minimal.jsonl`](eval/ja-coding-minimal.jsonl). See [`eval/README.md`](eval/README.md) for their purpose, validation procedure, measured-baseline workflow, and limitations.

## Communication and errors

Every tool sends the supplied evidence and questions to:

```text
POST https://api.typesafe.ai/v1/systemone
```

| Item | Behavior |
| --- | --- |
| Connection timeout | 10 seconds |
| `reliable` | 30 seconds per attempt and 60 seconds total; retry HTTP 429/529 at most twice; do not retry timeouts |
| `interactive` | About 1.5 seconds per attempt and 3 seconds total; retry HTTP 429/529 and timeouts at most once |
| Retry delay | Use a short profile-specific delay and honor a seconds-form `Retry-After` header while preserving the total budget |
| Other HTTP and network errors | Return the error without retrying |
| Redirects | Do not follow |
| Maximum response size | 8 MiB |

Invalid input, missing keys, API failures, and invalid responses are returned as MCP tool results with `isError: true`. `structuredContent.error` contains `kind`, `message`, and `retryable`, plus `status` when an HTTP response exists. `kind` is one of `validation`, `authentication`, `rate_limit`, `timeout`, `network`, `http`, or `invalid_response`. Failure policy is always `reevaluate/evaluation_failed`; the server never invents an answer. Unknown tool names produce a JSON-RPC error.

Fail-open behavior belongs to the host. If Jev is unavailable and Codex can continue safely from direct investigation, tests, and user requirements, it may proceed without Jev. It must not invent a judgment that is required for the next action or bypass permission checks. The MCP server fails explicitly; the agent fails open only when safe.

Remote error bodies and credentials are never included in error messages. stdout is reserved for MCP messages.

## Development and testing

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all
cargo build --release --locked
```

GitHub Actions uses Rust 1.88 and runs the same formatting check, Clippy, full test suite, release build, and offline validation of the Japanese evaluation fixture for pushes, pull requests, and manual runs.

Tests use a local mock HTTP server and a real stdio child process. They require neither a TypeSafe API key nor paid API calls.

Coverage includes each tool's request shape, mixed-batch MCP communication, input validation, structured and authentication errors, retries, timeouts, invalid and oversized responses, privacy-preserving telemetry, evaluation JSONL validation, the mock-API evaluation runner, the Japanese fixture, JSON examples in Skill references, and stdio initialization and shutdown.

Evaluate real-model judgment quality, cost, and latency separately with an actual API key and representative data.

## License

This project is available under the [MIT License](LICENSE).

## Design and implementation status

Codex investigates, implements, and verifies work. Jev handles small judgments within that process. Keep each question focused on one judgment, separate multiple dimensions, and combine results in caller code.

The project is experimental. It currently provides four stdio MCP tools, TypeSafe API integration, input and response validation, two latency profiles, deterministic policy guidance, structured errors, retries and timeouts, and optional JSONL telemetry. Quality tooling includes offline validation for labeled JSONL evaluation cases, a real-API evaluation runner, and a 12-case Japanese fixture.

The bundled English `jev-decisions` Skill includes references for basic tool selection, next-action and prepared-call selection, multi-axis evaluation, incident hypothesis ranking, pre-submission evidence sufficiency, work-trace monitoring, context selection, and advisory assessment of risky operations. These are recipes built from the existing four tools. They do not execute operations or implement authorization.

Repository auto-investigation, automatic trace collection, tool execution, tool-argument generation, and host-specific model or session routing for Codex, Claude, or other agents are not implemented. Future evaluation should measure whether Jev reduces token usage, unnecessary file reads, repeated reasoning, and latency while preserving end-to-end task quality.
