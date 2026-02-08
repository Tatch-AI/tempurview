# Contributing

## Setup

```bash
git clone https://github.com/Tatch-AI/tempurview.git
cd tempurview

# Enable git hooks (auto-rebuilds binary after each commit)
git config core.hooksPath hooks
```

## Development Workflow

```bash
# Build and install locally (makes `tpv` available in your PATH)
cargo install --path . --force --quiet

# Run tests
cargo test

# Run clippy
cargo clippy --lib --tests
```

Use `cargo install --path . --force --quiet` whenever you want to test changes via the `tpv` binary. This builds a release binary and copies it to `~/.cargo/bin`. No commit required — run it as often as you like during development.

The post-commit git hook runs this automatically after each commit, so committed code is always installed.

## Running Without Installing

If you don't need the binary on your PATH, you can also use `cargo run` directly:

```bash
# TUI with mock data
cargo run -- --mock

# CLI commands
cargo run -- workflow list --mock
cargo run -- insight scan --mock
```

## Project Structure

| Path | Purpose |
|------|---------|
| `src/app.rs` | App state, TEA update loop, effects |
| `src/event.rs` | Key-to-action mapping |
| `src/action.rs` | Action enum (all user actions) |
| `src/main.rs` | TUI render loop, CLI dispatch |
| `src/widgets/` | Ratatui widget implementations |
| `src/domain/` | Domain types, pure computation |
| `src/cli.rs` | Clap CLI definitions |
| `src/commands/` | CLI command handlers |
| `src/cli_worker.rs` | Async worker for TUI data loading |
| `src/client/` | Temporal gRPC + mock clients |
| `design-docs/` | Architecture slide decks |
