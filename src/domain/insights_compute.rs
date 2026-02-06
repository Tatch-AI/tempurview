use crate::domain::{
    ActivityExecution, ActivityStatus, ChildWorkflowExecution, ChildWorkflowStatus,
    InsightCategory, InsightFinding, InsightSeverity, InsightThresholds, InsightsConfig,
    WorkflowStatus, WorkflowSummary,
};
use chrono::Utc;
use std::collections::HashMap;

/// Compute findings from workflow list data (no history needed).
/// Implements: High Failure Rate, Stuck Running Workflows, All-Failed Type.
pub fn compute_list_findings(workflows: &[WorkflowSummary]) -> Vec<InsightFinding> {
    let mut findings = Vec::new();
    let now = Utc::now();

    // Group workflows by type
    let mut by_type: HashMap<&str, Vec<&WorkflowSummary>> = HashMap::new();
    for wf in workflows {
        by_type.entry(wf.workflow_type.as_str()).or_default().push(wf);
    }

    let failed_statuses = [
        WorkflowStatus::Failed,
        WorkflowStatus::Terminated,
        WorkflowStatus::TimedOut,
    ];

    for (wf_type, wfs) in &by_type {
        let total = wfs.len();
        let failed_count = wfs
            .iter()
            .filter(|w| failed_statuses.contains(&w.status))
            .count();

        // Finding #3: All-Failed Type
        if total >= InsightThresholds::ALL_FAILED_MIN_WORKFLOWS && failed_count == total {
            let affected: Vec<String> = wfs.iter().map(|w| w.workflow_id.clone()).collect();
            findings.push(InsightFinding {
                severity: InsightSeverity::Critical,
                category: InsightCategory::TypeAnomaly,
                title: format!(
                    "{}: all {} executions failed/terminated/timed out",
                    wf_type, total
                ),
                detail: format!(
                    "Every execution of {} has ended in failure. This type appears to be completely broken.",
                    wf_type
                ),
                affected_entities: affected,
                computed_at: now,
            });
            continue; // Skip the rate-based finding — all-failed is more specific
        }

        // Finding #1: High Failure Rate by Type
        if total >= InsightThresholds::MIN_WORKFLOWS_FOR_RATE && failed_count > 0 {
            let rate = failed_count as f64 / total as f64;
            let severity = if rate >= InsightThresholds::FAILURE_RATE_CRITICAL {
                Some(InsightSeverity::Critical)
            } else if rate >= InsightThresholds::FAILURE_RATE_WARNING {
                Some(InsightSeverity::Warning)
            } else if rate >= InsightThresholds::FAILURE_RATE_INFO {
                Some(InsightSeverity::Info)
            } else {
                None
            };

            if let Some(severity) = severity {
                let affected: Vec<String> = wfs
                    .iter()
                    .filter(|w| failed_statuses.contains(&w.status))
                    .map(|w| w.workflow_id.clone())
                    .collect();
                findings.push(InsightFinding {
                    severity,
                    category: InsightCategory::FailureRate,
                    title: format!(
                        "{}: {}/{} failed ({:.0}%)",
                        wf_type,
                        failed_count,
                        total,
                        rate * 100.0
                    ),
                    detail: format!(
                        "Workflow type {} has a {:.0}% failure rate across {} executions.",
                        wf_type,
                        rate * 100.0,
                        total
                    ),
                    affected_entities: affected,
                    computed_at: now,
                });
            }
        }
    }

    // Finding #2: Stuck Running Workflows
    let mut stuck_by_type: HashMap<&str, Vec<(&WorkflowSummary, i64)>> = HashMap::new();
    for wf in workflows {
        if wf.status == WorkflowStatus::Running {
            let elapsed = now - wf.start_time;
            let elapsed_hours = elapsed.num_hours();
            if elapsed_hours >= InsightThresholds::STUCK_WARNING_HOURS {
                stuck_by_type
                    .entry(wf.workflow_type.as_str())
                    .or_default()
                    .push((wf, elapsed_hours));
            }
        }
    }

    for (wf_type, stuck_wfs) in &stuck_by_type {
        let max_hours = stuck_wfs.iter().map(|(_, h)| *h).max().unwrap_or(0);
        let severity = if max_hours >= InsightThresholds::STUCK_CRITICAL_HOURS {
            InsightSeverity::Critical
        } else {
            InsightSeverity::Warning
        };

        let affected: Vec<String> = stuck_wfs.iter().map(|(w, _)| w.workflow_id.clone()).collect();
        let count = stuck_wfs.len();

        findings.push(InsightFinding {
            severity,
            category: InsightCategory::StuckWorkflow,
            title: format!(
                "{} {} workflow{} running > {}h",
                count,
                wf_type,
                if count == 1 { "" } else { "s" },
                InsightThresholds::STUCK_WARNING_HOURS,
            ),
            detail: format!(
                "Longest running: {}h. These workflows may be stuck or require investigation.",
                max_hours
            ),
            affected_entities: affected,
            computed_at: now,
        });
    }

    findings
}

