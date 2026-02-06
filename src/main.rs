/// Version from git tag at build time, falls back to Cargo.toml version
const VERSION: &str = env!("GIT_VERSION");

use tempurview::action::Action;
use tempurview::app::{App, Effect, InputMode, View};
use tempurview::cli_worker::{CliHandle, CliRequest, CliWorker};
use tempurview::client::{GrpcTemporalClient, MockTemporalClient, TemporalClient};
use tempurview::config::Config;
use tempurview::domain::WorkflowStatus;
use tempurview::event::{event_to_action, EventHandler};
use tempurview::logging;
use tempurview::tui::Tui;
use tempurview::widgets::{
    ActivityListWidget, FilterInput, HelpBar, HelpOverlay, InsightDetailWidget, InsightsWidget,
    StatusDashboard, TypeListWidget, WorkflowDetailWidget, WorkflowListWidget,
};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    // Load .env file - check current directory first, then ~/.tempurview/.env
    if dotenvy::dotenv().is_err() {
        if let Some(config_dir) = logging::config_dir() {
            let env_path = config_dir.join(".env");
            let _ = dotenvy::from_path(&env_path);
        }
    }

    // Initialize logging FIRST (before color_eyre which may set up its own subscriber)
    // Keep the guard alive for the duration of the program to ensure logs are flushed
    let _log_guard = match logging::init() {
        Ok(guard) => Some(guard),
        Err(e) => {
            eprintln!("Warning: Failed to initialize logging: {}", e);
            None
        }
    };

    color_eyre::install()?;

    // Parse configuration
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Check for version flag
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("tempurview {}", VERSION);
        return Ok(());
    }

    // Check for help flag
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    // Check for logs flag
    if args.iter().any(|a| a == "--logs") {
        show_logs_info();
        return Ok(());
    }

    // Check for test-connection flag
    if args.iter().any(|a| a == "--test-connection") {
        return test_connection().await;
    }

    let config = Config::from_args(&args)?;
    info!("Starting Tempurview");
    debug!(
        "Config: use_mock={}, limit={}",
        config.use_mock, config.default_limit
    );

    // Create client (gRPC for real connections, mock for testing)
    let client: Arc<dyn TemporalClient> = if config.use_mock {
        info!(
            "Using mock client with {} workflows",
            config.mock_workflow_count
        );
        Arc::new(MockTemporalClient::with_random_data(
            config.mock_workflow_count,
        ))
    } else {
        println!("Connecting to Temporal via gRPC...");
        match GrpcTemporalClient::from_env().await {
            Ok(c) => {
                info!("Created gRPC Temporal client");
                Arc::new(c)
            }
            Err(e) => {
                error!("Failed to create gRPC Temporal client: {}", e);
                eprintln!("Connection failed: {}", e);
                eprintln!("\nMake sure the following environment variables are set:");
                eprintln!("  TEMPORAL_ADDRESS   - Temporal server address (e.g., us-west1.gcp.api.temporal.io:7233)");
                eprintln!("  TEMPORAL_NAMESPACE - Temporal namespace");
                eprintln!("  TEMPORAL_API_KEY   - API key for authentication (required for Temporal Cloud)");
                eprintln!("\nOr use --mock to run with simulated data");
                return Ok(());
            }
        }
    };

    // Verify connection works before entering raw mode
    if !config.use_mock {
        match client.count(None).await {
            Ok(count) => {
                println!("Connected! ({} workflows)", count);
                info!("Connection verified: {} workflows", count);
            }
            Err(e) => {
                error!("Connection verification failed: {}", e);
                eprintln!("Connection verification failed: {}", e);
                eprintln!("\nTroubleshooting tips:");
                eprintln!("  1. Verify your TEMPORAL_ADDRESS is correct");
                eprintln!("  2. Verify your TEMPORAL_NAMESPACE matches your Temporal namespace");
                eprintln!("  3. For Temporal Cloud, ensure TEMPORAL_API_KEY is valid");
                eprintln!("  4. Try running: tempurview --test-connection");
                return Ok(());
            }
        }
    }

    // NOW enter raw mode - after connection is verified
    let mut tui = Tui::new()?;
    info!("TUI initialized");

    // Create app state
    let mut app = App::new();
    app.temporal_namespace = config.temporal_namespace.clone();

    // Create event handler
    let mut events = EventHandler::new(config.tick_rate);

    // Create action channel for async data loading
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

    // Create CLI worker for serialized request execution
    let (request_tx, request_rx) = mpsc::unbounded_channel::<CliRequest>();
    let cli_worker = CliWorker::new(client.clone(), request_rx, action_tx.clone());
    let _worker_handle = cli_worker.spawn();
    let cli_handle = CliHandle::new(request_tx);

    // Initial data load - load both counts and workflows
    cli_handle.load_counts();
    cli_handle.load_workflows(app.filter.clone(), config.default_limit);

    // Main loop
    loop {
        // Render
        tui.terminal().draw(|frame| render(&app, frame))?;

        // Handle events
        tokio::select! {
            Some(event) = events.next() => {
                debug!("Event received: {:?}", event);
                debug!("Current state - view: {:?}, input_mode: {:?}", app.view, app.input_mode);
                let action = event_to_action(event, app.view, app.input_mode);
                debug!("Action mapped: {:?}", action);
                if let Some(action) = action {
                    let effects = app.update(action);
                    handle_effects(effects, &cli_handle, &app, config.default_limit, &action_tx);
                }
            }
            Some(action) = action_rx.recv() => {
                let effects = app.update(action);
                handle_effects(effects, &cli_handle, &app, config.default_limit, &action_tx);
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn print_help() {
    println!("Tempurview {} - A terminal interface for Temporal workflows", VERSION);
    println!();
    println!("USAGE:");
    println!("    tempurview [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    --mock              Use mock data instead of connecting to Temporal");
    println!("    --mock-count N      Number of mock workflows to generate (default: 100)");
    println!("    --address ADDR      Temporal server address (overrides TEMPORAL_ADDRESS)");
    println!("    --namespace NS      Temporal namespace (overrides TEMPORAL_NAMESPACE)");
    println!("    --limit N           Maximum workflows to fetch (default: 50)");
    println!("    --test-connection   Test connection to Temporal and exit");
    println!("    --logs              Show log file location and recent errors");
    println!("    -h, --help          Show this help message");
    println!("    -V, --version       Show version information");
    println!();
    println!("ENVIRONMENT VARIABLES:");
    println!("    TEMPORAL_ADDRESS    Temporal server address (e.g., localhost:7233)");
    println!("    TEMPORAL_NAMESPACE  Temporal namespace");
    println!("    TEMPORAL_API_KEY    (optional) API key for authentication");
    println!();
    println!("KEYBOARD SHORTCUTS:");
    println!("    j/k or arrows   Navigate up/down");
    println!("    Enter           Select/view details");
    println!("    Esc             Go back");
    println!("    1-5             Filter by status");
    println!("    0               Clear filters");
    println!("    /               Open filter input");
    println!("    r               Refresh data");
    println!("    ?               Toggle help");
    println!("    q               Quit");
}

fn show_logs_info() {
    println!("Tempurview Logs");
    println!("===============\n");

    match logging::logs_dir() {
        Some(dir) => {
            println!("Log directory: {}", dir.display());
            println!();

            // List log files
            if dir.exists() {
                println!("Log files:");
                match std::fs::read_dir(&dir) {
                    Ok(entries) => {
                        let mut files: Vec<_> = entries
                            .filter_map(|e| e.ok())
                            .filter(|e| {
                                e.file_name()
                                    .to_string_lossy()
                                    .starts_with("tempurview.log")
                            })
                            .collect();
                        files.sort_by_key(|e| {
                            std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok()))
                        });

                        if files.is_empty() {
                            println!("  (no log files yet)");
                        } else {
                            for entry in files.iter().take(5) {
                                let path = entry.path();
                                let size = entry.metadata().ok().map(|m| m.len()).unwrap_or(0);
                                println!(
                                    "  {} ({} bytes)",
                                    path.file_name().unwrap().to_string_lossy(),
                                    size
                                );
                            }
                            if files.len() > 5 {
                                println!("  ... and {} more", files.len() - 5);
                            }
                        }
                    }
                    Err(e) => println!("  Error reading directory: {}", e),
                }

                // Show recent errors from the most recent log file
                println!();
                println!("Recent errors (last 10):");
                let log_files: Vec<_> = std::fs::read_dir(&dir)
                    .ok()
                    .map(|entries| {
                        let mut files: Vec<_> = entries
                            .filter_map(|e| e.ok())
                            .filter(|e| {
                                e.file_name()
                                    .to_string_lossy()
                                    .starts_with("tempurview.log")
                            })
                            .collect();
                        files.sort_by_key(|e| {
                            std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok()))
                        });
                        files
                    })
                    .unwrap_or_default();

                if log_files.is_empty() {
                    println!("  (no log files yet)");
                } else {
                    let mut found_errors = false;
                    for entry in log_files.iter().take(3) {
                        if let Ok(content) = std::fs::read_to_string(entry.path()) {
                            let errors: Vec<&str> = content
                                .lines()
                                .filter(|line| line.contains("ERROR"))
                                .collect();
                            for line in errors.iter().rev().take(10) {
                                println!("  {}", line);
                                found_errors = true;
                            }
                        }
                    }
                    if !found_errors {
                        println!("  (no errors)");
                    }
                }
            } else {
                println!("Log directory does not exist yet.");
                println!("It will be created when you first run tempurview.");
            }

            println!();
            println!("To view logs in real-time:");
            println!("  tail -f {}/tempurview.log", dir.display());
            println!();
            println!("To enable debug logging:");
            println!("  RUST_LOG=tempurview=debug tempurview");
        }
        None => {
            println!("Could not determine log directory (HOME not set?)");
        }
    }
}

