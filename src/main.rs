use tempurview::action::{Action, DataPayload};
use tempurview::app::{App, Effect, InputMode, View};
use tempurview::client::{CliTemporalClient, MockTemporalClient, TemporalClient};
use tempurview::config::Config;
use tempurview::domain::{StatusCounts, WorkflowFilter, WorkflowStatus};
use tempurview::event::{event_to_action, EventHandler};
use tempurview::tui::Tui;
use tempurview::widgets::{FilterInput, HelpBar, HelpOverlay, StatusDashboard, WorkflowDetailWidget, WorkflowListWidget};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    // Load .env file if present (silently ignore if not found)
    let _ = dotenvy::dotenv();

    // Parse configuration
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Check for help flag
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    // Check for test-connection flag
    if args.iter().any(|a| a == "--test-connection") {
        return test_connection().await;
    }

    let config = Config::from_args(&args)?;

    // Create client
    let client: Arc<dyn TemporalClient> = if config.use_mock {
        Arc::new(MockTemporalClient::with_random_data(config.mock_workflow_count))
    } else {
        match CliTemporalClient::from_env() {
            Ok(c) => Arc::new(c),
            Err(e) => {
                eprintln!("Error: {}", e);
                eprintln!("\nMake sure the following environment variables are set:");
                eprintln!("  TEMPORAL_ADDRESS   - Temporal server address (e.g., localhost:7233)");
                eprintln!("  TEMPORAL_NAMESPACE - Temporal namespace");
                eprintln!("  TEMPORAL_API_KEY   - (optional) API key for authentication");
                eprintln!("\nOr use --mock to run with simulated data");
                return Ok(());
            }
        }
    };

    // Initialize terminal
    let mut tui = Tui::new()?;

    // Create app state
    let mut app = App::new();

    // Create event handler
    let mut events = EventHandler::new(config.tick_rate);

    // Create action channel for async data loading
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

    // Initial data load
    spawn_load_counts(client.clone(), action_tx.clone());
    spawn_load_workflows(
        client.clone(),
        action_tx.clone(),
        app.filter.clone(),
        config.default_limit,
    );

    // Main loop
    loop {
        // Render
        tui.terminal().draw(|frame| render(&app, frame))?;

        // Handle events
        tokio::select! {
            Some(event) = events.next() => {
                if let Some(action) = event_to_action(event, app.view, app.input_mode) {
                    let effects = app.update(action);
                    handle_effects(
                        effects,
                        client.clone(),
                        action_tx.clone(),
                        &app,
                        config.default_limit,
                    );
                }
            }
            Some(action) = action_rx.recv() => {
                let effects = app.update(action);
                handle_effects(
                    effects,
                    client.clone(),
                    action_tx.clone(),
                    &app,
                    config.default_limit,
                );
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn print_help() {
    println!("Tempurview - A terminal interface for Temporal workflows");
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
    println!("    -h, --help          Show this help message");
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

async fn test_connection() -> color_eyre::Result<()> {
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

    // Try to create client
    let client = match CliTemporalClient::from_env() {
        Ok(c) => c,
        Err(e) => {
            println!("✗ Failed to create client: {}", e);
            println!("\nMake sure TEMPORAL_ADDRESS and TEMPORAL_NAMESPACE are set.");
            println!("For Temporal Cloud, you also need TEMPORAL_API_KEY.");
            std::process::exit(1);
        }
    };

    // Try to count workflows (simplest operation)
    println!("Attempting to connect...");
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
            println!("\n✓ All tests passed! Your connection is working.");
        }
        Err(e) => {
            println!("\n✗ Connection failed: {}", e);
            println!("\nTroubleshooting tips:");
            println!("  1. Verify your TEMPORAL_ADDRESS is correct");
            println!("     - For Temporal Cloud: <namespace>.<accountId>.tmprl.cloud:7233");
            println!("     - For self-hosted: localhost:7233 (or your server address)");
            println!("  2. Verify your TEMPORAL_NAMESPACE matches your Temporal namespace");
            println!("  3. For Temporal Cloud, ensure TEMPORAL_API_KEY is valid");
            println!("  4. Check that the 'temporal' CLI is installed and in your PATH");
            println!("  5. Try running: temporal workflow list --address $TEMPORAL_ADDRESS --namespace $TEMPORAL_NAMESPACE");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Main layout
    let layout = Layout::vertical([
        Constraint::Length(1),  // Title
        Constraint::Length(5),  // Dashboard
        Constraint::Length(3),  // Filter
        Constraint::Fill(1),    // Content (list or detail)
        Constraint::Length(1),  // Help bar
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
    frame.render_widget(
        FilterInput::new(&app.filter_input).active(app.input_mode == InputMode::FilterInput),
        layout[2],
    );

    // Render main content based on view
    match app.view {
        View::Dashboard | View::WorkflowList => {
            let mut list_state = app.list_state.clone();
            frame.render_stateful_widget(
                WorkflowListWidget::new(&app.workflows, &app.filter),
                layout[3],
                &mut list_state,
            );
        }
        View::WorkflowDetail => {
            if let Some(ref detail) = app.selected_workflow {
                frame.render_widget(WorkflowDetailWidget::new(detail), layout[3]);
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
        View::Dashboard => "Dashboard",
        View::WorkflowList => "Workflow List",
        View::WorkflowDetail => "Workflow Detail",
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
        y: area.height.saturating_sub(3),
        width: area.width.saturating_sub(4),
        height: 2,
    };

    let error_widget = Paragraph::new(format!(" Error: {} ", error))
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
    client: Arc<dyn TemporalClient>,
    tx: mpsc::UnboundedSender<Action>,
    app: &App,
    default_limit: u32,
) {
    for effect in effects {
        match effect {
            Effect::LoadCounts => spawn_load_counts(client.clone(), tx.clone()),
            Effect::LoadWorkflows => {
                spawn_load_workflows(client.clone(), tx.clone(), app.filter.clone(), default_limit)
            }
            Effect::LoadWorkflowDetail(id) => {
                let run_id = app.selected_workflow_run_id().map(|s| s.to_string());
                spawn_load_detail(client.clone(), tx.clone(), id, run_id);
            }
            Effect::CancelWorkflow(id) => {
                let run_id = app.selected_workflow_run_id().map(|s| s.to_string());
                spawn_cancel_workflow(client.clone(), tx.clone(), id, run_id);
            }
            Effect::TerminateWorkflow(id) => {
                let run_id = app.selected_workflow_run_id().map(|s| s.to_string());
                spawn_terminate_workflow(client.clone(), tx.clone(), id, run_id);
            }
        }
    }
}

fn spawn_load_counts(client: Arc<dyn TemporalClient>, tx: mpsc::UnboundedSender<Action>) {
    tokio::spawn(async move {
        let mut counts = StatusCounts::new();

        for status in WorkflowStatus::all() {
            let query = format!("ExecutionStatus='{}'", status.as_query_value());
            match client.count(Some(&query)).await {
                Ok(n) => counts.set(*status, n),
                Err(e) => {
                    let _ = tx.send(Action::Error(e.to_string()));
                    return;
                }
            }
        }

        let _ = tx.send(Action::DataLoaded(DataPayload::Counts(counts)));
    });
}

fn spawn_load_workflows(
    client: Arc<dyn TemporalClient>,
    tx: mpsc::UnboundedSender<Action>,
    filter: WorkflowFilter,
    limit: u32,
) {
    tokio::spawn(async move {
        match client.list(&filter, limit).await {
            Ok(workflows) => {
                let _ = tx.send(Action::DataLoaded(DataPayload::Workflows(workflows)));
            }
            Err(e) => {
                let _ = tx.send(Action::Error(e.to_string()));
            }
        }
    });
}

fn spawn_load_detail(
    client: Arc<dyn TemporalClient>,
    tx: mpsc::UnboundedSender<Action>,
    workflow_id: String,
    run_id: Option<String>,
) {
    tokio::spawn(async move {
        match client.describe(&workflow_id, run_id.as_deref()).await {
            Ok(detail) => {
                let _ = tx.send(Action::DataLoaded(DataPayload::Detail(Box::new(detail))));
            }
            Err(e) => {
                let _ = tx.send(Action::Error(e.to_string()));
            }
        }
    });
}

fn spawn_cancel_workflow(
    client: Arc<dyn TemporalClient>,
    tx: mpsc::UnboundedSender<Action>,
    workflow_id: String,
    run_id: Option<String>,
) {
    tokio::spawn(async move {
        match client.cancel(&workflow_id, run_id.as_deref()).await {
            Ok(()) => {
                // Refresh after cancel
                let _ = tx.send(Action::Refresh);
            }
            Err(e) => {
                let _ = tx.send(Action::Error(e.to_string()));
            }
        }
    });
}

fn spawn_terminate_workflow(
    client: Arc<dyn TemporalClient>,
    tx: mpsc::UnboundedSender<Action>,
    workflow_id: String,
    run_id: Option<String>,
) {
    tokio::spawn(async move {
        match client
            .terminate(&workflow_id, run_id.as_deref(), "Terminated via TUI")
            .await
        {
            Ok(()) => {
                // Refresh after terminate
                let _ = tx.send(Action::Refresh);
            }
            Err(e) => {
                let _ = tx.send(Action::Error(e.to_string()));
            }
        }
    });
}