/// Compute findings from sampled activity data (requires history).
/// Implements: Retry Storm, Queue Latency, Activity Failure Hotspot, Long-Running Activity.
pub fn compute_activity_findings(
    samples: &[(String, Vec<ActivityExecution>)],
    config: &InsightsConfig,
) -> Vec<InsightFinding> {
    let mut findings = Vec::new();
    let now = Utc::now();

    // Collect all activities across all sampled workflows
    // Track (activity_type -> list of (workflow_id, activity))
    let mut by_activity_type: HashMap<&str, Vec<(&str, &ActivityExecution)>> = HashMap::new();
    // Track (task_queue -> list of queue_wait_ms)
    let mut queue_waits: HashMap<&str, Vec<i64>> = HashMap::new();

    for (wf_id, activities) in samples {
        for activity in activities {
            by_activity_type
                .entry(activity.activity_type.as_str())
                .or_default()
                .push((wf_id.as_str(), activity));

            if let (Some(ref tq), Some(ref qw)) = (&activity.task_queue, &activity.queue_wait) {
                queue_waits
                    .entry(tq.as_str())
                    .or_default()
                    .push(qw.num_milliseconds());
            }
        }
    }

    // Finding #4: Retry Storm
    for (activity_type, instances) in &by_activity_type {
        let attempts: Vec<i32> = instances.iter().map(|(_, a)| a.attempt).collect();
        if attempts.len() >= InsightThresholds::RETRY_MIN_INSTANCES {
            let avg_attempt = attempts.iter().map(|&a| a as f64).sum::<f64>() / attempts.len() as f64;
            let severity = if avg_attempt >= InsightThresholds::RETRY_CRITICAL_AVG {
                Some(InsightSeverity::Critical)
            } else if avg_attempt >= InsightThresholds::RETRY_WARNING_AVG {
                Some(InsightSeverity::Warning)
            } else {
                None
            };

            if let Some(severity) = severity {
                let max_attempt = attempts.iter().max().copied().unwrap_or(0);
                let affected_wfs: Vec<String> = instances
                    .iter()
                    .filter(|(_, a)| a.attempt >= 2)
                    .map(|(wf_id, _)| wf_id.to_string())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                let wf_count = affected_wfs.len();

                findings.push(InsightFinding {
                    severity,
                    category: InsightCategory::RetryStorm,
                    title: format!(
                        "{}: avg {:.1} attempts ({} workflows)",
                        activity_type, avg_attempt, wf_count
                    ),
                    detail: format!(
                        "Highest: {} attempts. High retry counts indicate transient failures or misconfigured timeouts.",
                        max_attempt
                    ),
                    affected_entities: affected_wfs,
                    computed_at: now,
                });
            }
        }
    }

    // Finding #5: Queue Wait Latency
    for (queue_name, waits) in &queue_waits {
        if waits.is_empty() {
            continue;
        }
        let mut sorted = waits.clone();
        sorted.sort();
        let median = sorted[sorted.len() / 2];

        let severity = if median >= InsightThresholds::QUEUE_LATENCY_CRITICAL_MS {
            Some(InsightSeverity::Critical)
        } else if median >= InsightThresholds::QUEUE_LATENCY_WARNING_MS {
            Some(InsightSeverity::Warning)
        } else {
            None
        };

        if let Some(severity) = severity {
            let median_secs = median as f64 / 1000.0;
            findings.push(InsightFinding {
                severity,
                category: InsightCategory::QueueLatency,
                title: format!("'{}': {:.1}s median queue wait", queue_name, median_secs),
                detail: format!(
                    "Measured across {} activities. High queue wait may indicate insufficient workers.",
                    sorted.len()
                ),
                affected_entities: vec![queue_name.to_string()],
                computed_at: now,
            });
        }
    }

    // Finding #6: Activity Failure Hotspot
    for (activity_type, instances) in &by_activity_type {
        let failures: Vec<(&str, &ActivityExecution)> = instances
            .iter()
            .filter(|(_, a)| a.status == ActivityStatus::Failed)
            .copied()
            .collect();

        let failure_count = failures.len();
        if failure_count == 0 {
            continue;
        }

        let severity = if failure_count >= InsightThresholds::ACTIVITY_FAILURE_CRITICAL {
            Some(InsightSeverity::Critical)
        } else if failure_count >= InsightThresholds::ACTIVITY_FAILURE_WARNING {
            Some(InsightSeverity::Warning)
        } else {
            None
        };

        if let Some(severity) = severity {
            // Find most common failure message
            let mut message_counts: HashMap<&str, usize> = HashMap::new();
            for (_, a) in &failures {
                if let Some(ref f) = a.failure {
                    *message_counts.entry(f.message.as_str()).or_default() += 1;
                }
            }
            let common_message = message_counts
                .iter()
                .max_by_key(|(_, &count)| count)
                .map(|(&msg, _)| msg);

            let affected_wfs: Vec<String> = failures
                .iter()
                .map(|(wf_id, _)| wf_id.to_string())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            let wf_count = affected_wfs.len();

            let detail = if let Some(msg) = common_message {
                format!(
                    "{} failures across {} workflows. Common error: {}",
                    failure_count, wf_count, msg
                )
            } else {
                format!(
                    "{} failures across {} workflows.",
                    failure_count, wf_count
                )
            };

            findings.push(InsightFinding {
                severity,
                category: InsightCategory::ActivityFailure,
                title: format!(
                    "{}: {} failures in {} workflows",
                    activity_type, failure_count, wf_count
                ),
                detail,
                affected_entities: affected_wfs,
                computed_at: now,
            });
        }
    }

    // Finding #8: Activity Retry (individual activities with attempt >= 2)
    // Groups by activity type, lists each retried instance
    let mut retried_by_type: HashMap<&str, Vec<(&str, i32)>> = HashMap::new(); // activity_type -> [(wf_id, attempt)]
    for (wf_id, activities) in samples {
        for activity in activities {
            if activity.attempt >= InsightThresholds::ACTIVITY_RETRY_MIN_ATTEMPT {
                retried_by_type
                    .entry(activity.activity_type.as_str())
                    .or_default()
                    .push((wf_id.as_str(), activity.attempt));
            }
        }
    }

    for (activity_type, retries) in &retried_by_type {
        let count = retries.len();
        let max_attempt = retries.iter().map(|(_, a)| *a).max().unwrap_or(0);
        let affected_wfs: Vec<String> = retries
            .iter()
            .map(|(wf_id, _)| wf_id.to_string())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let wf_count = affected_wfs.len();

        // Info for any retries, Warning for 3+, Critical for 5+
        let severity = if count >= 5 {
            InsightSeverity::Critical
        } else if count >= 3 {
            InsightSeverity::Warning
        } else {
            InsightSeverity::Info
        };

        findings.push(InsightFinding {
            severity,
            category: InsightCategory::ActivityRetry,
            title: format!(
                "{}: {} retried (max {} attempts, {} workflows)",
                activity_type, count, max_attempt, wf_count
            ),
            detail: format!(
                "Activities with attempt >= {} detected. Retries indicate transient failures, timeouts, or resource contention.",
                InsightThresholds::ACTIVITY_RETRY_MIN_ATTEMPT,
            ),
            affected_entities: affected_wfs,
            computed_at: now,
        });
    }

    // Finding #9: Error in I/O — scan activity input/output/failure for error patterns
    // This catches "swallowed" failures where workflows succeed but activity I/O contains error signals
    {
        let patterns = InsightThresholds::ERROR_PATTERNS;
        // (activity_type, wf_id, matched_pattern, snippet)
        let mut matches: Vec<(&str, &str, &str, String)> = Vec::new();

        for (wf_id, activities) in samples {
            for activity in activities {
                // Scan output
                if let Some(ref output) = activity.output {
                    let output_str = output.to_string().to_lowercase();
                    if !config.is_allowlisted(&output_str) {
                    for &pattern in patterns {
                        if output_str.contains(pattern) {
                            let snippet = extract_snippet(&output.to_string(), pattern);
                            matches.push((
                                activity.activity_type.as_str(),
                                wf_id.as_str(),
                                pattern,
                                format!("output: {}", snippet),
                            ));
                            break; // one match per activity output is enough
                        }
                    }
                    }
                }
                // Scan input
                if let Some(ref input) = activity.input {
                    let input_str = input.to_string().to_lowercase();
                    if !config.is_allowlisted(&input_str) {
                    for &pattern in patterns {
                        if input_str.contains(pattern) {
                            let snippet = extract_snippet(&input.to_string(), pattern);
                            matches.push((
                                activity.activity_type.as_str(),
                                wf_id.as_str(),
                                pattern,
                                format!("input: {}", snippet),
                            ));
                            break;
                        }
                    }
                    }
                }
                // Scan failure message (even on "completed" activities that had prior failures)
                if let Some(ref failure) = activity.failure {
                    let msg_lower = failure.message.to_lowercase();
                    if !config.is_allowlisted(&msg_lower) {
                    for &pattern in patterns {
                        if msg_lower.contains(pattern) {
                            matches.push((
                                activity.activity_type.as_str(),
                                wf_id.as_str(),
                                pattern,
                                format!("failure: {}", truncate(&failure.message, 80)),
                            ));
                            break;
                        }
                    }
                    }
                }
            }
        }

        if !matches.is_empty() {
            // Group by activity type
            let mut by_type: HashMap<&str, Vec<(&str, &str, String)>> = HashMap::new();
            for (at, wf_id, pattern, snippet) in &matches {
                by_type
                    .entry(at)
                    .or_default()
                    .push((wf_id, pattern, snippet.clone()));
            }

            for (activity_type, type_matches) in &by_type {
                let count = type_matches.len();
                let severity = if count >= InsightThresholds::ERROR_IN_OUTPUT_CRITICAL {
                    InsightSeverity::Critical
                } else if count >= InsightThresholds::ERROR_IN_OUTPUT_WARNING {
                    InsightSeverity::Warning
                } else {
                    InsightSeverity::Info
                };

                // Count pattern frequency
                let mut pattern_counts: HashMap<&str, usize> = HashMap::new();
                for (_, pattern, _) in type_matches {
                    *pattern_counts.entry(pattern).or_default() += 1;
                }
                let top_pattern = pattern_counts
                    .iter()
                    .max_by_key(|(_, &c)| c)
                    .map(|(&p, _)| p)
                    .unwrap_or("error");

                let affected_wfs: Vec<String> = type_matches
                    .iter()
                    .map(|(wf_id, _, _)| wf_id.to_string())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                let wf_count = affected_wfs.len();

                // Show a few example snippets in the detail
                let examples: Vec<String> = type_matches
                    .iter()
                    .take(3)
                    .map(|(_, _, snippet)| snippet.clone())
                    .collect();

                findings.push(InsightFinding {
                    severity,
                    category: InsightCategory::ErrorInOutput,
                    title: format!(
                        "{}: \"{}\" found in {} I/O ({} workflows)",
                        activity_type, top_pattern, count, wf_count,
                    ),
                    detail: format!(
                        "Error patterns detected in activity I/O despite workflow success. Examples: {}",
                        examples.join("; "),
                    ),
                    affected_entities: affected_wfs,
                    computed_at: now,
                });
            }
        }
    }

    // Finding #7: Long-Running Activity
    let mut long_running: Vec<(&str, &str, i64)> = Vec::new(); // (wf_id, activity_type, elapsed_mins)
    for (wf_id, activities) in samples {
        for activity in activities {
            if activity.status == ActivityStatus::Running {
                if let Some(ref started) = activity.started_time {
                    let elapsed_mins = (now - *started).num_minutes();
                    if elapsed_mins >= InsightThresholds::LONG_ACTIVITY_WARNING_MINS {
                        long_running.push((wf_id.as_str(), activity.activity_type.as_str(), elapsed_mins));
                    }
                }
            }
        }
    }

    if !long_running.is_empty() {
        let max_mins = long_running.iter().map(|(_, _, m)| *m).max().unwrap_or(0);
        let severity = if max_mins >= InsightThresholds::LONG_ACTIVITY_CRITICAL_MINS {
            InsightSeverity::Critical
        } else {
            InsightSeverity::Warning
        };

        let affected: Vec<String> = long_running
            .iter()
            .map(|(wf_id, _, _)| wf_id.to_string())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Group by activity type for the title
        let mut by_type: HashMap<&str, usize> = HashMap::new();
        for (_, at, _) in &long_running {
            *by_type.entry(at).or_default() += 1;
        }
        let type_summary: Vec<String> = by_type
            .iter()
            .map(|(t, c)| format!("{}({})", t, c))
            .collect();

        findings.push(InsightFinding {
            severity,
            category: InsightCategory::LongRunningActivity,
            title: format!(
                "{} activities running > {}min: {}",
                long_running.len(),
                InsightThresholds::LONG_ACTIVITY_WARNING_MINS,
                type_summary.join(", "),
            ),
            detail: format!(
                "Longest: {}min. Long-running activities may indicate stuck workers or infinite loops.",
                max_mins
            ),
            affected_entities: affected,
            computed_at: now,
        });
    }

    findings
}

