# Testing Guide for Tempurview

This guide covers testing patterns used in Tempurview and Rust testing best practices.

## Table of Contents

1. [Test Organization](#test-organization)
2. [Running Tests](#running-tests)
3. [Unit Tests](#unit-tests)
4. [Integration Tests](#integration-tests)
5. [Testing Async Code](#testing-async-code)
6. [Mocking and Test Doubles](#mocking-and-test-doubles)
7. [Test Helpers and Fixtures](#test-helpers-and-fixtures)
8. [Testing the Temporal Connection](#testing-the-temporal-connection)

---

## Test Organization

Rust has two main places for tests:

### Inline Unit Tests (`#[cfg(test)]` modules)

Located in the same file as the code they test. Best for:
- Testing private functions
- Testing small, focused units of logic
- Fast feedback during development

```rust
// In src/domain/workflow.rs
pub fn parse_status(s: &str) -> Option<WorkflowStatus> {
    // implementation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_running() {
        assert_eq!(parse_status("RUNNING"), Some(WorkflowStatus::Running));
    }
}
```

### Integration Tests (`tests/` directory)

Separate files in `tests/` folder. Best for:
- Testing public API
- Testing multiple modules together
- Testing against real external services (with `#[ignore]`)

```
tests/
├── common/           # Shared test utilities
│   └── mod.rs
├── integration.rs    # Integration tests
└── connection.rs     # Connection/smoke tests
```

---

## Running Tests

```bash
# Run all unit tests (fast, no external dependencies)
cargo test

# Run tests with output visible (useful for debugging)
cargo test -- --nocapture

# Run a specific test
cargo test test_parse_status

# Run tests matching a pattern
cargo test workflow

# Run ignored tests (integration tests needing real credentials)
cargo test -- --ignored

# Run ALL tests including ignored
cargo test -- --include-ignored

# Run tests in release mode (faster execution, slower compile)
cargo test --release
```

---

## Unit Tests

### Basic Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // Arrange: Set up test data
        let input = "test";

        // Act: Call the function
        let result = my_function(input);

        // Assert: Verify the result
        assert_eq!(result, expected);
    }
}
```

### Testing for Errors

```rust
#[test]
fn test_invalid_input_returns_error() {
    let result = parse_count_output("invalid");
    assert!(result.is_err());
}

#[test]
fn test_specific_error_type() {
    let result = parse_count_output("invalid");
    assert!(matches!(result, Err(ClientError::ParseError(_))));
}
```

### Testing with `Result`

```rust
// Tests can return Result for cleaner error handling
#[test]
fn test_with_result() -> Result<(), Box<dyn std::error::Error>> {
    let output = parse_count_output("42")?;
    assert_eq!(output, 42);
    Ok(())
}
```

### Parameterized Tests (Table-Driven)

Rust doesn't have built-in parameterized tests, but you can simulate them:

```rust
#[test]
fn test_status_parsing_all_variants() {
    let cases = vec![
        ("RUNNING", Some(WorkflowStatus::Running)),
        ("COMPLETED", Some(WorkflowStatus::Completed)),
        ("FAILED", Some(WorkflowStatus::Failed)),
        ("invalid", None),
        ("", None),
    ];

    for (input, expected) in cases {
        assert_eq!(
            WorkflowStatus::from_str(input),
            expected,
            "Failed for input: {}",
            input
        );
    }
}
```

### Testing Panics

```rust
#[test]
#[should_panic(expected = "index out of bounds")]
fn test_panic_condition() {
    let v = vec![1, 2, 3];
    let _ = v[99]; // This will panic
}
```

---

## Integration Tests

Integration tests live in `tests/` and test the public API:

```rust
// tests/integration.rs
use tempurview::client::{MockTemporalClient, TemporalClient};
use tempurview::domain::WorkflowFilter;

#[tokio::test]
async fn test_mock_client_list_workflows() {
    let client = MockTemporalClient::with_random_data(50);
    let filter = WorkflowFilter::new();

    let workflows = client.list(&filter, 10).await.unwrap();

    assert!(workflows.len() <= 10);
}
```

### Ignored Tests (Require External Services)

Use `#[ignore]` for tests that need real credentials:

```rust
#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn test_real_temporal_connection() {
    // This test requires TEMPORAL_* env vars to be set
    let client = CliTemporalClient::from_env().unwrap();
    let count = client.count(None).await.unwrap();
    println!("Total workflows: {}", count);
}
```

---

## Testing Async Code

### Basic Async Test

```rust
#[tokio::test]
async fn test_async_function() {
    let result = some_async_function().await;
    assert!(result.is_ok());
}
```

### Testing with Timeouts

```rust
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_with_timeout() {
    let result = timeout(
        Duration::from_secs(5),
        some_async_function()
    ).await;

    assert!(result.is_ok(), "Operation timed out");
}
```

### Testing Concurrent Operations

```rust
#[tokio::test]
async fn test_concurrent_requests() {
    let client = MockTemporalClient::with_random_data(100);

    let (count1, count2) = tokio::join!(
        client.count(Some("ExecutionStatus='Running'")),
        client.count(Some("ExecutionStatus='Failed'"))
    );

    assert!(count1.is_ok());
    assert!(count2.is_ok());
}
```

---

## Mocking and Test Doubles

### Trait-Based Mocking

Our `TemporalClient` trait enables easy mocking:

```rust
// The trait defines the interface
#[async_trait]
pub trait TemporalClient: Send + Sync {
    async fn count(&self, query: Option<&str>) -> ClientResult<u64>;
    async fn list(&self, filter: &WorkflowFilter, limit: u32) -> ClientResult<Vec<WorkflowSummary>>;
    // ...
}

// MockTemporalClient implements it for testing
pub struct MockTemporalClient {
    pub workflows: Vec<WorkflowSummary>,
    pub should_fail: bool,
    // ...
}

#[async_trait]
impl TemporalClient for MockTemporalClient {
    async fn count(&self, query: Option<&str>) -> ClientResult<u64> {
        if self.should_fail {
            return Err(ClientError::ConnectionError("Mock failure".into()));
        }
        // ... mock implementation
    }
}
```

### Testing Error Conditions

```rust
#[tokio::test]
async fn test_handles_connection_failure() {
    let client = MockTemporalClient::new()
        .with_failure(); // Configure mock to fail

    let result = client.count(None).await;

    assert!(matches!(result, Err(ClientError::ConnectionError(_))));
}
```

---

## Test Helpers and Fixtures

### Creating Test Data Builders

```rust
// In tests/common/mod.rs or src/test_helpers.rs

pub fn make_test_workflow(status: WorkflowStatus) -> WorkflowSummary {
    WorkflowSummary {
        workflow_id: format!("test-{}", uuid::Uuid::new_v4()),
        run_id: format!("run-{}", uuid::Uuid::new_v4()),
        workflow_type: "TestWorkflow".to_string(),
        status,
        start_time: Utc::now(),
        close_time: None,
        task_queue: "default".to_string(),
    }
}

pub fn make_test_workflows(statuses: &[WorkflowStatus]) -> Vec<WorkflowSummary> {
    statuses.iter().map(|s| make_test_workflow(*s)).collect()
}
```

### Using Test Fixtures

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Shared setup for multiple tests
    fn setup_app_with_workflows() -> App {
        let mut app = App::new();
        app.workflows = LoadState::Loaded(vec![
            make_test_workflow(WorkflowStatus::Running),
            make_test_workflow(WorkflowStatus::Failed),
        ]);
        app
    }

    #[test]
    fn test_navigation_with_workflows() {
        let mut app = setup_app_with_workflows();
        app.update(Action::NavigateDown);
        assert_eq!(app.list_state.selected(), Some(1));
    }

    #[test]
    fn test_filter_with_workflows() {
        let mut app = setup_app_with_workflows();
        app.update(Action::SetStatusFilter(Some(WorkflowStatus::Running)));
        // ...
    }
}
```

---

## Testing the Temporal Connection

### Quick Connection Test (CLI)

Use the built-in connection test:

```bash
# Test connection with current .env or environment variables
tempurview --test-connection

# Expected output on success:
# ✓ Connected to Temporal
#   Address: your-namespace.tmprl.cloud:7233
#   Namespace: your-namespace
#   Total workflows: 42
```

### Integration Test

```rust
// tests/connection.rs

#[tokio::test]
#[ignore] // Only run manually with: cargo test connection -- --ignored
async fn test_temporal_connection() {
    // Load .env file
    dotenvy::dotenv().ok();

    // Try to create client from environment
    let client = CliTemporalClient::from_env()
        .expect("TEMPORAL_* environment variables must be set");

    // Try to count workflows (simplest operation)
    let result = client.count(None).await;

    match result {
        Ok(count) => {
            println!("✓ Connection successful! Total workflows: {}", count);
        }
        Err(e) => {
            panic!("✗ Connection failed: {}", e);
        }
    }
}
```

### Environment Variable Checklist

For Temporal Cloud, you need:

```bash
# Required
TEMPORAL_ADDRESS=<namespace>.<account>.tmprl.cloud:7233
TEMPORAL_NAMESPACE=<namespace>.<account>

# Required for Temporal Cloud (API Key authentication)
TEMPORAL_API_KEY=<your-api-key>
```

To get these values:
1. Log into [cloud.temporal.io](https://cloud.temporal.io)
2. Go to your namespace
3. Click "Connect" or find connection info
4. The address format is typically: `<namespace>.<accountId>.tmprl.cloud:7233`
5. Generate an API key under Settings > API Keys

For self-hosted Temporal:

```bash
TEMPORAL_ADDRESS=localhost:7233
TEMPORAL_NAMESPACE=default
# TEMPORAL_API_KEY not needed for most self-hosted setups
```

---

## Best Practices Summary

1. **Keep unit tests fast** - No I/O, no network, no sleeping
2. **Use `#[ignore]` for slow/external tests** - Run them explicitly
3. **Test behavior, not implementation** - Focus on what, not how
4. **Use meaningful assertion messages** - `assert!(x, "Expected foo but got {}", x)`
5. **One assertion per test** (when practical) - Easier to debug failures
6. **Use test helpers** - DRY up common setup code
7. **Test edge cases** - Empty inputs, max values, error conditions
8. **Mock external dependencies** - Use traits and test doubles
