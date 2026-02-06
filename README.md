# TemPurview

A CLI and TUI for viewing and managing [Temporal](https://temporal.io) workflows. The name is a portmanteau of **Temporal** and **purview** — your window into what your workflows are doing.

<!-- TODO: Add GIF/screenshot here -->

## Install

```bash
cargo install tempurview
```

## Quick start

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

Try it without a Temporal connection:

```bash
tpv --mock
```

## Documentation

Full docs are at [tatch-ai.github.io/tempurview](https://tatch-ai.github.io/tempurview/), covering configuration, CLI reference, TUI keybindings, and more.

To browse locally:

```bash
cargo install mdbook  # one-time
cd docs && mdbook serve --open
```

## Design docs

The [`design-docs/`](design-docs/) directory contains [Marp](https://marp.app) slide decks covering the project philosophy, architecture, and CLI design. Open them in any Marp-compatible viewer or VS Code with the Marp extension.

## License

MIT
