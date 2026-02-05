# Tempurview

A terminal user interface for viewing and managing Temporal workflows.

## Features

- **Dashboard View**: Real-time workflow counts by status (Running, Completed, Failed, etc.)
- **Workflow List**: Scrollable, filterable list of workflows with status, type, and timing info
- **Workflow Details**: Detailed view with input/output, failure info, and metadata
- **Filtering**: Filter by status (number keys) or custom query (/)
- **Actions**: Cancel or terminate workflows directly from the TUI

## Installation

```bash
cargo install --path .
```

Or run directly:

```bash
cargo run -- --mock
```

## Usage

### With Temporal Cloud or Server

Set the required environment variables (or use a `.env` file):

```bash
export TEMPORAL_ADDRESS="your-namespace.tmprl.cloud:7233"
export TEMPORAL_NAMESPACE="your-namespace"
export TEMPORAL_API_KEY="your-api-key"  # Optional, for Temporal Cloud

tempurview
```

Or create a `.env` file in your working directory:

```env
TEMPORAL_ADDRESS=your-namespace.tmprl.cloud:7233
TEMPORAL_NAMESPACE=your-namespace
TEMPORAL_API_KEY=your-api-key
```

Then simply run:

```bash
tempurview
```

### With Mock Data

For testing or demo purposes:

```bash
tempurview --mock
tempurview --mock --mock-count 500  # Generate 500 mock workflows
```

### Command Line Options

```
OPTIONS:
    --mock              Use mock data instead of connecting to Temporal
    --mock-count N      Number of mock workflows to generate (default: 100)
    --address ADDR      Temporal server address (overrides TEMPORAL_ADDRESS)
    --namespace NS      Temporal namespace (overrides TEMPORAL_NAMESPACE)
    --limit N           Maximum workflows to fetch (default: 50)
    -h, --help          Show help message
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `↓` | Navigate down |
| `k` / `↑` | Navigate up |
| `g` / `Home` | Go to top |
| `G` / `End` | Go to bottom |
| `PgUp` / `PgDn` | Page up/down |
| `Enter` | Select / View details |
| `Esc` | Go back |
| `1` | Filter: Running |
| `2` | Filter: Completed |
| `3` | Filter: Failed |
| `4` | Filter: Canceled |
| `5` | Filter: Terminated |
| `0` | Clear all filters |
| `/` | Open filter input |
| `r` | Refresh data |
| `c` | Cancel workflow (detail view) |
| `t` | Terminate workflow (detail view) |
| `?` | Toggle help overlay |
| `q` / `Ctrl+C` | Quit |

## Architecture

Tempurview follows an Elm-inspired architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Application                                │
├─────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────┐  │
│  │    Event     │───▶│     App      │───▶│      Terminal        │  │
│  │   Handler    │    │    State     │    │      Renderer        │  │
│  └──────────────┘    └──────────────┘    └──────────────────────┘  │
│         │                   │                      │                │
│         ▼                   ▼                      ▼                │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────┐  │
│  │   Actions    │    │    Domain    │    │      Widgets         │  │
│  │   (enum)     │    │    Types     │    │   (pure render)      │  │
│  └──────────────┘    └──────────────┘    └──────────────────────┘  │
│                             │                                       │
│                             ▼                                       │
│                      ┌──────────────┐                               │
│                      │   Temporal   │                               │
│                      │    Client    │                               │
│                      │   (trait)    │                               │
│                      └──────────────┘                               │
└─────────────────────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Separation of Concerns**: UI rendering, state management, data fetching, and event handling are distinct modules
2. **Pure Functions**: Business logic uses pure functions that can be unit tested without terminal/async dependencies
3. **Trait-Based Abstraction**: External dependencies (Temporal CLI) hidden behind traits for easy mocking
4. **Unidirectional Data Flow**: Events → State Updates → Render cycle

### Module Structure

```
src/
├── main.rs           # Entry point, terminal setup, main loop
├── app.rs            # App state and update logic
├── action.rs         # Action enum (user intents)
├── event.rs          # Event handling (keyboard, tick)
├── config.rs         # Configuration (CLI args, env vars)
├── tui.rs            # Terminal wrapper
├── domain/           # Domain types (pure, testable)
│   ├── workflow.rs
│   ├── workflow_filter.rs
│   └── stats.rs
├── client/           # Temporal client abstraction
│   ├── trait.rs      # TemporalClient trait
│   ├── cli.rs        # CLI-based implementation
│   └── mock.rs       # Mock for testing
└── widgets/          # UI components (stateless renderers)
    ├── status_dashboard.rs
    ├── workflow_list.rs
    ├── workflow_detail.rs
    ├── help_bar.rs
    └── filter_input.rs
```

## Development

### Running Tests

```bash
cargo test
```

### Running with Debug Output

```bash
RUST_LOG=debug cargo run -- --mock
```

### Linting

```bash
cargo clippy
```

## Dependencies

- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Cross-platform terminal manipulation
- [tokio](https://tokio.rs/) - Async runtime
- [chrono](https://github.com/chronotope/chrono) - Date/time handling
- [serde](https://serde.rs/) - Serialization

## License

MIT
