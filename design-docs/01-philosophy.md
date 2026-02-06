---
marp: true
theme: ember
paginate: true
---

<script type="module">
import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
mermaid.initialize({ startOnLoad: true, theme: 'dark', flowchart: { htmlLabels: true, nodeSpacing: 30, rankSpacing: 30 } });
</script>

# TemPurview

### CLI-first SRE tooling for Temporal workflows at scale

Built for operators and AI agents who think in pipelines,
not dashboards.

---

## The Problem

Temporal Cloud's Web UI is built for **exploration**.

Production operators need **answers**.

```
"How many workflows failed in the last 2 hours?"
"Which activity types are retrying the most?"
"Is there a retry storm happening right now?"
```

These questions should be one command away.

---

## The Deeper Problem

- Workflows can **succeed while silently swallowing issues**
  - Developers implement inconsistent error handling across activities and workflows
  - A retry that eventually succeeds doesn't show up as a failure — but it's still a problem

- Horizontal scaling of workers creates a **logging explosion**
  - 10 workers retrying the same activity type produces 10x the log volume with no additional signal
  - More infrastructure, more noise, less clarity

- The Web UI shows you individual workflows
  - It doesn't tell you **what's going wrong across your fleet**

TemPurview's `insight scan` semantics exist to cut through the noise.

---

<!-- _class: compact -->

## Existing Alternatives: The Dashboard Path

