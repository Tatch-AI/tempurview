# Workflows

## List workflows

```bash
tpv workflow list [--status <STATUS>] [--workflow-type <TYPE>] [--since <TIME>] [--before <TIME>]
```

### Filters

| Flag | Description | Example |
|------|-------------|---------|
| `--status` | Filter by execution status | `Running`, `Completed`, `Failed` |
| `--workflow-type` | Filter by workflow type name | `OrderWorkflow` |
| `--since` | Workflows started after this time | `2h`, `3d`, `2024-01-15` |
| `--before` | Workflows started before this time | `1h`, `2024-12-01` |

```bash
# Running workflows from the last 24 hours
tpv workflow list --status Running --since 24h

# All failed workflows of a specific type
tpv workflow list --status Failed --workflow-type PaymentWorkflow
```

## Get workflow details

```bash
tpv workflow get <workflow-id> [--run-id <RUN_ID>]
```

If `--run-id` is omitted, the latest run is returned.

## Count workflows

```bash
tpv workflow count [--status <STATUS>] [--query <QUERY>]
```

The `--query` flag accepts raw Temporal visibility queries.

## Cancel a workflow

```bash
tpv workflow cancel <workflow-id> [--run-id <RUN_ID>]
```

## Terminate a workflow

```bash
tpv workflow terminate <workflow-id> [--run-id <RUN_ID>] [--reason <REASON>]
```

The default reason is `"Terminated via CLI"`.
