---
marp: true
theme: ember
paginate: true
---

<script type="module">
import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
mermaid.initialize({ startOnLoad: true, theme: 'dark', flowchart: { htmlLabels: true, nodeSpacing: 30, rankSpacing: 30 } });
</script>

# Fleet Analytics Algorithms

### How TemPurview turns workflow data into operational findings

---

## The Problem

At 3am, operators don't ask "show me workflow X."

They ask:

```
"Is anything broken right now?"
"Which activity type is causing the most pain?"
"Are we in a retry storm?"
"Why did this type start failing after the last deploy?"
```

Single-workflow inspection can't answer fleet-level questions.
You need algorithms that scan across workflows and surface patterns.

---

## Architecture: Three-Layer Pipeline

<div class="mermaid">
graph TD
    A["Workflow List<br>(all matching filter)"] --> B["List-Level Findings"]
    A --> C["Priority Sampling<br>(up to 30 histories)"]
    C --> D["Activity Correlation"]
    C --> E["Child WF Correlation"]
    D --> F["Activity-Level Findings"]
    E --> G["Child WF Findings"]
    B --> H["Rank & Merge"]
    F --> H
    G --> H
    H --> I["Ordered InsightFindings"]
    style A fill:#295264,stroke:#5097b7,color:#f6e1ce
    style B fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style C fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style D fill:#1d3a47,stroke:#5097b7,color:#f6e1ce
    style E fill:#1d3a47,stroke:#5097b7,color:#f6e1ce
    style F fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style G fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
    style H fill:#295264,stroke:#ff6d63,color:#f6e1ce
    style I fill:#1d3a47,stroke:#22c55e,color:#f6e1ce
</div>

Layer 1 needs only the workflow list. Layers 2 and 3 need sampled history events.

---

## Scan Pipeline

The full pipeline runs in `run_insights_scan()`:

1. **Fetch workflows** matching the date filter (ignores status/type filters)
2. **Compute list-level findings** from workflow metadata alone
3. **Select up to 30 workflows for history sampling** via priority queue
4. **Fetch histories** and correlate activities + child workflows
5. **Compute activity-level + child workflow findings**
6. **Rank all findings** by severity, then affected count

Sampling priority order:
**Failed/Terminated/TimedOut** > **Running** (oldest first) > **Completed** (recent)

This ensures the most problematic workflows are always inspected first.

---

## Finding Anatomy

Every algorithm produces one or more `InsightFinding` values:

```rust
pub struct InsightFinding {
    pub severity: InsightSeverity,     // Info | Warning | Critical
    pub category: InsightCategory,     // Which algorithm produced it
    pub title: String,                 // One-line summary
    pub detail: String,                // Explanation + evidence
    pub affected_entities: Vec<String>,// Workflow IDs, queue names, etc.
    pub trigger_terms: Vec<String>,    // Values that triggered it (for highlighting)
    pub computed_at: DateTime<Utc>,
}
```

Severity drives ranking. Affected count breaks ties.
Trigger terms power the TUI's detail-view highlighting.

---

## List-Level: High Failure Rate

**What it detects**: Workflow types with unusually high failure rates.

Groups workflows by type, counts failures (Failed + Terminated + TimedOut).

```rust
pub const MIN_WORKFLOWS_FOR_RATE: usize = 3;
pub const FAILURE_RATE_INFO: f64 = 0.10;      // 10%
pub const FAILURE_RATE_WARNING: f64 = 0.25;    // 25%
pub const FAILURE_RATE_CRITICAL: f64 = 0.50;   // 50%
```

Requires at least 3 workflows of a type to avoid noise from small samples.

Example output: `Payment: 12/16 failed (75%)` -- Critical

---

## List-Level: All-Failed Type

**What it detects**: Workflow types where every single execution has failed.

```rust
pub const ALL_FAILED_MIN_WORKFLOWS: usize = 2;
```

If all N executions of a type are Failed/Terminated/TimedOut (N >= 2),
this fires as **Critical** and the type is marked completely broken.

**Deduplication**: When All-Failed fires, the Failure Rate finding is
skipped for that type (`continue` in the loop). All-Failed is strictly
more specific -- 100% is a different signal than "high."

---

## List-Level: Stuck Workflows

**What it detects**: Running workflows that have been running "too long."

```rust
pub const STUCK_WARNING_HOURS: i64 = 2;
pub const STUCK_CRITICAL_HOURS: i64 = 6;
```

Groups stuck workflows by type. Reports the longest-running duration.
The severity is determined by the **worst case** in each type group.

Example output: `3 DataProc workflows running > 2h` -- Warning
(longest: 5h)

---

## Activity-Level: Retry Storm

**What it detects**: Activity types with abnormally high average retry counts.

