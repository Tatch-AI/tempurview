# Configuration

Tempurview reads configuration from environment variables, CLI flags, and an optional config file. CLI flags take precedence over environment variables.

## Environment variables

| Variable | Description | Default |
|----------|-------------|---------|
| `TEMPORAL_ADDRESS` | Temporal server gRPC address | `localhost:7233` |
| `TEMPORAL_NAMESPACE` | Temporal namespace | `default` |
| `TEMPORAL_API_KEY` | API key (Temporal Cloud) | — |
| `TEMPORAL_TUI_REFRESH_INTERVAL` | Auto-refresh interval in seconds | `30` |
| `TEMPORAL_TUI_DEFAULT_LIMIT` | Max workflows to fetch | `50` |
| `TEMPORAL_TUI_TICK_RATE` | TUI tick rate in milliseconds | `250` |

You can also place these in a `.env` file in your working directory.

## CLI flags

These override environment variables when provided:

```bash
tpv --address my-cluster:7233 --namespace production --limit 100
```

| Flag | Description |
|------|-------------|
| `--address <ADDR>` | Temporal server address |
| `--namespace <NS>` | Temporal namespace |
| `--mock` | Use mock data (no connection needed) |
| `--mock-count <N>` | Number of mock workflows (default: 100) |
| `--limit <N>` | Max workflows to fetch (default: 50) |
| `--output <FORMAT>` | Output format: `json` or `table` (auto-detected) |
| `--logs` | Show log file location and recent errors |

## Config file

Tempurview reads `~/.tempurview/config.toml` for additional settings.

```toml
[insights]
# Allowlist of workflow types to include in insight scans.
# If empty, all workflow types are scanned.
allowlist = ["OrderWorkflow", "PaymentWorkflow"]
```
