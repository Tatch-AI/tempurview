---
marp: true
theme: ember
paginate: true
---

# CLI Design

### Subcommand structure, conventions, and ergonomics

---

## Command Taxonomy

```
tpv                                 # TUI (default)
tpv workflow list                   # List workflows
tpv workflow get <ID>               # Describe a workflow
tpv workflow count                  # Count workflows
tpv workflow cancel <ID>            # Cancel a workflow
tpv workflow terminate <ID>         # Terminate a workflow
tpv activity list <ID>             # List activities
tpv event list <ID>                # List history events
tpv insight scan                    # Run insights analysis
tpv config show                     # Show configuration
tpv test-connection                 # Verify connectivity
```

**Noun-verb pattern**: `<resource> <action>`

Same pattern as **kubectl**, **gh**, **docker**.

---

## Naming Conventions

**Singular nouns**, not plural:

```
tpv workflow list       # not "workflows list"
tpv activity list       # not "activities list"
tpv insight scan        # not "insights scan"
```

Follows the precedent of:
- `kubectl pod list` (not `pods`)
- `gh issue list` (not `issues`)
- `docker container ls` (not `containers`)

The noun names the resource type.
The verb names the action.
There is no ambiguity.

---

## Global Flags

Available on every command, at any position:

```
--address <ADDR>        Temporal server address
--namespace <NS>        Temporal namespace
--mock                  Use mock data
--mock-count <N>        Mock workflow count
--limit <N>             Max workflows to fetch
--output json|table     Output format
--logs                  Show log file location
```

Powered by clap's `#[arg(global = true)]`.

```bash
# All equivalent:
tpv --mock workflow list
tpv workflow --mock list
tpv workflow list --mock
```

---

## Environment Variable Integration

Every connection flag has an env var equivalent:

| Flag | Environment Variable |
|------|---------------------|
| `--address` | `TEMPORAL_ADDRESS` |
| `--namespace` | `TEMPORAL_NAMESPACE` |
| (none) | `TEMPORAL_API_KEY` |

Clap's `#[arg(env = "...")]` makes this declarative:

```rust
#[arg(long, global = true, env = "TEMPORAL_ADDRESS")]
pub address: Option<String>,
```

The help text automatically shows the current env value:

```
--address <ADDRESS>  Temporal server address
                     [env: TEMPORAL_ADDRESS=localhost:7233]
```

---

## Output Format Auto-Detection

```rust
pub fn resolve(explicit: Option<OutputFormatArg>) -> Self {
    match explicit {
        Some(OutputFormatArg::Json) => OutputFormat::Json,
        Some(OutputFormatArg::Table) => OutputFormat::Table,
        None => {
            if std::io::stdout().is_terminal() {
                OutputFormat::Table
            } else {
                OutputFormat::Json
            }
        }
    }
}
```

Interactive:
```bash
$ tpv workflow list --mock
┌────────┬──────────────────────┬──────────────────┐
│ STATUS ┆ TYPE                 ┆ WORKFLOW ID       │
...
```

Piped:
```bash
$ tpv workflow list --mock | jq '.[0]'
{
  "workflow_id": "order-123",
  "status": "Running",
  ...
}
```

---

## Subcommand: workflow list

```
tpv workflow list [OPTIONS]

Options:
  --status <STATUS>               Filter by execution status
  --workflow-type <WORKFLOW_TYPE>  Filter by workflow type
  --since <SINCE>                 Start time filter (e.g., 2h, 3d)
  --before <BEFORE>               End time filter
```

Accepts human-friendly time formats:
- Relative: `2h`, `3d`, `1w`, `30m`
- ISO 8601: `2024-01-15T10:00:00Z`
- Date only: `2024-01-15`

```bash
# Failed workflows in the last 2 hours
tpv workflow list --status failed --since 2h

# All payment workflows since Monday
tpv workflow list --workflow-type PaymentWorkflow --since 2024-01-15
```

---

## Subcommand: workflow get

```
tpv workflow get <WORKFLOW_ID> [--run-id <RUN_ID>]
```

Key-value table for TTY:

```
┌────────────────┬──────────────────────────────────────┐
│ FIELD          ┆ VALUE                                 │
╞════════════════╪══════════════════════════════════════╡
│ Workflow ID    ┆ order-processing-456                  │
│ Run ID         ┆ run-abc-123                           │
│ Type           ┆ PaymentProcessingWorkflow             │
│ Status         ┆ Failed                                │
│ Started        ┆ 2024-01-15 10:00:00 UTC               │
│ History Length ┆ 42                                    │
│ Failure        ┆ Payment declined by processor         │
└────────────────┴──────────────────────────────────────┘
```

Full JSON for automation:
```bash
tpv workflow get order-456 --output json | jq '.failure'
```

---

## Subcommand: insight scan

```
tpv insight scan [--since <SINCE>] [--before <BEFORE>]
```

Multi-step pipeline:
1. Fetch workflow list (fleet overview)
2. Compute list-level findings (failure rates, stuck workflows)
3. Sample workflow histories (up to 30)
4. Correlate activities and child workflows
5. Compute activity-level findings (retries, latency, failures)
6. Rank all findings by severity

```
Scanned 50 workflows, fetched 30 histories in 1.6s. 27 findings.
┌──────────┬──────────────────┬─────────────────────────────────┬──────────┐
│ SEVERITY ┆ CATEGORY         ┆ TITLE                            ┆ AFFECTED │
╞══════════╪══════════════════╪═════════════════════════════════╪══════════╡
│ CRIT     ┆ Error in I/O     ┆ ProcessPayment: "error" in 18   ┆ 18       │
│ CRIT     ┆ Activity Retry   ┆ ProcessPayment: 13 retried       ┆ 13       │
│ WARN     ┆ Retry Storm      ┆ ProcessPayment: avg 2.1 attempts ┆ 13       │
│ INFO     ┆ Failure Rate     ┆ DataProcessing: 3/13 failed (23%)┆ 3        │
└──────────┴──────────────────┴─────────────────────────────────┴──────────┘
```

---

## Subcommand: test-connection

```
tpv test-connection [--mock]
```

Validates:
1. Environment variables are set
2. gRPC channel connects
3. `CountWorkflowExecutions` succeeds
4. Per-status counts work

```
Testing Temporal connection...

Environment variables:
  TEMPORAL_ADDRESS:   us-west1.gcp.api.temporal.io:7233
  TEMPORAL_NAMESPACE: production.abc123
  TEMPORAL_API_KEY:   (set, hidden)

Connection successful!
  Total workflows: 12,456

Workflow counts by status:
  Running:        2,341
  Completed:      9,876
  Failed:           239

All tests passed! Your gRPC connection is working.
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Runtime error (connection failed, workflow not found) |
| 2 | Usage error (invalid arguments, handled by clap) |

Follows POSIX conventions.
Machine-parseable with `$?`.

```bash
if tpv workflow count --status failed --output json \
    | jq -e '. > 100' > /dev/null; then
  echo "ALERT: More than 100 failed workflows"
fi
```

---

## Design Anchors

TemPurview's CLI draws from proven patterns:

| Pattern | Source |
|---------|--------|
| Noun-verb subcommands | kubectl, gh, docker |
| Auto-detect output format | gh (GitHub CLI) |
| Global flags at any position | clap (standard in Rust CLIs) |
| Env var fallback for flags | kubectl, terraform |
| `--output json\|table` | kubectl, az (Azure CLI) |
| Connection test command | `pg_isready`, `redis-cli ping` |
| `config show` | terraform, kubectl |

Every decision has precedent.
Nothing is invented.
