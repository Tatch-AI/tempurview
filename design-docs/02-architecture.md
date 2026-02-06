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
  blockquote {
    border-left: 4px solid #7b68ee;
    color: #b0b0d0;
  }
---

# Architecture

### How Tempurview is built

---

## The Stack

| Layer | Technology | Why |
|-------|-----------|-----|
| Language | **Rust** | Zero-cost abstractions, no runtime, single binary |
| CLI parsing | **clap 4** (derive) | Industry standard, env var support, auto-help |
| TUI framework | **ratatui** | Immediate-mode rendering, rich widgets |
| Terminal I/O | **crossterm** | Cross-platform terminal manipulation |
| gRPC client | **tonic** | Native async Rust gRPC, TLS support |
| Async runtime | **tokio** | The Rust async runtime |
| Table output | **comfy-table** | Unicode box-drawing, dynamic column widths |
| Serialization | **serde** | Zero-copy deserialization, derive macros |

---

## Module Layout

```
src/
  cli.rs              Clap derive structs
  commands/           CLI command handlers
    workflow.rs         workflow list/get/count/cancel/terminate
    activity.rs         activity list
    event.rs            event list
    insight.rs          insight scan
    config_cmd.rs       config show
    connection.rs       test-connection
  output/             Output formatting
    mod.rs              OutputFormat enum, dispatch
    json.rs             JSON output (serde_json)
    table.rs            Table output (comfy-table) + TableDisplay trait
  app.rs              TUI state machine (TEA pattern)
  cli_worker.rs       TUI async request serializer
  client/             Temporal client implementations
  domain/             Business logic + types
  widgets/            TUI widget implementations
  main.rs             Entry point + dispatch
```

---

## Two Execution Paths

```
                    Cli::parse()
                        |
              +---------+---------+
              |                   |
         No subcommand       Subcommand
              |                   |
         run_tui()          create_client()
              |                   |
    +---------+-------+     +---------+
    | App state       |     | Direct  |
    | EventHandler    |     | client  |
    | CliWorker       |     | call    |
    | Main loop       |     |         |
    +---------+-------+     +----+----+
              |                  |
         ratatui render     OutputFormat
                                |
                        +-------+-------+
                        |               |
                    Table (TTY)    JSON (pipe)
```

---

## The TEA Pattern (TUI Path)

The TUI uses **The Elm Architecture**:

```
Event → Action → update(state, action) → (State, Effects)
                                              |
                                         Side effects
                                              |
                                    CliWorker → Action
```

Inspired by Elm, Redux, and the
[ratatui component pattern](https://ratatui.rs/concepts/application-patterns/the-elm-architecture/).

- **State** is a plain struct (`App`)
- **Actions** are an enum (`Action`)
- **Effects** describe what the system should do
- **CliWorker** serializes async operations

---

## The CLI Path (Direct)

CLI commands bypass the TUI state machine entirely.

```rust
// CLI command handler
pub async fn handle(
    action: WorkflowAction,
    client: &dyn TemporalClient,
    format: OutputFormat,
    limit: u32,
) -> Result<()> {
    let workflows = client.list(&filter, limit).await?;
    output::print_list(&workflows, &workflows, format);
    Ok(())
}
```

No channels. No event loop. No intermediate state.

**Direct call → format → print → exit.**

This is why the CLI path exists separately from the
TUI path. The CliWorker exists for TUI's raw-mode
constraints (serialized async over mpsc channels).
CLI commands don't have that constraint.

---

## Client Abstraction

```rust
#[async_trait]
pub trait TemporalClient: Send + Sync {
    async fn count(&self, query: Option<&str>) -> ClientResult<u64>;
    async fn list(&self, filter: &WorkflowFilter, limit: u32)
        -> ClientResult<Vec<WorkflowSummary>>;
    async fn describe(&self, workflow_id: &str, run_id: Option<&str>)
        -> ClientResult<WorkflowDetail>;
    async fn get_history(&self, workflow_id: &str, run_id: Option<&str>)
        -> ClientResult<Vec<HistoryEvent>>;
    async fn cancel(&self, workflow_id: &str, run_id: Option<&str>)
        -> ClientResult<()>;
    async fn terminate(&self, workflow_id: &str, run_id: Option<&str>, reason: &str)
        -> ClientResult<()>;
}
```

Three implementations:
- **GrpcTemporalClient** -- production (tonic + TLS)
- **MockTemporalClient** -- development (`--mock`)
- **CliTemporalClient** -- legacy (shells out to `temporal`)

---

## Domain Layer

Pure business logic with no I/O dependencies:

- **`correlate_activities()`** -- builds ActivityExecution from raw history events
- **`correlate_child_workflows()`** -- same for child workflows
- **`compute_list_findings()`** -- fleet-level insight algorithms
- **`compute_activity_findings()`** -- activity-level insight algorithms
- **`run_insights_scan()`** -- orchestrates the full scan pipeline

All functions take data in, return data out.
Testable without a Temporal server.

**152 unit tests**, all pure functions.

---

## Output Layer: The TableDisplay Trait

```rust
pub trait TableDisplay {
    fn to_table(&self) -> comfy_table::Table;
}
```

Implemented for every domain type that can be displayed:

| Type | Table Format |
|------|-------------|
| `Vec<WorkflowSummary>` | STATUS, TYPE, ID, STARTED, QUEUE |
| `WorkflowDetail` | Key-value pairs |
| `Vec<ActivityExecution>` | ID, TYPE, STATUS, ATTEMPT, WAIT, TIME |
| `Vec<HistoryEvent>` | EVENT ID, TYPE, TIMESTAMP |
| `InsightsResult` | SEVERITY, CATEGORY, TITLE, AFFECTED |
| `StatusCounts` | STATUS, COUNT |
| `Config` | SETTING, VALUE |

Every type also derives `Serialize` for JSON output.
One trait, two formats, zero duplication.

---

## Configuration Resolution

```
Priority (highest first):
  1. CLI flags (--address, --namespace, --mock, --limit)
  2. Environment variables (TEMPORAL_ADDRESS, etc.)
  3. ~/.tempurview/.env file
  4. Defaults (localhost:7233, namespace "default")
```

No YAML. No JSON config. No config directory discovery.

- **TOML** for the one config file (`~/.tempurview/config.toml`)
- **Environment variables** for everything else
- **CLI flags** override everything

Same pattern as **kubectl**, **docker**, **terraform**.

---

## Error Handling

```
CLI mode:
  Error → color_eyre → stderr + exit code 1

TUI mode:
  Error → Action::Error(String) → UI error bar
  (non-fatal, keeps running)
```

CLI errors propagate naturally with `?`.
TUI errors are caught and displayed without crashing.

The split is intentional. A CLI tool should fail loudly.
A TUI should be resilient.

---

## Proto Build Strategy

```
Dev machine (submodule present):
  build.rs detects proto/temporal-api/
  → tonic-build regenerates into src/proto/generated/
  → commit the generated .rs files

crates.io install (no submodule):
  build.rs skips proto compilation
  → uses checked-in src/proto/generated/*.rs
  → zero protoc/proto dependency
```

**Why not ship .proto files?**
- Saves ~2.5 MiB of raw protos from the package
- Eliminates protoc system dependency for consumers
- `cargo install tempurview` just works

**To regenerate** (after updating proto submodule):
```bash
git submodule update --remote proto/temporal-api
cargo build   # build.rs regenerates src/proto/generated/
```