Collects `attempt` values across all sampled instances of each activity type.

```rust
pub const RETRY_MIN_INSTANCES: usize = 3;
pub const RETRY_WARNING_AVG: f64 = 2.0;
pub const RETRY_CRITICAL_AVG: f64 = 3.0;
```

Requires at least 3 instances to compute a meaningful average.
Reports the max individual attempt count alongside the average.

Example: `SendEmail: avg 4.2 attempts (8 workflows)` -- Critical

---

## Activity-Level: Activity Retry

**What it detects**: Individual activities that have retried (attempt >= 2).

```rust
pub const ACTIVITY_RETRY_MIN_ATTEMPT: i32 = 2;
```

Severity scales with the count of retried instances:
- **1-2** retried activities: Info
- **3-4**: Warning
- **5+**: Critical

**Complement to Retry Storm**: Retry Storm detects *systemic* retry issues
via averages. Activity Retry catches *individual* retries that might
not raise the average enough to trigger Retry Storm.

---

## Activity-Level: Queue Wait Latency

**What it detects**: Task queues where workers are slow to pick up activities.

Computes the **median** queue wait time per task queue across all samples.

```rust
pub const QUEUE_LATENCY_WARNING_MS: i64 = 1000;   // 1s
pub const QUEUE_LATENCY_CRITICAL_MS: i64 = 5000;   // 5s
```

Median is chosen over mean to resist outlier skew.

Example: `'order-processing': 3.2s median queue wait` -- Warning

High queue wait usually means: insufficient workers, or workers
are stuck processing long activities.

---

## Activity-Level: Failure Hotspot

**What it detects**: Activity types with concentrated failures.

Counts failed activities per type. Finds the most common error message
via histogram.

```rust
pub const ACTIVITY_FAILURE_WARNING: usize = 3;
pub const ACTIVITY_FAILURE_CRITICAL: usize = 5;
```

The detail includes the most frequent error message, giving operators
an immediate clue about root cause without opening individual workflows.

Example: `ValidateInput: 5 failures in 4 workflows` -- Critical
`Common error: connection refused to validation-service:8080`

---

## Activity-Level: Long-Running Activity

**What it detects**: Activities stuck in `Running` status for an extended time.

Checks elapsed time since `started_time` for any currently-running activity.

```rust
pub const LONG_ACTIVITY_WARNING_MINS: i64 = 30;
pub const LONG_ACTIVITY_CRITICAL_MINS: i64 = 120;
```

Groups by activity type in the title. Reports the longest duration.

Example: `2 activities running > 30min: ProcessPayment(1), SyncData(1)` -- Warning
(longest: 45min)

Long-running activities may indicate stuck workers, infinite loops,
or missing heartbeat timeouts.

---

<!-- _class: compact -->

## Activity-Level: Error in I/O

**What it detects**: Error patterns hidden in activity input/output/failure,
even when the workflow reports success.

Scans three fields per activity: `output`, `input`, `failure.message`.

```rust
pub const ERROR_PATTERNS: &[&str] = &[
    "error", "exception", "failed", "failure",
    "timed out", "timeout", "panic", "fatal",
    "unhandled", "traceback", "stack trace", "errno",
];
```

12 patterns, case-insensitive. Extracts a snippet around the match for context.

```rust
pub const ERROR_IN_OUTPUT_WARNING: usize = 2;
pub const ERROR_IN_OUTPUT_CRITICAL: usize = 5;
```

---

## The Allowlist Pattern

Error in I/O can produce false positives. Insurance workflows might have
"errors and omissions" as legitimate business data, not an actual error.

**Solution**: `~/.tempurview/config.toml`

```toml
[insights]
allowlist = [
    "errors and omissions",
    "error_code_field_name",
    "failure mode analysis",
]
```

Matching is **case-insensitive**. If any allowlisted phrase appears in the
same text that triggered the error pattern, the match is suppressed.

```rust
pub fn is_allowlisted(&self, text: &str) -> bool {
    let text_lower = text.to_lowercase();
    self.allowlist
        .iter()
        .any(|phrase| text_lower.contains(phrase.to_lowercase().as_str()))
}
```

---

## Child Workflow: Failure Hotspot

**What it detects**: Child workflow types with concentrated failures
across parent workflows.

```rust
pub const CHILD_WF_FAILURE_WARNING: usize = 2;
pub const CHILD_WF_FAILURE_CRITICAL: usize = 5;
```

Failed statuses: Failed, TimedOut, Canceled, Terminated, StartFailed.

Reports the count of failures and the number of affected parent workflows.

Example: `ProcessOrder: 5 failures across 4 parent workflows` -- Critical

---

## Child Workflow: Start Latency

**What it detects**: Child workflow types with high median start latency
(time from initiated to started).

