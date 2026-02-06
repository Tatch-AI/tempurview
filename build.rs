use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Embed git version at compile time
    set_git_version();

    let proto_root = PathBuf::from("proto/temporal-api");
    let service_proto = proto_root.join("temporal/api/workflowservice/v1/service.proto");

    if service_proto.exists() {
        // Dev mode: proto submodule present, regenerate into checked-in directory
        println!("cargo:rerun-if-changed=proto/temporal-api");

        let out_dir = PathBuf::from("src/proto/generated");
        std::fs::create_dir_all(&out_dir)?;

        tonic_build::configure()
            .build_server(false)
            .out_dir(&out_dir)
            .compile_protos(
                &[service_proto],
                &[proto_root.as_path()],
            )?;
    }
    // else: crates.io install — no proto submodule, use checked-in generated code

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
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");
}
