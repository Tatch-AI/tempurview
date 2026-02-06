# Views

## Workflow List

The main view. Shows workflows in a scrollable table with columns for status, type, workflow ID, and start time.

**Key actions:**
- `T` — switch to Type List view
- `I` — run an Insights scan
- `r` — refresh data
- `d` — date range filter
- `s` — sort mode
- `F1`-`F4` — toggle column visibility (Status, Type, Workflow ID, Started)

## Workflow Detail

Shows full details for a selected workflow: status, type, task queue, input/output payloads, failure info, timing, and metadata.

**Key actions:**
- `a` — view activities
- `l` — view event log
- `x` — copy workflow URL to clipboard
- `gx` — open workflow in browser
- `c` — cancel workflow
- `t` — terminate workflow

## Activity List

Lists activities for a workflow with status, type, attempt count, and timing.

**Key actions:**
- `Enter` — expand to Activity Detail

## Activity Detail

Full-screen scrollable JSON view of an activity with syntax highlighting.

**Key actions:**
- `/` — search within JSON
- `n` / `N` — next / previous search match

## Event Log

History events for a workflow in a table view, showing event type, timestamp, and event ID.

**Key actions:**
- `Enter` — expand to Event Detail

## Event Detail

Full-screen scrollable JSON view of a history event with syntax highlighting.

**Key actions:**
- `/` — search within JSON
- `n` / `N` — next / previous search match

## Type List

Workflow types with execution counts broken down by status. Selecting a type filters the Workflow List.

## Insights

Operational findings from scanning workflows. Shows a list of findings with severity and description.

**Key actions:**
- `r` — re-scan
- `Enter` — expand to Insight Detail

## Insight Detail

Full detail of a finding. Shows description, affected entities with trigger term highlighting, and links to individual workflows.

**Key actions:**
- `n` / `p` — next / previous affected entity
- `Enter` — drill into workflow detail