```rust
pub const CHILD_WF_LATENCY_WARNING_MS: i64 = 2000;    // 2s
pub const CHILD_WF_LATENCY_CRITICAL_MS: i64 = 10000;   // 10s
```

Uses median to resist outlier skew, same as queue wait latency.

Example: `ProcessOrder: 3.5s median start latency` -- Warning
(across 12 child workflow executions)

High start latency may indicate task queue congestion or
resource contention on the child workflow's worker pool.

---

## Ranking and Output

All findings from all three layers are merged into a single list,
then sorted:

1. **Primary**: Severity descending (Critical > Warning > Info)
2. **Tiebreak**: Affected entity count descending

```rust
pub fn rank_findings(mut findings: Vec<InsightFinding>) -> Vec<InsightFinding> {
    findings.sort_by(|a, b| {
        b.severity.cmp(&a.severity)
            .then_with(|| b.affected_entities.len().cmp(&a.affected_entities.len()))
    });
    findings
}
```

Critical findings with many affected workflows always surface first.
This gives operators the most urgent, highest-impact items at the top.

---

<!-- _class: compact -->

## All Thresholds at a Glance

| Algorithm | Warning | Critical | Min / Notes |
|-----------|---------|----------|-------------|
| High Failure Rate | 25% | 50% | >= 3 workflows; Info at 10% |
| All-Failed Type | -- | 100% | >= 2 workflows |
| Stuck Workflows | 2h | 6h | Running status only |
| Retry Storm | avg 2.0 | avg 3.0 | >= 3 instances |
| Activity Retry | 3 retried | 5 retried | attempt >= 2 |
| Queue Wait Latency | 1s median | 5s median | Per task queue |
| Failure Hotspot | 3 failures | 5 failures | Per activity type |
| Long-Running Activity | 30min | 120min | Running status only |
| Error in I/O | 2 matches | 5 matches | 12 patterns; allowlist suppression |
| Child WF Failure | 2 failures | 5 failures | Per child type |
| Child WF Start Latency | 2s median | 10s median | Per child type |

All thresholds are compile-time constants in `InsightThresholds`.

---

## What Data We Have

The `TemporalClient` trait exposes the raw material for all algorithms:

```rust
pub trait TemporalClient: Send + Sync {
    async fn count(&self, query: Option<&str>) -> ClientResult<u64>;
    async fn list(&self, filter: &WorkflowFilter, limit: u32)
        -> ClientResult<Vec<WorkflowSummary>>;
    async fn describe(&self, workflow_id: &str, run_id: Option<&str>)
        -> ClientResult<WorkflowDetail>;
    async fn get_history(&self, workflow_id: &str, run_id: Option<&str>)
        -> ClientResult<Vec<HistoryEvent>>;
    async fn cancel(&self, workflow_id: &str, run_id: Option<&str>) -> ClientResult<()>;
    async fn terminate(&self, workflow_id: &str, run_id: Option<&str>,
        reason: &str) -> ClientResult<()>;
}
```

`list()` + `get_history()` are what the current algorithms use.
`describe()` and `count()` are available for future algorithms.

---

## Future: Signal Storm Detection

**Idea**: Detect workflows receiving excessive signals in short windows.

History events include `WorkflowExecutionSignaled`. Count signals per
workflow per time window. Flag workflows with signal rates that suggest
a producer bug or infinite signal loop.

**Available data**: `get_history()` returns signal events with timestamps.

**Thresholds (proposed)**: Warning at 50 signals/min, Critical at 200/min.

---

## Future: Decision Latency

**Idea**: Detect slow workflow task processing (decision latency).

History events include `WorkflowTaskScheduled` and `WorkflowTaskCompleted`.
The delta between these reveals how long the worker takes to process
a decision (evaluate workflow code).

**Available data**: `get_history()` returns both event types with timestamps.

**Why it matters**: High decision latency indicates heavy workflow logic,
large payloads being deserialized, or an overloaded worker.

---

## Future: Scheduling Overhead

**Idea**: Measure the gap between `ActivityTaskScheduled` and
`ActivityTaskStarted` at the event level (not just queue wait).

This is similar to Queue Wait Latency but operates at individual event
granularity rather than the correlated activity view. It captures
scheduling overhead that might be masked by aggregation.

**Available data**: Both event types exist in `get_history()` output.

**Why it matters**: Scheduling overhead spikes can indicate task queue
starvation even when median queue wait looks acceptable.

---

## Future: Dependency Chain Analysis

**Idea**: Trace parent > child > grandchild workflow chains and
compute cumulative latency across the entire chain.

Currently we detect child workflow latency per type. This algorithm
would follow the full chain: fetch parent history, identify child
workflows, fetch their histories, identify grandchildren, and so on.

