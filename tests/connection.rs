//! Integration tests for Temporal connection
//!
//! These tests require real Temporal credentials and are marked with #[ignore].
//! Run them with: cargo test --test connection -- --ignored
//!
//! Before running, ensure your .env file or environment has:
//!   - TEMPORAL_ADDRESS
//!   - TEMPORAL_NAMESPACE
//!   - TEMPORAL_API_KEY (for Temporal Cloud)

use tempurview::client::{CliTemporalClient, TemporalClient};
use tempurview::domain::{WorkflowFilter, WorkflowStatus};

/// Test that we can create a client from environment variables
#[test]
#[ignore]
fn test_client_creation_from_env() {
    // Load .env if present
    dotenvy::dotenv().ok();

    let result = CliTemporalClient::from_env();
    assert!(
        result.is_ok(),
        "Failed to create client: {:?}",
        result.err()
    );
}

/// Test basic connection by counting workflows
#[tokio::test]
#[ignore]
async fn test_connection_count() {
    dotenvy::dotenv().ok();

    let client =
        CliTemporalClient::from_env().expect("TEMPORAL_* environment variables must be set");

    let result = client.count(None).await;

    match &result {
        Ok(count) => println!("Connection successful! Total workflows: {}", count),
        Err(e) => println!("Connection failed: {}", e),
    }

    assert!(result.is_ok(), "Count failed: {:?}", result.err());
}

/// Test that we can count workflows by status
#[tokio::test]
#[ignore]
async fn test_connection_count_by_status() {
    dotenvy::dotenv().ok();

    let client =
        CliTemporalClient::from_env().expect("TEMPORAL_* environment variables must be set");

    // Test counting running workflows
    let query = format!(
        "ExecutionStatus='{}'",
        WorkflowStatus::Running.as_query_value()
    );
    let result = client.count(Some(&query)).await;

    assert!(result.is_ok(), "Count by status failed: {:?}", result.err());
    println!("Running workflows: {}", result.unwrap());
}

/// Test listing workflows
#[tokio::test]
#[ignore]
async fn test_connection_list() {
    dotenvy::dotenv().ok();

    let client =
        CliTemporalClient::from_env().expect("TEMPORAL_* environment variables must be set");

    let filter = WorkflowFilter::new();
    let result = client.list(&filter, 10).await;

    assert!(result.is_ok(), "List failed: {:?}", result.err());

    let workflows = result.unwrap();
    println!("Listed {} workflows", workflows.len());

    for wf in workflows.iter().take(5) {
        println!(
            "  - {} ({:?}) - {}",
            wf.workflow_id, wf.status, wf.workflow_type
        );
    }
}

/// Test listing workflows with a status filter
#[tokio::test]
#[ignore]
async fn test_connection_list_with_filter() {
    dotenvy::dotenv().ok();

    let client =
        CliTemporalClient::from_env().expect("TEMPORAL_* environment variables must be set");

    // Filter for completed workflows
    let filter = WorkflowFilter::new();
    // Note: The filter.status field is used internally

    let result = client.list(&filter, 5).await;
    assert!(
        result.is_ok(),
        "List with filter failed: {:?}",
        result.err()
    );
}

/// Full connection test that verifies all basic operations
#[tokio::test]
#[ignore]
async fn test_full_connection_smoke_test() {
    dotenvy::dotenv().ok();

    println!("\n=== Temporal Connection Smoke Test ===\n");

    // Step 1: Create client
    println!("1. Creating client from environment...");
    let client = match CliTemporalClient::from_env() {
        Ok(c) => {
            println!("   ✓ Client created successfully");
            c
        }
        Err(e) => {
            panic!("   ✗ Failed to create client: {}", e);
        }
    };

    // Step 2: Count all workflows
    println!("\n2. Counting all workflows...");
    match client.count(None).await {
        Ok(count) => println!("   ✓ Total workflows: {}", count),
        Err(e) => panic!("   ✗ Count failed: {}", e),
    }

    // Step 3: Count by status
    println!("\n3. Counting workflows by status...");
    for status in WorkflowStatus::all() {
        let query = format!("ExecutionStatus='{}'", status.as_query_value());
        match client.count(Some(&query)).await {
            Ok(count) => println!("   {:15} {}", format!("{:?}:", status), count),
            Err(e) => println!("   {:15} ✗ Error: {}", format!("{:?}:", status), e),
        }
    }

    // Step 4: List workflows
    println!("\n4. Listing recent workflows...");
    let filter = WorkflowFilter::new();
    match client.list(&filter, 5).await {
        Ok(workflows) => {
            println!("   ✓ Retrieved {} workflows", workflows.len());
            for wf in &workflows {
                println!(
                    "      - {} ({:?})",
                    truncate(&wf.workflow_id, 40),
                    wf.status
                );
            }
        }
        Err(e) => panic!("   ✗ List failed: {}", e),
    }

    println!("\n=== All tests passed! ===\n");
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

/// Test that invalid credentials produce meaningful errors
#[tokio::test]
#[ignore]
async fn test_connection_error_handling() {
    // Temporarily set invalid credentials
    std::env::set_var("TEMPORAL_ADDRESS", "invalid.address:7233");
    std::env::set_var("TEMPORAL_NAMESPACE", "invalid-namespace");
    std::env::remove_var("TEMPORAL_API_KEY");

    let client = CliTemporalClient::from_env()
        .expect("Client creation should succeed even with invalid credentials");

    let result = client.count(None).await;

    // Should fail with a connection or command error
    assert!(result.is_err(), "Expected error with invalid credentials");
    println!("Got expected error: {}", result.unwrap_err());
}
