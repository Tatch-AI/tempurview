use crate::client::{GrpcTemporalClient, MockTemporalClient, TemporalClient};
use crate::config::Config;
use crate::domain::WorkflowStatus;
use tracing::info;

pub async fn handle(config: &Config) -> color_eyre::Result<()> {
    info!("Running connection test");
    println!("Testing Temporal connection...\n");

    if config.use_mock {
        println!("Using mock client (--mock flag set).");
        let client = MockTemporalClient::with_random_data(config.mock_workflow_count);
        let count = client.count(None).await?;
        println!("\nMock connection successful! ({count} workflows)");
        return Ok(());
    }

    println!("Environment variables:");
    match std::env::var("TEMPORAL_ADDRESS") {
        Ok(addr) => println!("  TEMPORAL_ADDRESS:   {}", addr),
        Err(_) => println!("  TEMPORAL_ADDRESS:   (not set, using: {})", config.temporal_address),
    }
    println!("  TEMPORAL_NAMESPACE: {}", config.temporal_namespace);
    match std::env::var("TEMPORAL_API_KEY") {
        Ok(_) => println!("  TEMPORAL_API_KEY:   (set, hidden)"),
        Err(_) => println!("  TEMPORAL_API_KEY:   (not set - may be required for Temporal Cloud)"),
    }
    println!();

    println!("Attempting to connect via gRPC...");
    let client = GrpcTemporalClient::connect(
        &config.temporal_address,
        config.temporal_namespace.clone(),
        config.temporal_api_key.clone(),
    )
    .await
    .map_err(|e| {
        eprintln!("Failed to connect: {e}");
        eprintln!("\nMake sure TEMPORAL_ADDRESS and TEMPORAL_NAMESPACE are set.");
        eprintln!("For Temporal Cloud, you also need TEMPORAL_API_KEY.");
        color_eyre::eyre::eyre!("Connection failed: {e}")
    })?;

    match client.count(None).await {
        Ok(count) => {
            println!("\nConnection successful!");
            println!("  Total workflows: {count}");

            println!("\nWorkflow counts by status:");
            for status in WorkflowStatus::all() {
                let query = format!("ExecutionStatus='{}'", status.as_query_value());
                match client.count(Some(&query)).await {
                    Ok(n) if n > 0 => println!("  {:15} {}", format!("{:?}:", status), n),
                    Ok(_) => {}
                    Err(_) => println!("  {:15} (query failed)", format!("{:?}:", status)),
                }
            }
            println!("\nAll tests passed! Your gRPC connection is working.");
        }
        Err(e) => {
            eprintln!("\nConnection failed: {e}");
            eprintln!("\nTroubleshooting tips:");
            eprintln!("  1. Verify your TEMPORAL_ADDRESS is correct");
            eprintln!("  2. Verify your TEMPORAL_NAMESPACE matches your Temporal namespace");
            eprintln!("  3. For Temporal Cloud, ensure TEMPORAL_API_KEY is valid");
            return Err(color_eyre::eyre::eyre!("Connection test failed: {e}"));
        }
    }

    Ok(())
}