**[Temporal + Prometheus + Grafana](https://docs.temporal.io/cloud/metrics/prometheus-grafana)**

- Temporal Cloud exposes metrics via a Prometheus-compatible endpoint
- SDK metrics require instrumenting each worker with a Prometheus registry
- Grafana dashboards visualize state transitions, latency percentiles, task queue depth

**What it gives you**: infrastructure health, aggregate throughput, latency P99s

**What it doesn't**:
- No visibility into **why** workflows are failing — only **that** they are
- No correlation between retries, activity errors, and workflow outcomes
- No semantic analysis — "ProcessPayment retried 13 times" is invisible in a time-series graph
- Requires **separate infrastructure** (Prometheus, Grafana, cert management, dashboard authoring)

Dashboards answer "is the system healthy?"
They don't answer "what is going wrong and where?"

---

## Existing Alternatives: The Web UI

**[Temporal Web UI](https://docs.temporal.io/web-ui)**

- Shows workflow executions within the retention period
- Per-workflow detail: status, history, pending activities
- Search Attributes enable custom filtering
- Limited to 20 saved views

**What it gives you**: deep drill-down into individual workflows

**What it doesn't**:
- **No fleet-level analysis** — you see one workflow at a time
- No aggregation: "which activity type fails the most?" requires manual inspection
- No automation: can't pipe results into scripts or alerting
- Browser-based — doesn't compose with Unix tooling or AI agents

The Web UI is built for **exploration**.
Not for **operational answers at scale**.

---

## Existing Alternatives: The Observability Platform Path

**[Pydantic Logfire](https://pydantic.dev/logfire)** — AI observability built on OpenTelemetry

- Unified traces, metrics, logs across LLMs, agents, and full app stack
- SQL query interface for flexible data exploration
- Live span viewing, cost tracking, token analytics
- Native SDKs for Python, TypeScript, Rust
- OpenTelemetry-based — any OTel-instrumented framework works automatically

**What it gives you**: deep visibility into **application-level** behavior —
LLM calls, agent reasoning chains, API latency, database queries

**What it doesn't**:
- **Temporal-unaware** — sees spans and traces, not workflow semantics
- Can't answer "which activity type retries the most across my fleet?"
- No concept of workflow status, retry storms, or failure rate patterns
- Requires instrumentation in your application code
- **SaaS product** — data leaves your infrastructure ($49-$249/mo)

Logfire sees the **trees** (individual traces).
TemPurview sees the **forest** (fleet-level patterns via Temporal's own APIs).

---

## Existing Alternatives: The TUI Path

**[Tempo](https://github.com/galaxy-io/tempo)** (Go, tview + jig)

- Polished TUI with 26 built-in themes, connection profiles, hot-swap
- Event history in 3 modes: list, tree, and **Gantt-style timeline**
- Workflow relationship graphs, batch cancel/terminate
- Uses the official Temporal Go SDK directly
- Built by the Galaxy team; uses Temporal team's `jig` UI framework

**What it gives you**: the best interactive Temporal TUI available

**What it doesn't**:
- **TUI-only** — no CLI subcommands, no JSON output, no pipe composability
- No `insight scan` — no fleet-level analysis or finding algorithms
- No automation path — can't feed results into scripts or AI agents
- No web UI mode

Tempo is excellent for **hands-on-keyboard exploration**.
TemPurview is built for **operators and agents who think in pipelines**.

---

<!-- _class: compact -->

## Where TemPurview Fits

| Approach | Strength | Gap |
|----------|----------|-----|
| Grafana + Prometheus | Infrastructure metrics | No semantic analysis of failures |
| Temporal Web UI | Individual workflow drill-down | No fleet-level patterns |
| Tempo (TUI) | Rich interactive exploration | No CLI, no automation, no insights |
| Pydantic Logfire | Application-level traces + LLM observability | Temporal-unaware, SaaS dependency |
| General observability | Logs + traces + metrics | Temporal-unaware, no workflow semantics |
| **TemPurview** | **Fleet-level analysis + CLI automation** | **Complements all of the above** |

TemPurview doesn't replace your dashboard or your TUI.
It answers the questions they can't.

---

## Design Philosophy

**CLI-first, TUI-enhanced.**

Every capability is a structured subcommand.
The TUI is one execution mode, not the only one.

This is the same philosophy behind:
- **kubectl** -- every cluster operation is a subcommand
- **gh** -- GitHub's CLI that made the web UI optional
- **temporal** -- Temporal's own CLI (which we complement, not replace)

---

## Why the TUI Matters

CLI-first doesn't mean CLI-only.

The TUI is a **critical interface for an invested operator** —
someone who lives in the system and needs to move fast.

- **Speed**: keyboard-driven navigation across views in seconds, not clicks
- **Pattern recognition**: scanning a workflow list builds intuition — you start to *see* failure patterns before you can articulate them
- **Broad strokes first**: status dashboard + workflow list → quick mental model of fleet health
- **Then delegate**: spot an issue in the TUI, hand it off —
  run `tpv insight scan` in a script, or point an AI agent
  at a specific workflow ID for deep analysis

The TUI is where human reasoning meets machine execution.
You develop the hypothesis. The CLI and agents do the legwork.

---

## Principle: Composability Over Features

***A tool that works with Unix pipes is more powerful
than a tool with every feature built in.***

```bash
# Count failed workflows by type
tpv workflow list --status failed --output json \
  | jq -r '.[] | .workflow_type' \
  | sort | uniq -c | sort -rn

# Alert on stuck workflows
tpv insight scan --since 1h --output json \
  | jq '.findings[] | select(.severity == "critical")'
```

The JSON output format is not an afterthought.
**It is the primary interface for automation.**

---

## Principle: Progressive Disclosure

**Bare command = TUI** for backward compatibility
and interactive exploration.

```
tpv                     # Launch TUI
tpv --mock              # Launch TUI with mock data
```

**Subcommands = CLI** for scripting, CI/CD, and
quick answers.

```
tpv workflow list --status running
tpv insight scan --since 2h
tpv config show
```

Same tool. Two interaction paradigms.
Zero configuration to switch between them.

---

## Progressive Disclosure in `--help`

The CLI's help hierarchy is **self-documenting**:

```
tpv --help                      # top-level: all subcommands
tpv workflow --help             # resource: all actions
tpv workflow list --help        # action: all flags + env vars
```

Each level reveals only what's relevant at that depth.
As nesting grows, the discoverability scales with it.

This is intentional for **AI agent ergonomics**. An LLM
can navigate the CLI by reading `--help` output at each level —
no need for an overwhelming
[Agent Skill](https://agentskills.io) definition
to steer the agent through every possible flag and subcommand.

The CLI **is** the skill. `--help` **is** the documentation.

---

## Principle: Machines Read JSON, Humans Read Tables

Output format auto-detects based on context:

| Context | Format |
|---------|--------|
| TTY (interactive terminal) | Table |
| Piped to another command | JSON |
| Explicit `--output json` | JSON |
| Explicit `--output table` | Table |

Inspired by **gh** (GitHub CLI), which pioneered
this pattern for developer tools.

---

## Principle: No Surprises

Global flags work everywhere:

```bash
tpv --mock workflow list
tpv workflow list --mock
tpv workflow --mock list
```

All equivalent. Clap's `global = true` makes this
ergonomic without argument-order anxiety.

Environment variables are respected:

```bash
export TEMPORAL_ADDRESS=localhost:7233
export TEMPORAL_NAMESPACE=default
tpv workflow count  # Just works
```

---

## What We Don't Do

- **No interactive prompts** in CLI mode.
  Every parameter is a flag or argument.

- **No configuration wizard**.
  Environment variables and `~/.config/tempurview/config.toml`.

- **No daemon mode**.
  One invocation, one result, one exit code.

- **No plugin system**.
  Composability comes from Unix pipes, not abstractions.

> "Perfection is achieved not when there is nothing more
> to add, but when there is nothing left to take away."
> -- Antoine de Saint-Exupery

---

## The Operator's Workflow

<div class="mermaid">
graph LR
    A["Quick question?"] --> B["tpv workflow count"]
    B --> C{"Need context?"}
    C -->|Yes| D["tpv workflow list<br>--status failed"]
    D --> E{"Need depth?"}
    E -->|Yes| F["tpv insight scan<br>--since 2h"]
    F --> G{"Need exploration?"}
    G -->|Yes| H["tpv<br>(launches TUI)"]
    C -->|No| I["Done"]
    E -->|No| I
    G -->|No| I
    style A fill:#295264,stroke:#5097b7,color:#f6e1ce
    style B fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style C fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style D fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style E fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style F fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style G fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style H fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style I fill:#1d3a47,stroke:#5097b7,color:#f6e1ce
</div>

Every level of detail is one command deeper. Never more.

---

## LLM Agent's Workflow

<div class="mermaid">
graph LR
    A["Cron trigger"] --> B["List failed<br>workflows"]
    B --> C["Insight scan"]
    C --> D{"Findings?"}
    D -->|None| E["Healthy"]
    D -->|Yes| F["Triage"]
    F --> G["Fetch detail<br>+ activities<br>+ events"]
    G --> H["LLM correlates<br>root cause"]
    H --> I["Alert / Report"]
    style A fill:#295264,stroke:#5097b7,color:#f6e1ce
    style B fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style C fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style D fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style E fill:#1d3a47,stroke:#5097b7,color:#f6e1ce
    style F fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style G fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style H fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style I fill:#1d3a47,stroke:#ff6d63,color:#f6e1ce
</div>

The CLI's JSON output is the **agent's API**.
No SDK. No client library. Just `stdout`.
