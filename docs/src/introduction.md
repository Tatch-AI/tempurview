# Tempurview

A CLI and TUI for viewing and managing [Temporal](https://temporal.io) workflows.

Tempurview gives you two interfaces to your Temporal cluster:

- **CLI** — scriptable commands for listing, inspecting, and acting on workflows. Outputs JSON (for pipes) or tables (for humans).
- **TUI** — an interactive terminal UI with real-time navigation, filtering, sorting, and operational insights.

Both interfaces share the same configuration and connect to Temporal via gRPC.

## Quick start

```bash
cargo install tempurview
```

Connect to your cluster:

```bash
export TEMPORAL_ADDRESS="your-namespace.tmprl.cloud:7233"
export TEMPORAL_NAMESPACE="your-namespace"
export TEMPORAL_API_KEY="your-api-key"

# Launch the TUI
tpv

# Or use the CLI
tpv workflow list
tpv workflow get <workflow-id>
tpv insight scan --since 24h
```

Try it with mock data (no Temporal connection needed):

```bash
tpv --mock
```
