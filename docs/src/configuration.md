# Configuration

Tempurview reads configuration from CLI flags, connection profiles, environment variables, and an optional config file.

## Resolution priority

Connection parameters are resolved in this order (first wins):

1. **CLI flags** `--address` / `--namespace` — always win
2. **Profile** via `--profile <name>` or `TEMPURVIEW_PROFILE` env var
3. **Default profile** from `default_profile` in config.toml
4. **Environment variables** `TEMPORAL_ADDRESS` / `TEMPORAL_NAMESPACE` / `TEMPORAL_API_KEY`
5. **Hard-coded defaults** (`localhost:7233`, `default`, no API key)

When a profile is active, bare env vars (`TEMPORAL_ADDRESS`, etc.) do **not** override it. Only explicit `--address`/`--namespace` CLI flags win over a profile.

## Connection profiles

Profiles let you save named connection targets in `~/.tempurview/config.toml` and switch between them:

```bash
# Add profiles
tpv config profile-add local --address localhost:7233 --namespace default
tpv config profile-add cloud --address us-west1.gcp.api.temporal.io:7233 --namespace my-ns --api-key tctl_xxx

# List profiles
tpv config profile-list

# Set a default
tpv config set-default cloud

# Use a specific profile
tpv -p local workflow list        # CLI with local profile
tpv -p cloud                      # TUI with cloud profile
TEMPURVIEW_PROFILE=local tpv      # env var selection

# Override a profile's address
tpv -p local --address custom:9999 config show
```

The first profile added is automatically set as the default.

### Config file format

```toml
default_profile = "cloud"

[profiles.cloud]
address = "us-west1.gcp.api.temporal.io:7233"
namespace = "my-namespace.abc123"
api_key = "tctl_..."

[profiles.local]
address = "localhost:7233"
namespace = "default"
# no api_key needed for local

[insights]
allowlist = ["expected error"]
concurrency = 50
```

## Environment variables

| Variable | Description | Default |
|----------|-------------|---------|
| `TEMPORAL_ADDRESS` | Temporal server gRPC address | `localhost:7233` |
| `TEMPORAL_NAMESPACE` | Temporal namespace | `default` |
| `TEMPORAL_API_KEY` | API key (Temporal Cloud) | — |
| `TEMPURVIEW_PROFILE` | Connection profile to use | — |
| `TEMPORAL_TUI_REFRESH_INTERVAL` | Auto-refresh interval in seconds | `30` |
| `TEMPORAL_TUI_DEFAULT_LIMIT` | Max workflows to fetch | `50` |
| `TEMPORAL_TUI_TICK_RATE` | TUI tick rate in milliseconds | `250` |

You can also place these in a `.env` file in your working directory.

## CLI flags

These override environment variables and profiles when provided:

```bash
tpv --address my-cluster:7233 --namespace production --limit 100
```

| Flag | Description |
|------|-------------|
| `--profile <NAME>` / `-p <NAME>` | Connection profile to use |
| `--address <ADDR>` | Temporal server address (overrides profile) |
| `--namespace <NS>` | Temporal namespace (overrides profile) |
| `--mock` | Use mock data (no connection needed) |
| `--mock-count <N>` | Number of mock workflows (default: 100) |
| `--limit <N>` | Max workflows to fetch (default: 50) |
| `--output <FORMAT>` | Output format: `json` or `table` (auto-detected) |
| `--logs` | Show log file location and recent errors |

## Config file

Tempurview reads `~/.tempurview/config.toml` for profiles and additional settings.

```toml
[insights]
# Allowlist of workflow types to include in insight scans.
# If empty, all workflow types are scanned.
allowlist = ["OrderWorkflow", "PaymentWorkflow"]
concurrency = 50
```