/// Compute findings from sampled child workflow data (requires history).
/// Implements: Child Workflow Failure Hotspot, Child Workflow Start Latency.
pub fn compute_child_workflow_findings(
    samples: &[(String, Vec<ChildWorkflowExecution>)],
) -> Vec<InsightFinding> {
    let mut findings = Vec::new();
    let now = Utc::now();

    // Track (child_workflow_type -> list of (parent_workflow_id, child_wf))
    let mut by_child_type: HashMap<&str, Vec<(&str, &ChildWorkflowExecution)>> = HashMap::new();

    for (wf_id, child_workflows) in samples {
        for cw in child_workflows {
            by_child_type
                .entry(cw.workflow_type.as_str())
                .or_default()
                .push((wf_id.as_str(), cw));
        }
    }

    let failed_statuses = [
        ChildWorkflowStatus::Failed,
        ChildWorkflowStatus::TimedOut,
        ChildWorkflowStatus::Canceled,
        ChildWorkflowStatus::Terminated,
        ChildWorkflowStatus::StartFailed,
    ];

    for (child_type, instances) in &by_child_type {
        // Finding: Child Workflow Failure Hotspot
        let failures: Vec<(&str, &ChildWorkflowExecution)> = instances
            .iter()
            .filter(|(_, cw)| failed_statuses.contains(&cw.status))
            .copied()
            .collect();

        let failure_count = failures.len();
        if failure_count > 0 {
            let severity = if failure_count >= InsightThresholds::CHILD_WF_FAILURE_CRITICAL {
                Some(InsightSeverity::Critical)
            } else if failure_count >= InsightThresholds::CHILD_WF_FAILURE_WARNING {
                Some(InsightSeverity::Warning)
            } else {
                None
            };

            if let Some(severity) = severity {
                let affected_wfs: Vec<String> = failures
                    .iter()
                    .map(|(wf_id, _)| wf_id.to_string())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                let wf_count = affected_wfs.len();

                findings.push(InsightFinding {
                    severity,
                    category: InsightCategory::ChildWorkflowFailure,
                    title: format!(
                        "{}: {} failures across {} parent workflows",
                        child_type, failure_count, wf_count
                    ),
                    detail: format!(
                        "{} child workflow failures detected for type {}.",
                        failure_count, child_type
                    ),
                    affected_entities: affected_wfs,
                    computed_at: now,
                });
            }
        }

        // Finding: Child Workflow Start Latency
        let mut latencies_ms: Vec<(&str, i64)> = Vec::new();
        for (wf_id, cw) in instances {
            if let Some(ref sl) = cw.start_latency {
                latencies_ms.push((wf_id, sl.num_milliseconds()));
            }
        }

        if !latencies_ms.is_empty() {
            let mut sorted: Vec<i64> = latencies_ms.iter().map(|(_, ms)| *ms).collect();
            sorted.sort();
            let median = sorted[sorted.len() / 2];

            let severity = if median >= InsightThresholds::CHILD_WF_LATENCY_CRITICAL_MS {
                Some(InsightSeverity::Critical)
            } else if median >= InsightThresholds::CHILD_WF_LATENCY_WARNING_MS {
                Some(InsightSeverity::Warning)
            } else {
                None
            };

            if let Some(severity) = severity {
                let median_secs = median as f64 / 1000.0;
                let affected_wfs: Vec<String> = latencies_ms
                    .iter()
                    .filter(|(_, ms)| *ms >= InsightThresholds::CHILD_WF_LATENCY_WARNING_MS)
                    .map(|(wf_id, _)| wf_id.to_string())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();

                findings.push(InsightFinding {
                    severity,
                    category: InsightCategory::ChildWorkflowLatency,
                    title: format!(
                        "{}: {:.1}s median start latency",
                        child_type, median_secs
                    ),
                    detail: format!(
                        "Measured across {} child workflow executions. High start latency may indicate task queue congestion.",
                        sorted.len()
                    ),
                    affected_entities: affected_wfs,
                    computed_at: now,
                });
            }
        }
    }

    findings
}

