# jev-mcp

A lightweight MCP server for using Jev as a fast decision layer inside Codex workflows.

Codex is good at understanding requirements, exploring repositories, editing code, running commands, and solving problems that require multi-step reasoning.

Jev is good at something different: making small, structured decisions quickly.

This project connects the two.

Instead of asking Codex to spend a full reasoning step on every small decision, Codex can delegate narrow classification, scoring, routing, and relevance checks to Jev through MCP.

## Build and connect

Requires Rust 1.88 or newer and a TypeSafe API key for evaluations.

```sh
cargo build --release --locked
```

The executable is `target/release/jev-mcp`. It speaks newline-delimited MCP JSON-RPC on stdin/stdout using the official Rust MCP SDK. Your MCP client launches it; running it in a terminal waits for protocol messages.

For Codex, add this to `~/.codex/config.toml`, replacing the executable path:

```toml
[mcp_servers.jev]
command = "/absolute/path/to/jev-mcp/target/release/jev-mcp"
env_vars = ["TYPESAFE_API_KEY", "TYPESAFE_MODEL"]
tool_timeout_sec = 70
```

Set `TYPESAFE_API_KEY` in the environment of the process launching Codex, then restart the client. GUI applications may not inherit your terminal environment. The server reads configuration at startup; it does not load `.env` files. See the [official Codex MCP configuration documentation](https://developers.openai.com/codex/mcp/).

| Environment variable | Behavior |
| --- | --- |
| `TYPESAFE_API_KEY` | Bearer credential. Missing or blank keys allow tool discovery, but evaluation returns a tool error. |
| `TYPESAFE_MODEL` | Default model, `jev-latest` when omitted. A tool's optional `model` overrides it. |

All four tools send the supplied state and questions to `https://api.typesafe.ai/v1/systemone`. They do not read repository files or collect agent traces themselves. The caller supplies the evidence to evaluate. API use may incur TypeSafe charges.

The client has a 10-second connection timeout, a 30-second per-attempt timeout, and a 60-second total deadline. HTTP 429 and 529 retry at most twice with 0.5/1-second backoff, or the server's `Retry-After` delay in seconds. Other HTTP errors and transport errors are returned without retries. Redirects are disabled and responses are capped at 8 MiB.

Input validation, missing credentials, API errors, and malformed answers return MCP tool results with `isError: true`. Unknown tool names return a JSON-RPC error. Remote error bodies and credentials are not included in errors. Stdout is reserved for MCP messages.

## Tool arguments and results

| Tool | Required arguments | Optional arguments | Answer |
| --- | --- | --- | --- |
| `jev.noul` | `state`, `instructions` | `criteria` with `true`/`false` descriptions, `model` | Probability of yes in `noul` (0–1). |
| `jev.choice` | `state`, `instructions`, `criteria` map with 1–255 named options | `model` | `choice`, `probabilities`, `confidence`. |
| `jev.score` | `state`, `instructions`, `criteria` array of 2–10 ordered descriptions | `model` | `score`, `legend`, `probabilities`, `confidence`. |
| `jev.batch` | `state`, nonempty `questions` map | `model` | One answer per question ID. |

`state` and `instructions` accept strings, objects, or arrays. Choice descriptions accept those forms or `null`; Score levels accept strings, objects, or arrays. Unknown argument fields are rejected. Input types follow the [TypeSafe API reference](https://docs.typesafe.ai/api).

Every successful tool returns the API envelope (`model`, `answers`, `usage`) in MCP `structuredContent` and as JSON text in `content`. Single tools place their answer at `answers.result`; batch preserves your question IDs. A Score uses zero-based level positions, so three levels produce a value from 0 to 2, including fractions. A Noul near 0 is a strong no, near 1 a strong yes, and near 0.5 uncertain; it has no separate `confidence`.

Prefer `jev.batch` for independent questions sharing the same state. These are tool arguments (the MCP client supplies the `tools/call` envelope):

```json
{
  "state": {
    "diff": "A worker retry limit changed from 3 to 0.",
    "test_failure": "worker_retries_transient_failure: expected 3 attempts, got 1"
  },
  "questions": {
    "related": {
      "type": "noul",
      "instructions": "Is test_failure related to diff?"
    },
    "subsystem": {
      "type": "choice",
      "instructions": "Which subsystem should be investigated first?",
      "criteria": {
        "worker": "Background execution and retries",
        "api": "HTTP request handling",
        "unknown": "Insufficient evidence"
      }
    },
    "impact": {
      "type": "score",
      "instructions": "How severely does diff affect retry behavior?",
      "criteria": [
        "Retry behavior is unchanged",
        "Some retry scenarios fail but retries still occur",
        "Retries are disabled entirely"
      ]
    }
  }
}
```

Questions in a batch cannot consume each other's answers. If a question needs an earlier result, make a later call with that result included in its state.

## Development

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Tests use a local mock HTTP server and an actual stdio subprocess; no TypeSafe key or paid API calls are needed. Live model quality and latency require separate evaluation with real credentials and representative data.

```text
                ┌──────────────┐
                │    Codex     │
                │ planner/actor│
                └──────┬───────┘
                       │
                  small decision
                       │
                ┌──────▼───────┐
                │     Jev      │
                │ decision layer│
                └──────┬───────┘
                       │
             typed / scored result
                       │
                ┌──────▼───────┐
                │    Codex     │
                │ edit/test/run│
                └──────────────┘
```

## Why?

Coding agents make a surprisingly large number of small decisions during a task.

For example:

* Is this file relevant to the current bug?
* Is this test failure related to the latest change?
* Which subsystem is the most likely source of the problem?
* Is this warning worth investigating now?
* Which hypothesis should be tested next?
* Does this diff look risky?
* Is the agent still making progress?

These decisions often do not require another large generative reasoning step.

They are closer to:

```text
yes / no
```

```text
A / B / C / D
```

or:

```text
risk = 0.82
```

`jev-mcp` exposes these kinds of decisions as MCP tools that Codex can call during a task.

## Concept

The goal is not to replace Codex with Jev.

The goal is to let each system do the kind of work it is best suited for.

### Codex

Use Codex for:

* repository exploration
* architecture and design
* code generation
* refactoring
* debugging
* shell operations
* test execution
* multi-step reasoning
* explaining results

### Jev

Use Jev for:

* binary decisions
* classification
* candidate selection
* relevance filtering
* prioritization
* risk scoring
* confidence-based routing
* large batches of small decisions

In other words:

> Codex handles the reasoning loop.
> Jev handles the decision nodes inside that loop.

## Example

A normal debugging workflow might look like this:

```text
User
  │
  ▼
Codex
  │
  ├─ inspect error
  ├─ inspect repository
  │
  ▼
Jev
  │
  ├─ worker       0.71
  ├─ scheduler    0.19
  ├─ database     0.06
  ├─ api          0.03
  └─ frontend     0.01
  │
  ▼
Codex
  │
  ├─ investigate worker
  ├─ modify code
  └─ run tests
       │
       ▼
      Jev
       │
       └─ regression related? → 0.93
       │
       ▼
     Codex
       │
       └─ continue / finish
```

## MCP interface

The initial API is intentionally small.

```text
jev.noul
jev.choice
jev.score
jev.batch
```

For example, arguments to `jev.choice`:

```json
{
  "state": "...repository state...",
  "instructions": "Which subsystem most likely contains the bug?",
  "criteria": {
    "api": null,
    "scheduler": null,
    "worker": null,
    "database": null,
    "frontend": null
  }
}
```

Illustrative `structuredContent` result (values are examples):

```json
{
  "model": "jev-example",
  "answers": {
    "result": {
      "type": "choice",
      "choice": "worker",
      "probabilities": {
        "worker": 0.71,
        "scheduler": 0.19,
        "database": 0.06,
        "api": 0.03,
        "frontend": 0.01
      },
      "confidence": 0.5
    }
  },
  "usage": { "input_tokens": 200, "output_tokens": 30 }
}
```

The important part is that Jev is not being used to generate prose.

It is being used as a typed decision primitive.

## Confidence-based escalation

Jev does not need to make every decision autonomously.

A workflow can escalate uncertain decisions back to Codex.

For example:

```text
confidence >= 0.90
    → accept Jev decision

0.60 <= confidence < 0.90
    → let Codex inspect the result

confidence < 0.60
    → gather more evidence and retry
```

This keeps cheap decisions cheap while preserving deeper reasoning when it is actually needed.

These thresholds illustrate a caller policy; the server does not enforce them. They apply to Choice/Score confidence, which describes the answer distribution rather than guaranteeing correctness. Calibrate thresholds on your own data. Noul requires a separate policy based on its yes probability. See [TypeSafe confidence](https://docs.typesafe.ai/confidence).

## Possible use cases

### File relevance

Before reading dozens of candidate files:

```text
Is this file relevant to the current task?
```

### Test failure triage

Classify failures as:

```text
regression
unrelated
flaky
unknown
```

### Hypothesis routing

Given several debugging hypotheses, select which one should be investigated next.

### Diff risk scoring

Estimate whether a change deserves additional review or testing.

### Agent trace monitoring

Jev can also observe the Codex execution trace itself.

```text
progressing?
stuck?
repeating?
risky?
```

This makes it possible to use Jev not only inside the coding workflow, but also as a lightweight observer of the coding agent.

## Design philosophy

Large language models are extremely capable, but not every decision needs full generative reasoning.

A coding agent may perform hundreds of tiny judgments while completing a single task.

This project explores a simple idea:

> Use a powerful reasoning model for difficult decisions, and a fast typed decision model for everything else.

Instead of treating Jev as a smaller replacement for an LLM, treat it as a primitive that can appear many times inside an agent's reasoning graph.

```text
Codex → Jev → Codex → Jev → Codex
```

The interesting question is not whether Jev can replace Codex.

The interesting question is:

> How much reasoning can Codex avoid doing without reducing the quality of the final result?

## Status

Experimental. The four stdio MCP tools, TypeSafe HTTP integration, validation, bounded retries, and mock/protocol tests are implemented. The workflows below are intended applications; automated repository inspection, trace collection, and measured cost/quality improvements are not implemented.

The first targets are:

* test failure triage
* file relevance filtering
* next-hypothesis selection
* diff risk scoring
* Codex trace monitoring

The project will measure whether Jev-assisted workflows can reduce:

* token usage
* unnecessary repository reads
* redundant reasoning
* agent latency

while preserving or improving task completion quality.
