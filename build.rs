use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Embed git version at compile time
    set_git_version();

    let proto_root = PathBuf::from("proto/temporal-api");

    // Tell Cargo to rerun if protos change
    println!("cargo:rerun-if-changed=proto/temporal-api");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");

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

/// Set GIT_VERSION environment variable for compile-time embedding
fn set_git_version() {
    // Try to get version from git describe (uses tags)
    let version = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    println!("cargo:rustc-env=GIT_VERSION={}", version);
}
