use tempurview::client::{GrpcTemporalClient, TemporalClient};
use tempurview::domain::WorkflowFilter;

#[tokio::test]
#[ignore] // Run with: cargo test --test grpc_manual_test -- --ignored --nocapture
async fn test_grpc_client_directly() {
    dotenvy::dotenv().ok();
    
    println!("Creating gRPC client...");
    let client = GrpcTemporalClient::from_env().await.expect("Failed to create client");
    
    println!("Counting workflows...");
    let count = client.count(None).await.expect("Failed to count");
    println!("Total workflows: {}", count);
    
    println!("\nListing workflows (limit 5)...");
    let filter = WorkflowFilter::new();
    let workflows = client.list(&filter, 5).await.expect("Failed to list");
    
    for wf in &workflows {
        println!("  - {} ({:?})", wf.workflow_id, wf.status);
    }
    
    println!("\nDone! gRPC client works.");
}
