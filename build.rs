use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = PathBuf::from("proto/temporal-api");

    // Tell Cargo to rerun if protos change
    println!("cargo:rerun-if-changed=proto/temporal-api");

    // Compile the Temporal WorkflowService protos
    tonic_build::configure()
        .build_server(false) // We only need the client
        .compile_protos(
            &[
                proto_root.join("temporal/api/workflowservice/v1/service.proto"),
            ],
            &[
                // Include paths for proto resolution
                proto_root.as_path(),
            ],
        )?;

    Ok(())
}
