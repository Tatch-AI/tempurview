// CHOROGRAPH-ARCHITECTURE: this repo's own nodes and edges. Rust codebase, so the map is
// declared here as free-standing doc comments; each node's prose names the implementing file.
// Included in both the per-repo render and the estate-wide render (unlike anchor.ts).

/**
 * TemPurview ("Temporal purview"): a local CLI, TUI, and optional web dashboard for
 * fleet-level visibility into Harper's Temporal workflows — list/count/describe executions,
 * scan for retry storms and stuck workflows, and cancel/terminate from the terminal. Not
 * load-bearing for brokerage operations; engineers and SRE use it during incidents and
 * workflow development. Ships as a crates.io binary (`tempurview` / `tpv`). No owned data
 * stores; all state lives in Temporal.
 * @service tempurview in:Platform tech:"Rust, CLI/TUI, crates.io" tags:stale
 */

/**
 * Default entrypoint when no subcommand is given. src/main.rs → run_tui() drives a ratatui
 * TUI with nine views (workflow list, type stats, detail, activities, events, insights).
 * Uses CliWorker to serialize gRPC calls off the render thread.
 * @endpoint TUI (default)
 * @calls Temporal ListWorkflowExecutions, CountWorkflowExecutions, DescribeWorkflowExecution, GetWorkflowExecutionHistory for live dashboard data
 * @calls Temporal RequestCancelWorkflowExecution when operator presses c on a running workflow
 * @calls Temporal TerminateWorkflowExecution when operator presses t
 */

/**
 * Workflow subcommands: list, get, count, cancel, terminate. src/commands/workflow.rs.
 * Output is table on a TTY, JSON when piped (for jq / agent pipelines).
 * @endpoint CLI workflow
 * @calls Temporal visibility and control APIs matching the subcommand
 */

/**
 * Activity subcommand: list correlated activity executions for one workflow run.
 * src/commands/activity.rs — derives activities from history events.
 * @endpoint CLI activity list
 * @calls Temporal GetWorkflowExecutionHistory to reconstruct the activity timeline
 */

/**
 * Event subcommand: dump raw workflow history events. src/commands/event.rs.
 * @endpoint CLI event list
 * @calls Temporal GetWorkflowExecutionHistory
 */

/**
 * Fleet insight scan across many workflows. src/commands/insight.rs,
 * src/domain/insights_scan.rs, src/domain/insights_compute.rs — 14 finding algorithms
 * (failure rate, retry storm, stuck workflow, queue latency, child-workflow issues, etc.).
 * @endpoint CLI insight scan
 * @calls Temporal paginated ListWorkflowExecutions plus per-workflow history for deep scans
 */

/**
 * Connection profile management and resolved-config display. src/commands/config_cmd.rs;
 * profiles persist in ~/.tempurview/config.toml.
 * @endpoint CLI config
 */

/**
 * gRPC connectivity smoke test before entering the TUI. src/commands/connection.rs.
 * @endpoint CLI test-connection
 * @calls Temporal CountWorkflowExecutions to verify address, namespace, and API key
 */

/**
 * Local Axum web server (`tpv serve`). src/web/mod.rs, src/web/handlers.rs — serves
 * static/index.html and JSON APIs mirroring the CLI read paths.
 * @endpoint GET / (web UI)
 * @calls Temporal same gRPC client as CLI/TUI for list, describe, history, insights
 */

/**
 * List workflows with optional status, type, and date filters. src/web/handlers.rs.
 * @endpoint GET /api/workflows
 * @calls Temporal ListWorkflowExecutions
 */

/**
 * Single workflow execution detail. src/web/handlers.rs.
 * @endpoint GET /api/workflows/{id}
 * @calls Temporal DescribeWorkflowExecution
 */

/**
 * Correlated activity list for one execution. src/web/handlers.rs.
 * @endpoint GET /api/workflows/{id}/activities
 * @calls Temporal GetWorkflowExecutionHistory
 */

/**
 * Raw history event log. src/web/handlers.rs.
 * @endpoint GET /api/workflows/{id}/events
 * @calls Temporal GetWorkflowExecutionHistory
 */

/**
 * Fleet insights JSON API (same scan as CLI). src/web/handlers.rs.
 * @endpoint GET /api/insights
 * @calls Temporal list + history for insight algorithms
 */

/**
 * Persistent gRPC client to Temporal's WorkflowService. src/client/grpc.rs — tonic over
 * TLS for Temporal Cloud (API-key auth) or plain gRPC for local dev.
 * @module grpc-client of:tempurview
 * @calls Temporal all workflow visibility and control RPCs
 */

/**
 * In-memory mock client for demos and offline development. src/client/mock.rs.
 * @module mock-client of:tempurview
 */
