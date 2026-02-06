---
marp: true
theme: ember
paginate: true
---

<script type="module">
import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
mermaid.initialize({ startOnLoad: true, theme: 'dark', flowchart: { htmlLabels: true, nodeSpacing: 30, rankSpacing: 30 } });
</script>

# Architecture

### How TemPurview is built

---

## The Stack

| Layer | Technology | Why |
|-------|-----------|-----|
| Language | **Rust** | Zero-cost abstractions, no runtime, single binary |
| Temporal API | **gRPC + Protobuf** | Temporal's official API surface; typed, versioned |
| gRPC client | **tonic** | Native async Rust gRPC, TLS support |
| CLI parsing | **clap 4** (derive) | Industry standard, env var support, auto-help |
| TUI framework | **ratatui** | Immediate-mode rendering, rich widgets |
| Terminal I/O | **crossterm** | Cross-platform terminal manipulation |
| Async runtime | **tokio** | The Rust async runtime |
| Table output | **comfy-table** | Unicode box-drawing, dynamic column widths |
| Serialization | **serde** | Zero-copy deserialization, derive macros |
| Release mgmt | **prek** | Automated versioning, changelog, crates.io publish |
| CI/CD | **GitHub Actions** | Build, test, lint, publish on every push/PR |

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

<div class="mermaid">
graph LR
    A["Cli::parse()"] --> B{Subcommand?}
    B -->|None| C["run_tui()"]
    B -->|Some| D["create_client()"]
    C --> E["App + CliWorker"]
    E --> F["ratatui"]
    D --> G["Direct call"]
    G --> H{"OutputFormat"}
    H --> I["Table"]
    H --> J["JSON"]
    style A fill:#1d3a47,stroke:#5097b7,color:#f6e1ce
    style B fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style C fill:#1d3a47,stroke:#5097b7,color:#f6e1ce
    style D fill:#1d3a47,stroke:#5097b7,color:#f6e1ce
    style E fill:#295264,stroke:#5097b7,color:#f6e1ce
    style F fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style G fill:#295264,stroke:#5097b7,color:#f6e1ce
    style H fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style I fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style J fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
</div>

---

## The TEA Pattern (TUI Path)

The TUI uses **The Elm Architecture**:

<div class="mermaid">
graph LR
    A["Event"] --> B["Action"]
    B --> C["update(state, action)"]
    C --> D["State + Effects"]
    D --> E["Side effects"]
    E --> F["CliWorker"]
    F --> B
    style A fill:#1d3a47,stroke:#5097b7,color:#f6e1ce
    style B fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style C fill:#1d3a47,stroke:#5097b7,color:#f6e1ce
    style D fill:#295264,stroke:#22c55e,color:#f6e1ce
    style E fill:#1d3a47,stroke:#ff6d63,color:#f6e1ce
    style F fill:#295264,stroke:#5097b7,color:#f6e1ce
</div>

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

<!-- _class: cols -->

## Output Layer: The TableDisplay Trait

<div class="left">

```rust
pub trait TableDisplay {
    fn to_table(&self) -> comfy_table::Table;
}
```

Every type also derives `Serialize` for JSON output.
One trait, two formats, zero duplication.

</div>
<div class="right">

| Type | Columns |
|------|---------|
| `Vec<WorkflowSummary>` | STATUS, TYPE, ID, ... |
| `WorkflowDetail` | Key-value pairs |
| `Vec<ActivityExecution>` | ID, TYPE, STATUS, ... |
| `Vec<HistoryEvent>` | ID, TYPE, TIMESTAMP |
| `InsightsResult` | SEV, CAT, TITLE, ... |
| `StatusCounts` | STATUS, COUNT |
| `Config` | SETTING, VALUE |

</div>

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

<div class="mermaid">
graph LR
    subgraph CLI Mode
        A1["Error"] --> B1["color_eyre"]
        B1 --> C1["stderr + exit 1"]
    end
    subgraph TUI Mode
        A2["Error"] --> B2["Action::Error"]
        B2 --> C2["UI error bar\n(non-fatal)"]
    end
    style A1 fill:#1d3a47,stroke:#ff6d63,color:#f6e1ce
    style B1 fill:#295264,stroke:#5097b7,color:#f6e1ce
    style C1 fill:#1d3a47,stroke:#ff6d63,color:#f6e1ce
    style A2 fill:#1d3a47,stroke:#ff6d63,color:#f6e1ce
    style B2 fill:#295264,stroke:#5097b7,color:#f6e1ce
    style C2 fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
</div>

CLI errors propagate naturally with `?`.
TUI errors are caught and displayed without crashing.

The split is intentional. A CLI tool should fail loudly.
A TUI should be resilient.

---

<!-- _class: compact -->

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