async fn test_connection() -> color_eyre::Result<()> {
    info!("Running connection test");
    println!("Testing Temporal connection...\n");

    // Check environment variables
    let address = std::env::var("TEMPORAL_ADDRESS");
    let namespace = std::env::var("TEMPORAL_NAMESPACE");
    let api_key = std::env::var("TEMPORAL_API_KEY");

    println!("Environment variables:");
    match &address {
        Ok(addr) => println!("  TEMPORAL_ADDRESS:   {}", addr),
        Err(_) => println!("  TEMPORAL_ADDRESS:   ✗ NOT SET"),
    }
    match &namespace {
        Ok(ns) => println!("  TEMPORAL_NAMESPACE: {}", ns),
        Err(_) => println!("  TEMPORAL_NAMESPACE: ✗ NOT SET"),
    }
    match &api_key {
        Ok(_) => println!("  TEMPORAL_API_KEY:   ✓ (set, hidden)"),
        Err(_) => println!("  TEMPORAL_API_KEY:   (not set - may be required for Temporal Cloud)"),
    }
    println!();

    // Try to create gRPC client
    println!("Attempting to connect via gRPC...");
    let client = match GrpcTemporalClient::from_env().await {
        Ok(c) => c,
        Err(e) => {
            println!("✗ Failed to connect: {}", e);
            println!("\nMake sure TEMPORAL_ADDRESS and TEMPORAL_NAMESPACE are set.");
            println!("For Temporal Cloud, you also need TEMPORAL_API_KEY.");
            std::process::exit(1);
        }
    };

    // Try to count workflows
    match client.count(None).await {
        Ok(count) => {
            println!("\n✓ Connection successful!");
            println!("  Total workflows: {}", count);

            // Try to get counts by status
            println!("\nWorkflow counts by status:");
            for status in WorkflowStatus::all() {
                let query = format!("ExecutionStatus='{}'", status.as_query_value());
                match client.count(Some(&query)).await {
                    Ok(n) if n > 0 => println!("  {:15} {}", format!("{:?}:", status), n),
                    Ok(_) => {}
                    Err(_) => println!("  {:15} (query failed)", format!("{:?}:", status)),
                }
            }
            println!("\n✓ All tests passed! Your gRPC connection is working.");
        }
        Err(e) => {
            println!("\n✗ Connection failed: {}", e);
            println!("\nTroubleshooting tips:");
            println!("  1. Verify your TEMPORAL_ADDRESS is correct");
            println!("     - For Temporal Cloud: <region>.<cloud>.api.temporal.io:7233");
            println!("     - For self-hosted: localhost:7233 (or your server address)");
            println!("  2. Verify your TEMPORAL_NAMESPACE matches your Temporal namespace");
            println!("  3. For Temporal Cloud, ensure TEMPORAL_API_KEY is valid");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Main layout
    let layout = Layout::vertical([
        Constraint::Length(1), // Title
        Constraint::Length(5), // Dashboard
        Constraint::Length(3), // Filter
        Constraint::Fill(1),   // Content (list or detail)
        Constraint::Length(1), // Help bar
    ])
    .split(area);

    // Render title
    render_title(app, frame, layout[0]);

    // Render dashboard
    frame.render_widget(
        StatusDashboard::new(&app.status_counts).selected(app.filter.status),
        layout[1],
    );

    // Render filter input
    let filter_widget = if app.input_mode == InputMode::DateRangeCustom {
        FilterInput::new(&app.date_range_input)
            .active(true)
            .date_mode(true)
    } else if app.view == View::TypeList && app.input_mode == InputMode::FilterInput {
        FilterInput::new(&app.filter_input)
            .active(true)
            .search_mode(true)
    } else {
        FilterInput::new(&app.filter_input)
            .active(app.input_mode == InputMode::FilterInput)
            .date_label(app.active_date_range_label.as_deref())
    };
    frame.render_widget(filter_widget, layout[2]);

    // Render main content based on view
    match app.view {
        View::WorkflowList => {
            let mut table_state = app.table_state.clone();
            frame.render_stateful_widget(
                WorkflowListWidget::new(
                    &app.workflows,
                    &app.filter,
                    &app.visible_columns,
                    &app.workflow_sort,
                )
                .date_label(app.active_date_range_label.as_deref()),
                layout[3],
                &mut table_state,
            );
        }
        View::TypeList => {
            let mut table_state = app.type_table_state.clone();
            frame.render_stateful_widget(
                TypeListWidget::new(&app.type_stats, &app.type_sort)
                    .date_label(app.active_date_range_label.as_deref())
                    .name_filter(app.type_name_filter.as_deref()),
                layout[3],
                &mut table_state,
            );
        }
        View::WorkflowDetail => {
            if let Some(ref detail) = app.selected_workflow {
                frame.render_widget(WorkflowDetailWidget::new(detail), layout[3]);
            }
        }
        View::ActivityList => {
            let mut table_state = app.activity_table_state.clone();
            frame.render_stateful_widget(
                ActivityListWidget::new(&app.activity_events, &app.activities)
                    .expanded(app.expanded_activity),
                layout[3],
                &mut table_state,
            );
        }
        View::Insights => {
            let mut table_state = app.insights_table_state.clone();
            frame.render_stateful_widget(
                InsightsWidget::new(&app.insights),
                layout[3],
                &mut table_state,
            );
        }
        View::InsightDetail => {
            if let Some(finding) = app
                .insights
                .as_ref()
                .and_then(|r| {
                    app.insights_table_state
                        .selected()
                        .and_then(|i| r.findings.get(i))
                })
            {
                frame.render_widget(
                    InsightDetailWidget::new(finding, app.insight_detail_scroll),
                    layout[3],
                );
            }
        }
    }

    // Render help bar
    frame.render_widget(HelpBar::for_view(app.view, app.input_mode), layout[4]);

    // Render error if present
    if let Some(ref error) = app.last_error {
        render_error(error, frame, area);
    }

    // Render help overlay if active
    if app.show_help {
        render_help_overlay(frame, area);
    }
}

fn render_title(app: &App, frame: &mut Frame, area: Rect) {
    let view_name = match app.view {
        View::WorkflowList => "Workflows",
        View::TypeList => "Workflow Types",
        View::WorkflowDetail => "Workflow Detail",
        View::ActivityList => "Activities",
        View::Insights => "Insights",
        View::InsightDetail => "Insight Detail",
    };

    let title = Paragraph::new(Span::styled(
        format!(" Tempurview - {} ", view_name),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    frame.render_widget(title, area);
}

fn render_error(error: &str, frame: &mut Frame, area: Rect) {
    let error_area = Rect {
        x: area.x + 2,
        y: area.height.saturating_sub(4),
        width: area.width.saturating_sub(4),
        height: 3,
    };

    let error_widget = Paragraph::new(format!(" {} ", error))
        .style(Style::default().fg(Color::White).bg(Color::Red))
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(error_widget, error_area);
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    // Center the help overlay
    let help_width = 50.min(area.width.saturating_sub(4));
    let help_height = 30.min(area.height.saturating_sub(4));

    let help_area = Rect {
        x: (area.width.saturating_sub(help_width)) / 2,
        y: (area.height.saturating_sub(help_height)) / 2,
        width: help_width,
        height: help_height,
    };

    frame.render_widget(HelpOverlay, help_area);
}

fn handle_effects(
    effects: Vec<Effect>,
    cli_handle: &CliHandle,
    app: &App,
    default_limit: u32,
    action_tx: &mpsc::UnboundedSender<Action>,
) {
    for effect in effects {
        match effect {
            Effect::LoadCounts => {
                cli_handle.load_counts();
            }
            Effect::LoadWorkflows => {
                let effective_limit = if app.filter.has_date_range() {
                    default_limit.max(1000)
                } else {
                    default_limit
                };
                cli_handle.load_workflows(app.filter.clone(), effective_limit);
            }
            Effect::LoadTypeStats => {
                let effective_limit = if app.filter.has_date_range() {
                    500_u32.max(1000)
                } else {
                    500
                };
                cli_handle.load_type_stats(app.filter.clone(), effective_limit);
            }
            Effect::LoadWorkflowDetail(id) => {
                let run_id = app.selected_workflow_run_id().map(|s| s.to_string());
                cli_handle.load_detail(id, run_id);
            }
            Effect::LoadHistory(workflow_id, run_id) => {
                cli_handle.load_history(workflow_id, run_id);
            }
            Effect::LoadInsights { filter, limit } => {
                cli_handle.load_insights(filter, limit);
            }
            Effect::CancelWorkflow(id) => {
                let run_id = app.selected_workflow_run_id().map(|s| s.to_string());
                cli_handle.cancel_workflow(id, run_id);
            }
            Effect::TerminateWorkflow(id) => {
                let run_id = app.selected_workflow_run_id().map(|s| s.to_string());
                cli_handle.terminate_workflow(id, run_id, "Terminated via TUI".to_string());
            }
            Effect::CopyToClipboard(text) => {
                copy_to_clipboard(&text, action_tx);
            }
            Effect::OpenInBrowser(url) => {
                open_in_browser(&url, action_tx);
            }
        }
    }
}

fn copy_to_clipboard(text: &str, action_tx: &mpsc::UnboundedSender<Action>) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let result = if cfg!(target_os = "macos") {
        Command::new("pbcopy")
            .stdin(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait()
            })
    } else if cfg!(target_os = "linux") {
        Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait()
            })
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Clipboard not supported on this platform",
        ))
    };

    match result {
        Ok(_) => {
            info!("Copied URL to clipboard: {}", text);
            let _ = action_tx.send(Action::Error("URL copied to clipboard".to_string()));
        }
        Err(e) => {
            error!("Failed to copy to clipboard: {}", e);
            let _ = action_tx.send(Action::Error(format!("Failed to copy: {}", e)));
        }
    }
}

fn open_in_browser(url: &str, action_tx: &mpsc::UnboundedSender<Action>) {
    use std::process::Command;

    let result = if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else if cfg!(target_os = "linux") {
        Command::new("xdg-open").arg(url).spawn()
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Browser open not supported on this platform",
        ))
    };

    match result {
        Ok(_) => {
            info!("Opening URL in browser: {}", url);
            let _ = action_tx.send(Action::Error("Opening in browser...".to_string()));
        }
        Err(e) => {
            error!("Failed to open browser: {}", e);
            let _ = action_tx.send(Action::Error(format!("Failed to open browser: {}", e)));
        }
    }
}
