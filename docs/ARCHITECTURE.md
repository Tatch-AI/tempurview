# Architecture: tempurview

Evidence-based map of the Rust codebase. TemPurview is a **single-binary client** with no database, no message bus, and no deployable server in Harper's estate — operators run it locally or in CI scripts.

## Entrypoints

| Entry | Trigger | Implementation |
|---|---|---|
| **TUI (default)** | `tpv` or `tempurview` with no subcommand | `src/main.rs` → `run_tui()` |
| **CLI workflow** | `tpv workflow list\|get\|count\|cancel\|terminate` | `src/commands/workflow.rs` |
| **CLI activity** | `tpv activity list <workflow-id>` | `src/commands/activity.rs` |
| **CLI event** | `tpv event list <workflow-id>` | `src/commands/event.rs` |
| **CLI insight** | `tpv insight scan [--since …]` | `src/commands/insight.rs`, `src/domain/insights_scan.rs` |
| **CLI config** | `tpv config show\|profile-add\|profile-list\|profile-remove\|set-default` | `src/commands/config_cmd.rs` |
| **CLI test-connection** | `tpv test-connection` | `src/commands/connection.rs` |
| **CLI completions** | `tpv completions {bash,zsh,fish}` | `src/main.rs` |
| **Web server** | `tpv serve [--port 3000] [--bind 127.0.0.1]` | `src/web/mod.rs` |
| **Shell completions** | Generated at install time | clap_complete in `src/main.rs` |

Global flags (`src/cli.rs` `GlobalArgs`): `--profile`, `--address`, `--namespace`, `--mock`, `--mock-count`, `--limit`, `--output`, `--watch`, `--interval`, `--logs`.

## HTTP routes (web mode only)

| Method | Path | Handler | Notes |
|---|---|---|---|
| `GET` | `/` | `handlers::index` | Embedded `static/index.html` SPA |
| `GET` | `/api/workflows` | `handlers::list_workflows` | Query: `status`, `workflow_type`, `since`, `before`, `limit` |
| `GET` | `/api/workflows/{id}` | `handlers::get_workflow` | Query: `run_id` |
| `GET` | `/api/workflows/{id}/activities` | `handlers::list_activities` | Correlated from history |
| `GET` | `/api/workflows/{id}/events` | `handlers::list_events` | Raw history events |
| `GET` | `/api/insights` | `handlers::get_insights` | Query: `since`, `before`, `limit` |
| `POST` | `/api/workflows/{id}/cancel` | `handlers::cancel_workflow` | **501 stub** |
| `POST` | `/api/workflows/{id}/terminate` | `handlers::terminate_workflow` | **501 stub** |

## Data stores

**None owned.** TemPurview is stateless relative to Harper's data platform.

| Store | Access | Purpose |
|---|---|---|
| `~/.tempurview/config.toml` | Read/write | Connection profiles, insights allowlist, concurrency |
| `~/.tempurview/logs/tempurview.log*` | Write | Local tracing logs (`src/logging.rs`) |
| Temporal workflow store | Read (+ cancel/terminate via API) | All workflow visibility data |

No `harper-supabase`, Redis, S3, or Kafka usage.

## Events

**None.** TemPurview does not emit or consume Relay/Kafka topics or legacy eventbus messages.

## External services

| Service | Protocol | Why |
|---|---|---|
| **Temporal** | gRPC (tonic) — `WorkflowService` | `ListWorkflowExecutions`, `CountWorkflowExecutions`, `DescribeWorkflowExecution`, `GetWorkflowExecutionHistory`, `RequestCancelWorkflowExecution`, `TerminateWorkflowExecution` (`src/client/grpc.rs`) |

Connection params: `TEMPORAL_ADDRESS` (default `localhost:7233`), `TEMPORAL_NAMESPACE` (default `default`), `TEMPORAL_API_KEY` (required for Temporal Cloud; enables TLS).

## Key flows

### 1. TUI dashboard load

```
main → create_client (gRPC or --mock) → App + CliWorker
  → load_counts + load_workflows (async via mpsc)
  → ratatui render loop (TEA: Action → update → Effect)
```

`CliWorker` (`src/cli_worker.rs`) serializes all Temporal RPCs so the TUI thread never blocks on network I/O.

### 2. Fleet insight scan

```
insight scan → run_insights_scan (insights_scan.rs)
  → paginated list_streaming from Temporal
  → compute_list_findings (workflows only)
  → per-workflow history fetch (concurrency from InsightsConfig)
  → compute_history_findings (retry storms, activity failures, …)
  → InsightResult JSON/table
```

Fourteen `InsightCategory` values in `src/domain/insights.rs`. Configurable allowlist and concurrency via `~/.tempurview/config.toml` `[insights]` section.

### 3. CLI JSON pipeline

```
tpv workflow list --status failed -o json | jq …
```

`OutputFormat::resolve` (`src/output/mod.rs`) picks table vs JSON based on TTY unless `--output` is set.

### 4. Web UI

```
tpv serve → axum::serve on bind:port
  → same Arc<dyn TemporalClient> as CLI
  → static SPA + /api/* JSON handlers
```

## Configuration / environment

Resolution order (`src/config.rs`): **CLI flags > profile > env vars > defaults**.

| Variable / file | Default | Purpose |
|---|---|---|
| `TEMPORAL_ADDRESS` | `localhost:7233` | gRPC endpoint |
| `TEMPORAL_NAMESPACE` | `default` | Temporal namespace |
| `TEMPORAL_API_KEY` | — | Temporal Cloud auth |
| `TEMPURVIEW_PROFILE` | — | Active profile name |
| `TEMPORAL_TUI_REFRESH_INTERVAL` | `30` | TUI auto-refresh seconds |
| `TEMPORAL_TUI_DEFAULT_LIMIT` | unlimited (`0` → `u32::MAX`) | Max workflows per fetch |
| `TEMPORAL_TUI_TICK_RATE` | `250` | TUI tick ms |
| `TEMPURVIEW_INSIGHTS_CONCURRENCY` | — | Parallel history fetches during scan |
| `~/.tempurview/config.toml` | — | Profiles + insights config |
| `~/.tempurview/.env` | — | Fallback dotenv |

## Deploy / release story

- **Not deployed to Harper infra.** Distributed as a Rust crate on [crates.io](https://crates.io/crates/tempurview).
- **CI** (`.github/workflows/ci.yml`): `cargo build`, `cargo test --lib`, `cargo publish --dry-run` on every PR/push to `main`.
- **Release** (`.github/workflows/release.yml`): `npx autorel@^2` on push to `main` — conventional commits → semver tag → `cargo publish` with `CARGO_REGISTRY_TOKEN`.
- **Docs** (`.github/workflows/docs.yml`): mdBook to GitHub Pages (`tatch-ai.github.io/tempurview/`).
- **Pre-commit**: tracked hooks in `hooks/pre-commit` (rustfmt, clippy).

## Module layout (summary)

```
src/
  main.rs           Entry + TUI loop
  cli.rs            Clap command tree
  app.rs            TUI state machine (TEA)
  cli_worker.rs     Async RPC serializer for TUI
  client/           TemporalClient trait, gRPC + mock impls
  commands/         CLI handlers
  domain/           Types, filters, insight algorithms
  widgets/          ratatui views (9 TUI screens)
  web/              Axum server + handlers
  output/           Table vs JSON formatting
  proto/generated/  Pre-generated Temporal API protos
```

Build: `build.rs` can regenerate protos via `tonic-build`; CI uses pre-generated code for crates.io compatibility.
