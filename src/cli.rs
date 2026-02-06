use clap::{Parser, Subcommand, ValueEnum};

/// Version from git tag at build time, falls back to Cargo.toml version
const VERSION: &str = env!("GIT_VERSION");

#[derive(Parser)]
#[command(
    name = "tempurview",
    version = VERSION,
    about = "A terminal interface for Temporal workflows",
    long_about = "Tempurview is a CLI and TUI tool for viewing and managing Temporal workflows.\n\nRun without a subcommand to launch the interactive TUI."
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(clap::Args)]
pub struct GlobalArgs {
    /// Temporal server address
    #[arg(long, global = true, env = "TEMPORAL_ADDRESS")]
    pub address: Option<String>,

    /// Temporal namespace
    #[arg(long, global = true, env = "TEMPORAL_NAMESPACE")]
    pub namespace: Option<String>,

    /// Use mock data instead of connecting to Temporal
    #[arg(long, global = true, default_value_t = false)]
    pub mock: bool,

    /// Number of mock workflows to generate
    #[arg(long, global = true, default_value_t = 100)]
    pub mock_count: usize,

    /// Maximum workflows to fetch
    #[arg(long, global = true, default_value_t = 50)]
    pub limit: u32,

    /// Output format (auto-detects: table for TTY, JSON for pipe)
    #[arg(long, global = true, value_enum)]
    pub output: Option<OutputFormatArg>,

    /// Show log file location and recent errors
    #[arg(long, global = true, default_value_t = false)]
    pub logs: bool,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OutputFormatArg {
    Json,
    Table,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage workflows
    Workflow {
        #[command(subcommand)]
        action: WorkflowAction,
    },
    /// View activities for a workflow
    Activity {
        #[command(subcommand)]
        action: ActivityAction,
    },
    /// View history events for a workflow
    Event {
        #[command(subcommand)]
        action: EventAction,
    },
    /// Scan workflows for operational insights
    Insight {
        #[command(subcommand)]
        action: InsightAction,
    },
    /// Show configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Test connection to Temporal server
    TestConnection,
    /// Start a web UI server
    Serve {
        /// Port to listen on
        #[arg(long, default_value_t = 3000)]
        port: u16,

        /// Address to bind to
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
    },
}

#[derive(Subcommand)]
pub enum WorkflowAction {
    /// List workflows
    List {
        /// Filter by execution status
        #[arg(long)]
        status: Option<String>,

        /// Filter by workflow type
        #[arg(long)]
        workflow_type: Option<String>,

        /// Show workflows started after this time (e.g., 2h, 3d, 2024-01-15)
        #[arg(long)]
        since: Option<String>,

        /// Show workflows started before this time
        #[arg(long)]
        before: Option<String>,
    },
    /// Get details of a specific workflow
    Get {
        /// Workflow ID
        workflow_id: String,

        /// Run ID (optional, defaults to latest run)
        #[arg(long)]
        run_id: Option<String>,
    },
    /// Count workflows matching a filter
    Count {
        /// Filter by execution status
        #[arg(long)]
        status: Option<String>,

        /// Raw Temporal visibility query
        #[arg(long)]
        query: Option<String>,
    },
    /// Cancel a running workflow
    Cancel {
        /// Workflow ID
        workflow_id: String,

        /// Run ID (optional)
        #[arg(long)]
        run_id: Option<String>,
    },
    /// Terminate a workflow
    Terminate {
        /// Workflow ID
        workflow_id: String,

        /// Run ID (optional)
        #[arg(long)]
        run_id: Option<String>,

        /// Reason for termination
        #[arg(long, default_value = "Terminated via CLI")]
        reason: String,
    },
}

#[derive(Subcommand)]
pub enum ActivityAction {
    /// List activities for a workflow
    List {
        /// Workflow ID
        workflow_id: String,

        /// Run ID (optional)
        #[arg(long)]
        run_id: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum EventAction {
    /// List history events for a workflow
    List {
        /// Workflow ID
        workflow_id: String,

        /// Run ID (optional)
        #[arg(long)]
        run_id: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum InsightAction {
    /// Scan workflows for operational insights
    Scan {
        /// Show workflows started after this time (e.g., 2h, 3d, 2024-01-15)
        #[arg(long)]
        since: Option<String>,

        /// Show workflows started before this time
        #[arg(long)]
        before: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show resolved configuration
    Show,
}
