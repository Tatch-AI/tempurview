# TUI Overview

Launch the interactive TUI by running `tpv` without a subcommand:

```bash
tpv
```

The TUI provides a navigable, real-time interface to your Temporal cluster. It follows vim-style keybindings throughout.

## Views

The TUI has several interconnected views:

- **Workflow List** — the main view. Lists workflows with status, type, ID, and timing. Supports filtering, sorting, and date range selection.
- **Workflow Detail** — detailed info for a single workflow including input/output payloads, failure messages, and metadata.
- **Activity List** — activities for a workflow, with status and timing.
- **Activity Detail** — full JSON detail of a single activity with syntax highlighting and search.
- **Event Log** — the complete history event table for a workflow.
- **Event Detail** — full JSON detail of a single event with syntax highlighting and search.
- **Type List** — workflow types with counts broken down by status.
- **Insights** — operational findings from scanning workflows.
- **Insight Detail** — full detail of a single finding with affected entities, trigger term highlighting, and drill-through to workflows.

## Navigation model

Views are arranged as a stack. `Enter` pushes a detail view, `Esc` pops back. The typical flow:

```
Workflow List → Workflow Detail → Activity List → Activity Detail
                     │
                     ├──→ Event Log → Event Detail
                     │
Workflow List → Type List
                     │
Workflow List → Insights → Insight Detail → Workflow Detail
```
