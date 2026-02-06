---
marp: true
theme: default
paginate: true
backgroundColor: #1a1a2e
color: #e0e0e0
style: |
  section {
    font-family: 'SF Mono', 'Fira Code', monospace;
  }
  h1 {
    color: #00d4ff;
  }
  h2 {
    color: #7b68ee;
  }
  code {
    background: #2a2a4a;
    color: #00ff88;
  }
  strong {
    color: #ff6b6b;
  }
  a {
    color: #00d4ff;
  }
  blockquote {
    border-left: 4px solid #7b68ee;
    color: #b0b0d0;
  }
---

# Tempurview

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

A tool that works with Unix pipes is more powerful
than a tool with every feature built in.

```bash
# Count failed workflows by type
tempurview workflow list --status failed --output json \
  | jq -r '.[] | .workflow_type' \
  | sort | uniq -c | sort -rn

# Alert on stuck workflows
tempurview insight scan --since 1h --output json \
  | jq '.findings[] | select(.severity == "critical")'
```

The JSON output format is not an afterthought.
**It is the primary interface for automation.**

---

## Principle: Progressive Disclosure

**Bare command = TUI** for backward compatibility
and interactive exploration.

```
tempurview              # Launch TUI
tempurview --mock       # Launch TUI with mock data
```

**Subcommands = CLI** for scripting, CI/CD, and
quick answers.

```
tempurview workflow list --status running
tempurview insight scan --since 2h
tempurview config show
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
tempurview --mock workflow list
tempurview workflow list --mock
tempurview workflow --mock list
```

All equivalent. Clap's `global = true` makes this
ergonomic without argument-order anxiety.

Environment variables are respected:

```bash
export TEMPORAL_ADDRESS=localhost:7233
export TEMPORAL_NAMESPACE=default
tempurview workflow count  # Just works
```

---

## What We Don't Do

- **No interactive prompts** in CLI mode.
  Every parameter is a flag or argument.

- **No configuration wizard**.
  Environment variables and `~/.tempurview/config.toml`.

- **No daemon mode**.
  One invocation, one result, one exit code.

- **No plugin system**.
  Composability comes from Unix pipes, not abstractions.

> "Perfection is achieved not when there is nothing more
> to add, but when there is nothing left to take away."
> -- Antoine de Saint-Exupery

---

## The Operator's Workflow

```
                 Quick question?
                      |
              tempurview workflow count
                      |
                 Need context?
                      |
              tempurview workflow list --status failed
                      |
                 Need depth?
                      |
              tempurview insight scan --since 2h
                      |
                 Need exploration?
                      |
                  tempurview
                  (launches TUI)
```

Every level of detail is one command deeper.
Never more.
