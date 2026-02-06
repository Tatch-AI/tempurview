//! Generated protobuf code for Temporal API
//!
//! This module includes pre-generated tonic code from the Temporal proto files.
//! In dev mode (proto submodule present), build.rs regenerates these files.
//! On crates.io installs, the checked-in generated code is used directly.

#![allow(clippy::all)]
#![allow(missing_docs)]
#![allow(unused_imports)]

pub mod google {
    pub mod api {
        include!("generated/google.api.rs");
    }
}

pub mod temporal {
    pub mod api {
        pub mod activity {
            pub mod v1 {
                include!("generated/temporal.api.activity.v1.rs");
            }
        }
        pub mod batch {
            pub mod v1 {
                include!("generated/temporal.api.batch.v1.rs");
            }
        }
        pub mod command {
            pub mod v1 {
                include!("generated/temporal.api.command.v1.rs");
            }
        }
        pub mod common {
            pub mod v1 {
                include!("generated/temporal.api.common.v1.rs");
            }
        }
        pub mod deployment {
            pub mod v1 {
                include!("generated/temporal.api.deployment.v1.rs");
            }
        }
        pub mod enums {
            pub mod v1 {
                include!("generated/temporal.api.enums.v1.rs");
            }
        }
        pub mod failure {
            pub mod v1 {
                include!("generated/temporal.api.failure.v1.rs");
            }
        }
        pub mod filter {
            pub mod v1 {
                include!("generated/temporal.api.filter.v1.rs");
            }
        }
        pub mod history {
            pub mod v1 {
                include!("generated/temporal.api.history.v1.rs");
            }
        }
        pub mod namespace {
            pub mod v1 {
                include!("generated/temporal.api.namespace.v1.rs");
            }
        }
        pub mod nexus {
            pub mod v1 {
                include!("generated/temporal.api.nexus.v1.rs");
            }
        }
        pub mod protocol {
            pub mod v1 {
                include!("generated/temporal.api.protocol.v1.rs");
            }
        }
        pub mod query {
            pub mod v1 {
                include!("generated/temporal.api.query.v1.rs");
            }
        }
        pub mod replication {
            pub mod v1 {
                include!("generated/temporal.api.replication.v1.rs");
            }
        }
        pub mod rules {
            pub mod v1 {
                include!("generated/temporal.api.rules.v1.rs");
            }
        }
        pub mod schedule {
            pub mod v1 {
                include!("generated/temporal.api.schedule.v1.rs");
            }
        }
        pub mod sdk {
            pub mod v1 {
                include!("generated/temporal.api.sdk.v1.rs");
            }
        }
        pub mod taskqueue {
            pub mod v1 {
                include!("generated/temporal.api.taskqueue.v1.rs");
            }
        }
        pub mod update {
            pub mod v1 {
                include!("generated/temporal.api.update.v1.rs");
            }
        }
        pub mod version {
            pub mod v1 {
                include!("generated/temporal.api.version.v1.rs");
            }
        }
        pub mod worker {
            pub mod v1 {
                include!("generated/temporal.api.worker.v1.rs");
            }
        }
        pub mod workflow {
            pub mod v1 {
                include!("generated/temporal.api.workflow.v1.rs");
            }
        }
        pub mod workflowservice {
            pub mod v1 {
                include!("generated/temporal.api.workflowservice.v1.rs");
            }
        }
    }
}

// Re-export commonly used types
pub use temporal::api::workflowservice::v1::workflow_service_client::WorkflowServiceClient;
pub use temporal::api::workflowservice::v1::*;