**Available data**: `ChildWorkflowExecutionStarted` events contain
the child's workflow ID. `describe()` can fetch run metadata.

**Why it matters**: A 2s latency at each of 4 nesting levels
becomes 8s of overhead invisible to per-level analysis.

---

## Future: Cross-Run Regression

**Idea**: Compare metrics (failure rate, retry counts, latency)
across workflow versions or deployment windows.

Group workflows by time window or by a version search attribute.
Compute the same metrics for each group. Flag significant regressions.

**Available data**: `list()` with time-range filters. Search attributes
on workflows can carry version metadata if the user sets them.

**Why it matters**: "Did the last deploy make things worse?" is one of
the most common operator questions, and currently requires manual comparison.

---

## Deterministic Findings vs. Advisory Signals

The 11 algorithms above are **deterministic**. Same input, same output.
Thresholds are compile-time constants. No randomness, no model weights.

A second class of signal is possible: **AI-generated advisory signals**.
These are stochastic -- the same input may produce slightly different
output across runs. Different framing is needed.

| | Deterministic Findings | Advisory Signals |
|---|---|---|
| Source | Threshold comparisons | LLM / embedding inference |
| Reproducibility | Identical across runs | Approximately consistent |
| Confidence | Binary (fires or doesn't) | Scored (0.0 - 1.0) |
| Speed | Microseconds | Milliseconds (local) to seconds (API) |
| Display | Severity badge | Confidence score + caveat |

The key constraint: advisory signals **annotate** deterministic findings.
They don't replace them. The deterministic layer remains the system of record.

---

## Future: Error Message Clustering

**Idea**: Use embeddings to group semantically similar failure messages
that string matching misses.

`"connection refused"`, `"ECONNREFUSED"`, `"failed to connect to host"`,
and `"dial tcp: connect: connection refused"` are the same root cause.
The Failure Hotspot algorithm sees four different strings.

**Approach**: Embed failure messages with a small local model (e.g.
all-MiniLM-L6-v2, ~80MB, ~5ms/embedding). Cluster by cosine similarity.
Report clusters instead of raw message histograms.

**Constraint**: Must run locally, sub-second for a typical scan's
worth of messages. No API calls in the hot path.

**Output**: Enriches the Failure Hotspot finding's detail field:
`"5 failures, 3 distinct messages, 1 semantic cluster: connectivity"`

---

## Future: Root Cause Correlation

**Idea**: Given a set of ranked findings, generate a short hypothesis
about shared root causes.

Example input (3 findings from a scan):
- `SendEmail: avg 4.2 retry attempts` (Retry Storm)
- `'email-workers': 3.1s median queue wait` (Queue Latency)
- `SendEmail: 7 failures in 5 workflows` (Failure Hotspot)

Example output:
`"These findings likely share a root cause: email-workers task queue`
`is under-provisioned, causing queue backpressure, retries, and failures."`

**Approach**: Feed the finding titles + details as structured context to
a small, fast model. Constrain output to 2-3 sentences.

**Constraint**: Runs after the deterministic scan completes, as a
post-processing step. Latency budget: < 2s for the full correlation.
Displayed separately from findings, clearly marked as AI-generated.

---

## Future: Payload Anomaly Detection

**Idea**: Flag activity I/O that is structurally different from the
norm for that activity type.

If 29 out of 30 `ProcessPayment` outputs have `{"status": "ok", "amount": ...}`
but one has `{"status": "ok", "error_details": {...}}`, that's an anomaly
even though no error pattern fired.

**Approach**: Compute a structural fingerprint of each JSON payload
(key paths + value types). Flag payloads whose fingerprint diverges
from the majority. Could use simple set-difference on key paths,
or embeddings for more nuanced detection.

**Constraint**: Must handle the case where payloads are legitimately
diverse (e.g. polymorphic activity types). Confidence scoring is
essential -- structural outlier != problem.

**Output**: Advisory signal attached to the workflow:
`"Unusual output structure in ProcessPayment (confidence: 0.82)"`

---

## Design Principles

**Why compile-time constants?**
Thresholds that change require recompilation. This is intentional --
threshold tuning is an engineering decision, not a runtime knob.
Wrong thresholds create alert fatigue or missed signals.

**Why sampling (30 max)?**
Each history fetch is a gRPC call. Fetching all histories would be
O(n) API calls. Sampling caps cost at 30 while prioritizing the
workflows most likely to reveal problems.

**Why ranked output?**
Operators have limited attention. A flat list of findings requires
triage. Ranking by severity + impact does the triage automatically.

**Why the allowlist?**
Pattern matching on I/O is inherently noisy. Rather than removing
useful patterns, let operators suppress known false positives per deployment.
