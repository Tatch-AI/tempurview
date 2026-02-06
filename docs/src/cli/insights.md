# Insights

The `insight scan` command analyzes workflows for operational patterns and potential issues.

```bash
tpv insight scan [--since <TIME>] [--before <TIME>]
```

## What it scans for

Tempurview runs 7 finding algorithms across your workflows:

### List-level findings
- **High failure rate** — workflow types with an unusually high proportion of failures
- **Repeated failures** — the same workflow type failing repeatedly in a short window
- **Long-running workflows** — executions that have been running significantly longer than peers

### Activity-level findings
- **Retry storms** — activities with a high number of retry attempts
- **Slow activities** — activities taking significantly longer than expected
- **Repeated activity failures** — the same activity failing across multiple workflows
- **Timeout patterns** — activities consistently hitting timeout limits

## Examples

```bash
# Scan the last 24 hours
tpv insight scan --since 24h

# Scan a specific date range
tpv insight scan --since 2024-01-15 --before 2024-01-16
```
