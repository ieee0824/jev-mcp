# jev-codex-mcp

A lightweight MCP server for using Jev as a fast decision layer inside Codex workflows.

Codex is good at understanding requirements, exploring repositories, editing code, running commands, and solving problems that require multi-step reasoning.

Jev is good at something different: making small, structured decisions quickly.

This project connects the two.

Instead of asking Codex to spend a full reasoning step on every small decision, Codex can delegate narrow classification, scoring, routing, and relevance checks to Jev through MCP.

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

`jev-codex-mcp` exposes these kinds of decisions as MCP tools that Codex can call during a task.

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

For example:

```json
{
  "state": "...repository state...",
  "question": "Which subsystem most likely contains the bug?",
  "choices": [
    "api",
    "scheduler",
    "worker",
    "database",
    "frontend"
  ]
}
```

Possible result:

```json
{
  "worker": 0.71,
  "scheduler": 0.19,
  "database": 0.06,
  "api": 0.03,
  "frontend": 0.01
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

Experimental.

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
