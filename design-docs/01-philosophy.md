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

### A CLI-first terminal tool for Temporal workflows

Built for operators who think in pipelines,
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

## Design Philosophy

**CLI-first, TUI-enhanced.**

Every capability is a structured subcommand.
The TUI is one execution mode, not the only one.

This is the same philosophy behind:
- **kubectl** -- every cluster operation is a subcommand
- **gh** -- GitHub's CLI that made the web UI optional
- **temporal** -- Temporal's own CLI (which we complement, not replace)

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