/// Extract a short snippet around a pattern match in a string
fn extract_snippet(text: &str, pattern: &str) -> String {
    let lower = text.to_lowercase();
    if let Some(pos) = lower.find(pattern) {
        let start = pos.saturating_sub(20);
        let end = (pos + pattern.len() + 40).min(text.len());
        let snippet = &text[start..end];
        let snippet = snippet.replace('\n', " ");
        if start > 0 || end < text.len() {
            format!("...{}...", snippet.trim())
        } else {
            snippet.trim().to_string()
        }
    } else {
        truncate(text, 60)
    }
}

/// Truncate a string to max_len characters
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

/// Select workflows for history sampling. Prioritizes: failed → running → recent completed.
/// Returns up to `max` workflow references.
pub fn select_workflows_for_sampling(
    workflows: &[WorkflowSummary],
    max: usize,
) -> Vec<&WorkflowSummary> {
    let mut selected: Vec<&WorkflowSummary> = Vec::with_capacity(max);
    let mut seen = std::collections::HashSet::new();

    // Priority 1: Failed/Terminated/TimedOut
    let failed_statuses = [
        WorkflowStatus::Failed,
        WorkflowStatus::Terminated,
        WorkflowStatus::TimedOut,
    ];
    for wf in workflows {
        if selected.len() >= max {
            break;
        }
        if failed_statuses.contains(&wf.status) && seen.insert(&wf.workflow_id) {
            selected.push(wf);
        }
    }

    // Priority 2: Running (especially long-running)
    let mut running: Vec<&WorkflowSummary> = workflows
        .iter()
        .filter(|w| w.status == WorkflowStatus::Running && !seen.contains(&w.workflow_id))
        .collect();
    running.sort_by_key(|w| w.start_time); // oldest first (most likely stuck)
    for wf in running {
        if selected.len() >= max {
            break;
        }
        if seen.insert(&wf.workflow_id) {
            selected.push(wf);
        }
    }

    // Priority 3: Recent completed
    for wf in workflows {
        if selected.len() >= max {
            break;
        }
        if wf.status == WorkflowStatus::Completed && seen.insert(&wf.workflow_id) {
            selected.push(wf);
        }
    }

    selected
}

