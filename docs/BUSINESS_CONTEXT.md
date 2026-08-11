# Business context: tempurview

## What this repo is for

**TemPurview** (repo name `tempurview`, binaries `tempurview` and `tpv`) is an **operator console for Harper's Temporal workflow fleet**. It does not participate in the insurance brokerage funnel (leads → intake → quoting → binding → payments → servicing → renewals). It exists so engineers and SRE can answer: *what are our workflows doing right now, and which types are failing or stuck?*

Temporal is Harper's durable workflow engine (`Platform` domain). Services like **hercules**, **relay-server**, and **sluice** run long-running business processes there — quote placement, event fanout, data pipelines. Temporal's own web UI is built for inspecting **one workflow at a time**. TemPurview adds **fleet-level visibility**: scan hundreds of executions, rank workflow types by failure rate, surface retry storms, and pipe JSON into `jq` or agent scripts.

**Funnel stage:** none (internal platform tooling).

**Money flow:** none.

**Human workflow served:** on-call engineer or workflow author debugging a production incident, or a developer validating local Temporal behavior (`tpv --mock`).

## Who depends on it

| Consumer | Relationship |
|---|---|
| Platform / backend engineers | Primary users — install via `cargo install tempurview` or `cargo install --path .` |
| SRE / on-call | Incident triage: `tpv insight scan --since 2h`, `tpv workflow list --status failed` |
| AI agents / shell automation | JSON output mode (`-o json`, auto-detected when piped) |
| Temporal Cloud / self-hosted Temporal | **Upstream dependency** — all data comes from Temporal's gRPC WorkflowService API |

No Harper microservice calls tempurview. It is a **client-only** tool. No HTTP API is exposed in production (the `tpv serve` web UI binds `127.0.0.1` by default for local use).

## Domain concepts

| Term | Meaning |
|---|---|
| **Workflow execution** | A single run of a Temporal workflow type, identified by `workflow_id` + `run_id`. Harper's business processes (e.g. quote placement, relay consumption) are implemented as workflow types registered with workers. |
| **Workflow type** | The registered name of a workflow implementation (e.g. `PaymentWorkflow`). TemPurview aggregates stats and insights per type. |
| **Activity** | A unit of side-effecting work inside a workflow (API call, DB write). TemPurview correlates activity events from execution history. |
| **Insight / finding** | A heuristic alert from fleet scan — retry storm, stuck running workflow, all-failed type, queue latency, etc. Fourteen categories in `InsightCategory`. |
| **Connection profile** | Named Temporal endpoint saved in `~/.tempurview/config.toml` (address, namespace, API key) so operators can switch between local, staging, and Temporal Cloud namespaces. |
| **Namespace** | Temporal isolation boundary; Harper likely has separate namespaces per environment. |

## Operational status: **stale**

Estate inventory tags this repo **stale, tier 3, Platform domain**. Evidence:

- Last meaningful commit: **2026-02-13** (multi-profile support). No commits in ~6 months as of the documentation pass.
- **Not prod-core** — brokerage operations do not depend on tempurview being up. If it vanished, workflows would keep running; only operator visibility would suffer.
- **Actively published** — CI on `main` still builds/tests; `autorel` tags releases and publishes to **crates.io** when conventional commits land.
- **Feature-complete for its scope** — TUI, CLI, web UI, insights, profiles, `--watch` mode, mock client. Web cancel/terminate are stubs (501).

**Verdict:** useful internal tool, maintained opportunistically, safe to deprioritize in a platform rewrite unless operator UX for Temporal is explicitly in scope.

## Rewrite notes

### Behavior that should survive (if rebuilt)

- **Fleet insight scan** — the 14 finding algorithms in `src/domain/insights_compute.rs` encode operational knowledge (retry storms, stuck workflows, child-workflow latency). Worth preserving or porting to whatever observability stack replaces this.
- **JSON-as-API CLI** — pipe-friendly output is intentional for scripting and agents.
- **Multi-profile config** — engineers routinely switch between local Temporal and Temporal Cloud namespaces.

### Known debt

- Web UI cancel/terminate return 501 (`src/web/handlers.rs`).
- No integration with Harper's central auth — operators supply Temporal API keys directly.
- Proto code is vendored/pre-generated; submodule path exists but CI builds without submodules (crates.io path).

### Overlap with siblings

- **Temporal Web UI** — per-workflow inspection; tempurview complements, does not replace.
- **harper-sre / eye** — other Platform observability; no code coupling found.
- No overlap with brokerage services (hercules, relay-server, etc.) — tempurview only **reads** their workflow executions via Temporal's API.