/// Rank findings: Critical first, then Warning, then Info.
/// Within same severity, sort by affected entity count descending.
pub fn rank_findings(mut findings: Vec<InsightFinding>) -> Vec<InsightFinding> {
    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| b.affected_entities.len().cmp(&a.affected_entities.len()))
    });
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ActivityStatus, ChildWorkflowExecution, ChildWorkflowStatus, FailureInfo, InsightsConfig};
    use chrono::{Duration, TimeDelta};

    fn make_wf(id: &str, wf_type: &str, status: WorkflowStatus, hours_ago: i64) -> WorkflowSummary {
        let start_time = Utc::now() - Duration::hours(hours_ago);
        let close_time = if status != WorkflowStatus::Running {
            Some(start_time + Duration::minutes(5))
        } else {
            None
        };
        WorkflowSummary {
            workflow_id: id.to_string(),
            run_id: format!("run-{}", id),
            workflow_type: wf_type.to_string(),
            status,
            start_time,
            close_time,
            task_queue: "default".to_string(),
        }
    }

    fn make_activity(
        activity_type: &str,
        status: ActivityStatus,
        attempt: i32,
        queue_wait_ms: Option<i64>,
        started_mins_ago: Option<i64>,
    ) -> ActivityExecution {
        let now = Utc::now();
        let scheduled_time = now - Duration::minutes(60);
        let started_time = started_mins_ago.map(|m| now - Duration::minutes(m));
        let closed_time = if matches!(status, ActivityStatus::Completed | ActivityStatus::Failed | ActivityStatus::TimedOut | ActivityStatus::Canceled) {
            Some(now - Duration::minutes(1))
        } else {
            None
        };

        ActivityExecution {
            activity_id: "1".to_string(),
            activity_type: activity_type.to_string(),
            status,
            task_queue: Some("default".to_string()),
            scheduled_time,
            started_time,
            closed_time,
            queue_wait: queue_wait_ms.map(TimeDelta::milliseconds),
            execution_time: Some(TimeDelta::seconds(1)),
            total_time: Some(TimeDelta::seconds(2)),
            attempt,
            input: None,
            output: None,
            failure: if status == ActivityStatus::Failed {
                Some(FailureInfo {
                    message: format!("{} failed: test error", activity_type),
                    failure_type: "ApplicationFailure".to_string(),
                    stack_trace: None,
                    cause: None,
                })
            } else {
                None
            },
            scheduled_event_id: 5,
            started_event_id: started_time.map(|_| 6),
            closed_event_id: closed_time.map(|_| 7),
        }
    }

    // --- compute_list_findings tests ---

    #[test]
    fn test_list_findings_empty() {
        let findings = compute_list_findings(&[]);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_high_failure_rate_critical() {
        let workflows = vec![
            make_wf("wf-1", "Payment", WorkflowStatus::Failed, 1),
            make_wf("wf-2", "Payment", WorkflowStatus::Failed, 2),
            make_wf("wf-3", "Payment", WorkflowStatus::Failed, 3),
            make_wf("wf-4", "Payment", WorkflowStatus::Completed, 4),
        ];

        let findings = compute_list_findings(&workflows);
        let failure_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::FailureRate)
            .collect();

        assert_eq!(failure_findings.len(), 1);
        assert_eq!(failure_findings[0].severity, InsightSeverity::Critical);
        assert!(failure_findings[0].title.contains("Payment"));
        assert!(failure_findings[0].title.contains("75%"));
    }

    #[test]
    fn test_high_failure_rate_warning() {
        let workflows = vec![
            make_wf("wf-1", "Email", WorkflowStatus::Failed, 1),
            make_wf("wf-2", "Email", WorkflowStatus::Completed, 2),
            make_wf("wf-3", "Email", WorkflowStatus::Completed, 3),
            make_wf("wf-4", "Email", WorkflowStatus::Completed, 4),
        ];

        let findings = compute_list_findings(&workflows);
        let failure_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::FailureRate)
            .collect();

        assert_eq!(failure_findings.len(), 1);
        assert_eq!(failure_findings[0].severity, InsightSeverity::Warning);
    }

    #[test]
    fn test_no_failure_rate_below_threshold() {
        let workflows = vec![
            make_wf("wf-1", "Email", WorkflowStatus::Completed, 1),
            make_wf("wf-2", "Email", WorkflowStatus::Completed, 2),
            make_wf("wf-3", "Email", WorkflowStatus::Completed, 3),
        ];

        let findings = compute_list_findings(&workflows);
        let failure_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::FailureRate)
            .collect();

        assert!(failure_findings.is_empty());
    }

    #[test]
    fn test_all_failed_type() {
        let workflows = vec![
            make_wf("wf-1", "BrokenType", WorkflowStatus::Failed, 1),
            make_wf("wf-2", "BrokenType", WorkflowStatus::Terminated, 2),
            make_wf("wf-3", "BrokenType", WorkflowStatus::TimedOut, 3),
        ];

        let findings = compute_list_findings(&workflows);
        let anomaly_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::TypeAnomaly)
            .collect();

        assert_eq!(anomaly_findings.len(), 1);
        assert_eq!(anomaly_findings[0].severity, InsightSeverity::Critical);
        assert!(anomaly_findings[0].title.contains("BrokenType"));

        // Should NOT also generate a failure rate finding for this type
        let failure_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::FailureRate && f.title.contains("BrokenType"))
            .collect();
        assert!(failure_findings.is_empty());
    }

    #[test]
    fn test_stuck_workflows_warning() {
        let workflows = vec![
            make_wf("wf-1", "DataProc", WorkflowStatus::Running, 3), // 3h > 2h threshold
            make_wf("wf-2", "DataProc", WorkflowStatus::Completed, 1),
        ];

        let findings = compute_list_findings(&workflows);
        let stuck_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::StuckWorkflow)
            .collect();

        assert_eq!(stuck_findings.len(), 1);
        assert_eq!(stuck_findings[0].severity, InsightSeverity::Warning);
    }

    #[test]
    fn test_stuck_workflows_critical() {
        let workflows = vec![
            make_wf("wf-1", "DataProc", WorkflowStatus::Running, 7), // 7h > 6h threshold
        ];

        let findings = compute_list_findings(&workflows);
        let stuck_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::StuckWorkflow)
            .collect();

        assert_eq!(stuck_findings.len(), 1);
        assert_eq!(stuck_findings[0].severity, InsightSeverity::Critical);
    }

    #[test]
    fn test_no_stuck_workflows_under_threshold() {
        let workflows = vec![
            make_wf("wf-1", "DataProc", WorkflowStatus::Running, 1), // 1h < 2h threshold
        ];

        let findings = compute_list_findings(&workflows);
        let stuck_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::StuckWorkflow)
            .collect();

        assert!(stuck_findings.is_empty());
    }

    // --- compute_activity_findings tests ---

    #[test]
    fn test_activity_findings_empty() {
        let findings = compute_activity_findings(&[], &InsightsConfig::default());
        assert!(findings.is_empty());
    }

    #[test]
    fn test_retry_storm_warning() {
        let samples = vec![
            ("wf-1".to_string(), vec![make_activity("SendEmail", ActivityStatus::Failed, 2, Some(50), Some(5))]),
            ("wf-2".to_string(), vec![make_activity("SendEmail", ActivityStatus::Failed, 3, Some(50), Some(5))]),
            ("wf-3".to_string(), vec![make_activity("SendEmail", ActivityStatus::Failed, 2, Some(50), Some(5))]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let retry_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::RetryStorm)
            .collect();

        assert_eq!(retry_findings.len(), 1);
        // avg = (2+3+2)/3 = 2.33 → Warning (>= 2.0)
        assert_eq!(retry_findings[0].severity, InsightSeverity::Warning);
        assert!(retry_findings[0].title.contains("SendEmail"));
    }

    #[test]
    fn test_retry_storm_critical() {
        let samples = vec![
            ("wf-1".to_string(), vec![make_activity("SendEmail", ActivityStatus::Failed, 3, Some(50), Some(5))]),
            ("wf-2".to_string(), vec![make_activity("SendEmail", ActivityStatus::Failed, 4, Some(50), Some(5))]),
            ("wf-3".to_string(), vec![make_activity("SendEmail", ActivityStatus::Failed, 5, Some(50), Some(5))]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let retry_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::RetryStorm)
            .collect();

        assert_eq!(retry_findings.len(), 1);
        // avg = (3+4+5)/3 = 4.0 → Critical (>= 3.0)
        assert_eq!(retry_findings[0].severity, InsightSeverity::Critical);
    }

    #[test]
    fn test_no_retry_storm_below_threshold() {
        let samples = vec![
            ("wf-1".to_string(), vec![make_activity("SendEmail", ActivityStatus::Completed, 1, Some(50), Some(5))]),
            ("wf-2".to_string(), vec![make_activity("SendEmail", ActivityStatus::Completed, 1, Some(50), Some(5))]),
            ("wf-3".to_string(), vec![make_activity("SendEmail", ActivityStatus::Completed, 1, Some(50), Some(5))]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let retry_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::RetryStorm)
            .collect();

        assert!(retry_findings.is_empty());
    }

    #[test]
    fn test_queue_latency_warning() {
        let samples = vec![
            ("wf-1".to_string(), vec![make_activity("A", ActivityStatus::Completed, 1, Some(1500), Some(5))]),
            ("wf-2".to_string(), vec![make_activity("A", ActivityStatus::Completed, 1, Some(1200), Some(5))]),
            ("wf-3".to_string(), vec![make_activity("A", ActivityStatus::Completed, 1, Some(1800), Some(5))]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let latency_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::QueueLatency)
            .collect();

        assert_eq!(latency_findings.len(), 1);
        assert_eq!(latency_findings[0].severity, InsightSeverity::Warning);
    }

    #[test]
    fn test_queue_latency_critical() {
        let samples = vec![
            ("wf-1".to_string(), vec![make_activity("A", ActivityStatus::Completed, 1, Some(6000), Some(5))]),
            ("wf-2".to_string(), vec![make_activity("A", ActivityStatus::Completed, 1, Some(7000), Some(5))]),
            ("wf-3".to_string(), vec![make_activity("A", ActivityStatus::Completed, 1, Some(5500), Some(5))]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let latency_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::QueueLatency)
            .collect();

        assert_eq!(latency_findings.len(), 1);
        assert_eq!(latency_findings[0].severity, InsightSeverity::Critical);
    }

    #[test]
    fn test_activity_failure_hotspot() {
        let samples = vec![
            ("wf-1".to_string(), vec![make_activity("ValidateInput", ActivityStatus::Failed, 1, Some(50), Some(5))]),
            ("wf-2".to_string(), vec![make_activity("ValidateInput", ActivityStatus::Failed, 1, Some(50), Some(5))]),
            ("wf-3".to_string(), vec![make_activity("ValidateInput", ActivityStatus::Failed, 1, Some(50), Some(5))]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let failure_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ActivityFailure)
            .collect();

        assert_eq!(failure_findings.len(), 1);
        assert_eq!(failure_findings[0].severity, InsightSeverity::Warning);
        assert!(failure_findings[0].title.contains("ValidateInput"));
    }

    #[test]
    fn test_activity_failure_hotspot_critical() {
        let samples: Vec<(String, Vec<ActivityExecution>)> = (0..5)
            .map(|i| {
                (
                    format!("wf-{}", i),
                    vec![make_activity("ValidateInput", ActivityStatus::Failed, 1, Some(50), Some(5))],
                )
            })
            .collect();

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let failure_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ActivityFailure)
            .collect();

        assert_eq!(failure_findings.len(), 1);
        assert_eq!(failure_findings[0].severity, InsightSeverity::Critical);
    }

    #[test]
    fn test_long_running_activity() {
        // Activity running for 45 minutes (> 30min threshold)
        let samples = vec![(
            "wf-1".to_string(),
            vec![make_activity("ProcessPayment", ActivityStatus::Running, 1, Some(50), Some(45))],
        )];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let long_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::LongRunningActivity)
            .collect();

        assert_eq!(long_findings.len(), 1);
        assert_eq!(long_findings[0].severity, InsightSeverity::Warning);
    }

    #[test]
    fn test_long_running_activity_critical() {
        // Activity running for 150 minutes (> 120min threshold)
        let samples = vec![(
            "wf-1".to_string(),
            vec![make_activity("ProcessPayment", ActivityStatus::Running, 1, Some(50), Some(150))],
        )];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let long_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::LongRunningActivity)
            .collect();

        assert_eq!(long_findings.len(), 1);
        assert_eq!(long_findings[0].severity, InsightSeverity::Critical);
    }

    // --- select_workflows_for_sampling tests ---

    #[test]
    fn test_sampling_prioritizes_failed() {
        let workflows = vec![
            make_wf("wf-ok-1", "A", WorkflowStatus::Completed, 1),
            make_wf("wf-fail-1", "A", WorkflowStatus::Failed, 2),
            make_wf("wf-ok-2", "A", WorkflowStatus::Completed, 3),
            make_wf("wf-fail-2", "A", WorkflowStatus::Terminated, 4),
        ];

        let selected = select_workflows_for_sampling(&workflows, 2);
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().all(|w| {
            w.status == WorkflowStatus::Failed || w.status == WorkflowStatus::Terminated
        }));
    }

    #[test]
    fn test_sampling_then_running() {
        let workflows = vec![
            make_wf("wf-fail-1", "A", WorkflowStatus::Failed, 1),
            make_wf("wf-run-1", "A", WorkflowStatus::Running, 5),
            make_wf("wf-ok-1", "A", WorkflowStatus::Completed, 2),
        ];

        let selected = select_workflows_for_sampling(&workflows, 3);
        assert_eq!(selected.len(), 3);
        // First: failed, then running, then completed
        assert_eq!(selected[0].workflow_id, "wf-fail-1");
        assert_eq!(selected[1].workflow_id, "wf-run-1");
        assert_eq!(selected[2].workflow_id, "wf-ok-1");
    }

    #[test]
    fn test_sampling_caps_at_max() {
        let workflows: Vec<_> = (0..50)
            .map(|i| make_wf(&format!("wf-{}", i), "A", WorkflowStatus::Failed, 1))
            .collect();

        let selected = select_workflows_for_sampling(&workflows, 5);
        assert_eq!(selected.len(), 5);
    }

    #[test]
    fn test_sampling_empty() {
        let selected = select_workflows_for_sampling(&[], 20);
        assert!(selected.is_empty());
    }

    // --- rank_findings tests ---

    #[test]
    fn test_rank_by_severity() {
        let findings = vec![
            InsightFinding {
                severity: InsightSeverity::Info,
                category: InsightCategory::FailureRate,
                title: "info".to_string(),
                detail: String::new(),
                affected_entities: vec!["a".to_string()],
                computed_at: Utc::now(),
            },
            InsightFinding {
                severity: InsightSeverity::Critical,
                category: InsightCategory::FailureRate,
                title: "critical".to_string(),
                detail: String::new(),
                affected_entities: vec!["a".to_string()],
                computed_at: Utc::now(),
            },
            InsightFinding {
                severity: InsightSeverity::Warning,
                category: InsightCategory::FailureRate,
                title: "warning".to_string(),
                detail: String::new(),
                affected_entities: vec!["a".to_string()],
                computed_at: Utc::now(),
            },
        ];

        let ranked = rank_findings(findings);
        assert_eq!(ranked[0].severity, InsightSeverity::Critical);
        assert_eq!(ranked[1].severity, InsightSeverity::Warning);
        assert_eq!(ranked[2].severity, InsightSeverity::Info);
    }

    // --- Activity Retry tests ---

    #[test]
    fn test_activity_retry_detected() {
        let samples = vec![
            ("wf-1".to_string(), vec![make_activity("SendEmail", ActivityStatus::Completed, 3, Some(50), Some(5))]),
            ("wf-2".to_string(), vec![make_activity("SendEmail", ActivityStatus::Completed, 2, Some(50), Some(5))]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let retry_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ActivityRetry)
            .collect();

        assert_eq!(retry_findings.len(), 1);
        assert!(retry_findings[0].title.contains("SendEmail"));
        assert!(retry_findings[0].title.contains("2 retried")); // 2 activities with attempt >= 2
    }

    #[test]
    fn test_activity_retry_not_triggered_for_attempt_1() {
        let samples = vec![
            ("wf-1".to_string(), vec![make_activity("SendEmail", ActivityStatus::Completed, 1, Some(50), Some(5))]),
            ("wf-2".to_string(), vec![make_activity("SendEmail", ActivityStatus::Completed, 1, Some(50), Some(5))]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let retry_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ActivityRetry)
            .collect();

        assert!(retry_findings.is_empty());
    }

    // --- Error in I/O tests ---

    #[test]
    fn test_error_in_output_detected() {
        let mut a1 = make_activity("ProcessPayment", ActivityStatus::Completed, 1, Some(50), Some(5));
        a1.output = Some(serde_json::json!({"status": "ok", "message": "Handled gracefully but encountered an Error during processing"}));
        let mut a2 = make_activity("ProcessPayment", ActivityStatus::Completed, 1, Some(50), Some(5));
        a2.output = Some(serde_json::json!({"result": "partial", "exception_count": 3, "details": "caught Exception in handler"}));

        let samples = vec![
            ("wf-1".to_string(), vec![a1]),
            ("wf-2".to_string(), vec![a2]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let error_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ErrorInOutput)
            .collect();

        assert_eq!(error_findings.len(), 1);
        assert!(error_findings[0].title.contains("ProcessPayment"));
        assert!(error_findings[0].detail.contains("Error patterns detected"));
    }

    #[test]
    fn test_error_in_output_not_triggered_for_clean_output() {
        let mut a1 = make_activity("ProcessPayment", ActivityStatus::Completed, 1, Some(50), Some(5));
        a1.output = Some(serde_json::json!({"status": "success", "amount": 42.0}));

        let samples = vec![
            ("wf-1".to_string(), vec![a1]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let error_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ErrorInOutput)
            .collect();

        assert!(error_findings.is_empty());
    }

    #[test]
    fn test_error_in_input_detected() {
        let mut a1 = make_activity("ValidateInput", ActivityStatus::Completed, 1, Some(50), Some(5));
        a1.input = Some(serde_json::json!({"data": "retry after timeout error from upstream"}));

        let mut a2 = make_activity("ValidateInput", ActivityStatus::Completed, 1, Some(50), Some(5));
        a2.input = Some(serde_json::json!({"data": "propagated TIMEOUT from service X"}));

        let samples = vec![
            ("wf-1".to_string(), vec![a1]),
            ("wf-2".to_string(), vec![a2]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let error_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ErrorInOutput)
            .collect();

        assert_eq!(error_findings.len(), 1);
        assert!(error_findings[0].title.contains("ValidateInput"));
    }

    #[test]
    fn test_error_in_output_suppressed_by_allowlist() {
        let mut a1 = make_activity("ProcessClaim", ActivityStatus::Completed, 1, Some(50), Some(5));
        a1.output = Some(serde_json::json!({"policy_type": "errors and omissions policy"}));
        let mut a2 = make_activity("ProcessClaim", ActivityStatus::Completed, 1, Some(50), Some(5));
        a2.output = Some(serde_json::json!({"policy_type": "errors and omissions coverage"}));

        let samples = vec![
            ("wf-1".to_string(), vec![a1]),
            ("wf-2".to_string(), vec![a2]),
        ];

        let config = InsightsConfig {
            allowlist: vec!["errors and omissions".to_string()],
        };

        let findings = compute_activity_findings(&samples, &config);
        let error_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ErrorInOutput)
            .collect();

        assert!(error_findings.is_empty(), "Allowlisted phrase should suppress ErrorInOutput finding");
    }

    #[test]
    fn test_error_in_output_not_suppressed_without_allowlist() {
        let mut a1 = make_activity("ProcessClaim", ActivityStatus::Completed, 1, Some(50), Some(5));
        a1.output = Some(serde_json::json!({"policy_type": "errors and omissions policy"}));
        let mut a2 = make_activity("ProcessClaim", ActivityStatus::Completed, 1, Some(50), Some(5));
        a2.output = Some(serde_json::json!({"policy_type": "errors and omissions coverage"}));

        let samples = vec![
            ("wf-1".to_string(), vec![a1]),
            ("wf-2".to_string(), vec![a2]),
        ];

        let findings = compute_activity_findings(&samples, &InsightsConfig::default());
        let error_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ErrorInOutput)
            .collect();

        assert_eq!(error_findings.len(), 1, "Without allowlist, ErrorInOutput should be produced");
    }

    #[test]
    fn test_rank_tiebreak_by_affected_count() {
        let findings = vec![
            InsightFinding {
                severity: InsightSeverity::Warning,
                category: InsightCategory::FailureRate,
                title: "fewer".to_string(),
                detail: String::new(),
                affected_entities: vec!["a".to_string()],
                computed_at: Utc::now(),
            },
            InsightFinding {
                severity: InsightSeverity::Warning,
                category: InsightCategory::RetryStorm,
                title: "more".to_string(),
                detail: String::new(),
                affected_entities: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                computed_at: Utc::now(),
            },
        ];

        let ranked = rank_findings(findings);
        assert_eq!(ranked[0].title, "more");
        assert_eq!(ranked[1].title, "fewer");
    }

    // --- compute_child_workflow_findings tests ---

    fn make_child_wf(
        workflow_type: &str,
        status: ChildWorkflowStatus,
        start_latency_ms: Option<i64>,
    ) -> ChildWorkflowExecution {
        let now = Utc::now();
        let initiated_time = now - Duration::minutes(10);
        let started_time = start_latency_ms.map(|ms| initiated_time + Duration::milliseconds(ms));
        let closed_time = if matches!(
            status,
            ChildWorkflowStatus::Completed
                | ChildWorkflowStatus::Failed
                | ChildWorkflowStatus::TimedOut
                | ChildWorkflowStatus::Canceled
                | ChildWorkflowStatus::Terminated
                | ChildWorkflowStatus::StartFailed
        ) {
            Some(now - Duration::minutes(1))
        } else {
            None
        };

        ChildWorkflowExecution {
            workflow_id: format!("child-{}", workflow_type),
            workflow_type: workflow_type.to_string(),
            status,
            namespace: Some("default".to_string()),
            run_id: Some("run-123".to_string()),
            initiated_time,
            started_time,
            closed_time,
            start_latency: start_latency_ms.map(TimeDelta::milliseconds),
            execution_time: Some(TimeDelta::seconds(5)),
            total_time: Some(TimeDelta::seconds(10)),
            failure: if matches!(
                status,
                ChildWorkflowStatus::Failed | ChildWorkflowStatus::StartFailed
            ) {
                Some(FailureInfo {
                    message: format!("{} child workflow failed", workflow_type),
                    failure_type: "ApplicationFailure".to_string(),
                    stack_trace: None,
                    cause: None,
                })
            } else {
                None
            },
            initiated_event_id: 10,
            started_event_id: started_time.map(|_| 11),
            closed_event_id: closed_time.map(|_| 15),
        }
    }

    #[test]
    fn test_child_wf_findings_empty() {
        let findings = compute_child_workflow_findings(&[]);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_child_wf_failure_hotspot_warning() {
        let samples = vec![
            (
                "parent-1".to_string(),
                vec![make_child_wf("ProcessOrder", ChildWorkflowStatus::Failed, Some(50))],
            ),
            (
                "parent-2".to_string(),
                vec![make_child_wf("ProcessOrder", ChildWorkflowStatus::Failed, Some(50))],
            ),
        ];

        let findings = compute_child_workflow_findings(&samples);
        let failure_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ChildWorkflowFailure)
            .collect();

        assert_eq!(failure_findings.len(), 1);
        assert_eq!(failure_findings[0].severity, InsightSeverity::Warning);
        assert!(failure_findings[0].title.contains("ProcessOrder"));
        assert!(failure_findings[0].title.contains("2 failures"));
    }

    #[test]
    fn test_child_wf_failure_hotspot_critical() {
        let samples: Vec<(String, Vec<ChildWorkflowExecution>)> = (0..5)
            .map(|i| {
                (
                    format!("parent-{}", i),
                    vec![make_child_wf("ProcessOrder", ChildWorkflowStatus::Failed, Some(50))],
                )
            })
            .collect();

        let findings = compute_child_workflow_findings(&samples);
        let failure_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ChildWorkflowFailure)
            .collect();

        assert_eq!(failure_findings.len(), 1);
        assert_eq!(failure_findings[0].severity, InsightSeverity::Critical);
    }

    #[test]
    fn test_child_wf_latency_warning() {
        let samples = vec![
            (
                "parent-1".to_string(),
                vec![make_child_wf("ProcessOrder", ChildWorkflowStatus::Completed, Some(2500))],
            ),
            (
                "parent-2".to_string(),
                vec![make_child_wf("ProcessOrder", ChildWorkflowStatus::Completed, Some(3000))],
            ),
            (
                "parent-3".to_string(),
                vec![make_child_wf("ProcessOrder", ChildWorkflowStatus::Completed, Some(2800))],
            ),
        ];

        let findings = compute_child_workflow_findings(&samples);
        let latency_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ChildWorkflowLatency)
            .collect();

        assert_eq!(latency_findings.len(), 1);
        assert_eq!(latency_findings[0].severity, InsightSeverity::Warning);
        assert!(latency_findings[0].title.contains("ProcessOrder"));
    }

    #[test]
    fn test_child_wf_latency_critical() {
        let samples = vec![
            (
                "parent-1".to_string(),
                vec![make_child_wf("ProcessOrder", ChildWorkflowStatus::Completed, Some(11000))],
            ),
            (
                "parent-2".to_string(),
                vec![make_child_wf("ProcessOrder", ChildWorkflowStatus::Completed, Some(12000))],
            ),
            (
                "parent-3".to_string(),
                vec![make_child_wf("ProcessOrder", ChildWorkflowStatus::Completed, Some(15000))],
            ),
        ];

        let findings = compute_child_workflow_findings(&samples);
        let latency_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.category == InsightCategory::ChildWorkflowLatency)
            .collect();

        assert_eq!(latency_findings.len(), 1);
        assert_eq!(latency_findings[0].severity, InsightSeverity::Critical);
    }

    #[test]
    fn test_child_wf_no_findings_all_healthy() {
        let samples = vec![
            (
                "parent-1".to_string(),
                vec![make_child_wf("ProcessOrder", ChildWorkflowStatus::Completed, Some(100))],
            ),
            (
                "parent-2".to_string(),
                vec![make_child_wf("ProcessOrder", ChildWorkflowStatus::Completed, Some(200))],
            ),
        ];

        let findings = compute_child_workflow_findings(&samples);
        assert!(findings.is_empty());
    }
}
