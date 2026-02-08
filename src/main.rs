use clap::Parser;

use tempurview::action::Action;
use tempurview::app::{App, Effect, InputMode, LoadState, View};
use tempurview::cli::Cli;
use tempurview::cli_worker::{CliHandle, CliRequest, CliWorker};
use tempurview::client::{GrpcTemporalClient, MockTemporalClient, TemporalClient};
use tempurview::commands;
use tempurview::config::Config;
use tempurview::event::{event_to_action, EventHandler};
use tempurview::logging;
use tempurview::output::OutputFormat;
use tempurview::tui::Tui;
use tempurview::app::TimelineItemRef;
use tempurview::widgets::{
    ActivityDetailWidget, ActivityListWidget, EventDetailWidget, EventLogWidget, FilterInput,
    HelpBar, HelpOverlay, InsightDetailWidget, InsightsWidget, StatusDashboard, TypeListWidget,
    WorkflowDetailWidget, WorkflowListWidget,
};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::io::IsTerminal;
use std::sync::Arc;
use std::time::Duration;
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
    let _log_guard = match logging::init() {
        Ok(guard) => Some(guard),
        Err(e) => {
            eprintln!("Warning: Failed to initialize logging: {}", e);
            None
        }
    };

    color_eyre::install()?;

    // Parse CLI args via clap
    let cli = Cli::parse();

    // Handle --logs flag (works regardless of subcommand)
    if cli.global.logs {
        show_logs_info();
        return Ok(());
    }

    // Build config from global args
    let config = Config::from_global_args(&cli.global)?;

    match cli.command {
        // No subcommand → launch TUI (backward compat)
        None => run_tui(config).await,

        Some(tempurview::cli::Commands::Completions { shell }) => {
            use clap::CommandFactory;
            use clap_complete::generate;
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
            Ok(())
        }

        Some(tempurview::cli::Commands::TestConnection) => {
            commands::connection::handle(&config).await
        }

        Some(tempurview::cli::Commands::Config { action }) => {
            let format = OutputFormat::resolve(cli.global.output);
            commands::config_cmd::handle(action, &config, format);
            Ok(())
        }

        Some(cmd) => {
            // All other commands need a client
            let client: Arc<dyn TemporalClient> = create_client(&config).await?;
            let format = OutputFormat::resolve(cli.global.output);

            // Build watch config if --watch is set
            let watch_config = if cli.global.watch {
                Some(tempurview::watch::WatchConfig {
                    interval: Duration::from_secs(cli.global.interval),
                    format,
                    is_tty: std::io::stdout().is_terminal(),
                })
            } else {
                None
            };

            match cmd {
                tempurview::cli::Commands::Workflow { action } => {
                    if let Some(ref wc) = watch_config {
                        tempurview::watch::run_watch_loop(wc, |w| {
                            let action = action.clone();
                            let client = client.clone();
                            let limit = config.default_limit;
                            Box::pin(async move {
                                commands::workflow::handle_to(
                                    action,
                                    client.as_ref(),
                                    format,
                                    limit,
                                    w,
                                )
                                .await
                            })
                        })
                        .await
                    } else {
                        commands::workflow::handle(
                            action,
                            client.as_ref(),
                            format,
                            config.default_limit,
                        )
                        .await
                    }
                }
                tempurview::cli::Commands::Activity { action } => {
                    if let Some(ref wc) = watch_config {
                        tempurview::watch::run_watch_loop(wc, |w| {
                            let action = action.clone();
                            let client = client.clone();
                            Box::pin(async move {
                                commands::activity::handle_to(
                                    action,
                                    client.as_ref(),
                                    format,
                                    w,
                                )
                                .await
                            })
                        })
                        .await
                    } else {
                        commands::activity::handle(action, client.as_ref(), format).await
                    }
                }
                tempurview::cli::Commands::Event { action } => {
                    if let Some(ref wc) = watch_config {
                        tempurview::watch::run_watch_loop(wc, |w| {
                            let action = action.clone();
                            let client = client.clone();
                            Box::pin(async move {
                                commands::event::handle_to(
                                    action,
                                    client.as_ref(),
                                    format,
                                    w,
                                )
                                .await
                            })
                        })
                        .await
                    } else {
                        commands::event::handle(action, client.as_ref(), format).await
                    }
                }
                tempurview::cli::Commands::Insight { action } => {
                    if let Some(ref wc) = watch_config {
                        let config = config.clone();
                        tempurview::watch::run_watch_loop(wc, |w| {
                            let action = action.clone();
                            let client = client.clone();
                            let config = config.clone();
                            Box::pin(async move {
                                commands::insight::handle_to(
                                    action,
                                    client,
                                    format,
                                    config.default_limit,
                                    &config,
                                    w,
                                )
                                .await
                            })
                        })
                        .await
                    } else {
                        commands::insight::handle(
                            action,
                            client.clone(),
                            format,
                            config.default_limit,
                            &config,
                        )
                        .await
                    }
                }
                tempurview::cli::Commands::Serve { port, bind } => {
                    tempurview::web::run_server(client, config, &bind, port).await
                }
                // Already handled above
                tempurview::cli::Commands::TestConnection
                | tempurview::cli::Commands::Config { .. }
                | tempurview::cli::Commands::Completions { .. } => unreachable!(),
            }
        }
    }
}

/// Create a Temporal client from config (shared between TUI and CLI paths).
async fn create_client(config: &Config) -> color_eyre::Result<Arc<dyn TemporalClient>> {
    if config.use_mock {
        info!(
            "Using mock client with {} workflows",
            config.mock_workflow_count
        );
        Ok(Arc::new(MockTemporalClient::with_random_data(
            config.mock_workflow_count,
        )))
    } else {
        let client = GrpcTemporalClient::connect(
            &config.temporal_address,
            config.temporal_namespace.clone(),
            config.temporal_api_key.clone(),
        )
        .await
        .map_err(|e| {
            error!("Failed to create gRPC Temporal client: {}", e);
            color_eyre::eyre::eyre!(
                "Connection failed: {e}\n\nMake sure the following environment variables are set:\n  \
                 TEMPORAL_ADDRESS   - Temporal server address\n  \
                 TEMPORAL_NAMESPACE - Temporal namespace\n  \
                 TEMPORAL_API_KEY   - API key for authentication (required for Temporal Cloud)\n\n\
                 Or use --mock to run with simulated data"
            )
        })?;
        info!("Created gRPC Temporal client");
        Ok(Arc::new(client))
    }
}

/// Run the interactive TUI. This contains all the code that was previously in main().
async fn run_tui(config: Config) -> color_eyre::Result<()> {
    info!("Starting Tempurview TUI");
    debug!(
        "Config: use_mock={}, limit={}",
        config.use_mock, config.default_limit
    );

    // Create client
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
        match GrpcTemporalClient::connect(
            &config.temporal_address,
            config.temporal_namespace.clone(),
            config.temporal_api_key.clone(),
        )
        .await
        {
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
                eprintln!("  4. Try running: tempurview test-connection");
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
    let cli_worker = CliWorker::new(
        client.clone(),
        request_rx,
        action_tx.clone(),
        config.insights.clone(),
    );
    let _worker_handle = cli_worker.spawn();
    let cli_handle = CliHandle::new(request_tx);

    // Initial data load - load both counts and workflows
    cli_handle.load_counts(app.filter.clone());
    app.workflows_load_gen += 1;
    cli_handle.load_workflows(app.filter.clone(), config.default_limit, app.workflows_load_gen);

    // Main loop
    loop {
        // Render
        tui.terminal().draw(|frame| render(&mut app, frame))?;

        // Wait for at least one event or action (blocks until something arrives)
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

        // Drain all remaining pending actions before re-rendering.
        // This batches streaming pages and stale ticks into one render cycle.
        while let Ok(action) = action_rx.try_recv() {
            let effects = app.update(action);
            handle_effects(effects, &cli_handle, &app, config.default_limit, &action_tx);
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn show_logs_info() {
    println!("Tempurview Logs");
    println!("===============\n");

    match logging::logs_dir() {
        Some(dir) => {
            println!("Log directory: {}", dir.display());
            println!();

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

fn render(app: &mut App, frame: &mut Frame) {
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

    // Build search status string for display in filter bar
    let search_status_text;
    let search_status: Option<&str> = if app.search_query.is_some()
        && app.input_mode != InputMode::SearchInput
    {
        let query = app.search_query.as_deref().unwrap_or("");
        if app.is_detail_view() {
            let total = app.search_match_lines.len();
            if total > 0 {
                search_status_text = format!(
                    "Search: \"{}\" ({}/{}) — n/N navigate, Esc clear",
                    query,
                    app.search_current_match + 1,
                    total
                );
            } else {
                search_status_text = format!("Search: \"{}\" (no matches) — Esc clear", query);
            }
        } else {
            let total = app.search_filtered_indices.len();
            if total > 0 {
                search_status_text = format!(
                    "Search: \"{}\" ({} matches) — n/N navigate, Esc clear",
                    query, total
                );
            } else {
                search_status_text = format!("Search: \"{}\" (no matches) — Esc clear", query);
            }
        }
        Some(&search_status_text)
    } else {
        None
    };

    // Render filter input
    let filter_widget = if app.input_mode == InputMode::SearchInput {
        let live_match_count = if !app.is_detail_view() && !app.search_input.is_empty() {
            Some(app.search_filtered_indices.len())
        } else {
            None
        };
        FilterInput::new(&app.search_input)
            .active(true)
            .search_mode(true)
            .match_count(live_match_count)
    } else if app.input_mode == InputMode::DateRangeCustom {
        FilterInput::new(&app.date_range_input)
            .active(true)
            .date_mode(true)
    } else if app.input_mode == InputMode::FilterInput {
        FilterInput::new(&app.filter_input)
            .active(true)
    } else {
        FilterInput::new(&app.filter_input)
            .active(false)
            .date_label(app.active_date_range_label.as_deref())
            .search_status(search_status)
    };
    frame.render_widget(filter_widget, layout[2]);

    // Render main content based on view
    match app.view {
        View::WorkflowList => {
            let filtered = if app.search_query.is_some() && !app.search_filtered_indices.is_empty()
            {
                Some(app.search_filtered_indices.as_slice())
            } else {
                None
            };
            let total_count = if let LoadState::Loaded(ref counts) = app.status_counts {
                Some(counts.total())
            } else {
                None
            };
            frame.render_stateful_widget(
                WorkflowListWidget::new(
                    &app.workflows,
                    &app.filter,
                    &app.visible_columns,
                    &app.workflow_sort,
                )
                .date_label(app.active_date_range_label.as_deref())
                .filtered_indices(filtered)
                .loading(app.workflows_loading)
                .total_count(total_count),
                layout[3],
                &mut app.table_state,
            );
        }
        View::TypeList => {
            let filtered = if app.search_query.is_some() && !app.search_filtered_indices.is_empty()
            {
                Some(app.search_filtered_indices.as_slice())
            } else {
                None
            };
            frame.render_stateful_widget(
                TypeListWidget::new(&app.type_stats, &app.type_sort)
                    .date_label(app.active_date_range_label.as_deref())
                    .name_filter(app.type_name_filter.as_deref())
                    .filtered_indices(filtered),
                layout[3],
                &mut app.type_table_state,
            );
        }
        View::WorkflowDetail => {
            if let Some(ref detail) = app.selected_workflow {
                frame.render_widget(
                    WorkflowDetailWidget::new(detail).search(
                        app.search_query.as_deref(),
                        app.search_current_match,
                        app.search_match_lines.len(),
                    ),
                    layout[3],
                );
            }
        }
        View::ActivityList => {
            let filtered = if app.search_query.is_some() && !app.search_filtered_indices.is_empty()
            {
                Some(app.search_filtered_indices.as_slice())
            } else {
                None
            };
            let expanded = app.activity_table_state.selected();
            frame.render_stateful_widget(
                ActivityListWidget::new(
                    &app.activity_events,
                    &app.activities,
                    &app.child_workflows,
                )
                .expanded(expanded)
                .filtered_indices(filtered),
                layout[3],
                &mut app.activity_table_state,
            );
        }
        View::ActivityDetail => {
            if let Some(item) = app.selected_timeline_item() {
                let widget = match item {
                    TimelineItemRef::Activity(a) => {
                        ActivityDetailWidget::from_activity(a, app.activity_detail_scroll)
                    }
                    TimelineItemRef::ChildWorkflow(cw) => {
                        ActivityDetailWidget::from_child_workflow(cw, app.activity_detail_scroll)
                    }
                };
                frame.render_widget(
                    widget.search(
                        app.search_query.as_deref(),
                        app.search_current_match,
                        app.search_match_lines.len(),
                    ),
                    layout[3],
                );
            }
        }
        View::EventLog => {
            let filtered = if app.search_query.is_some() && !app.search_filtered_indices.is_empty()
            {
                Some(app.search_filtered_indices.as_slice())
            } else {
                None
            };
            frame.render_stateful_widget(
                EventLogWidget::new(&app.activity_events).filtered_indices(filtered),
                layout[3],
                &mut app.event_log_table_state,
            );
        }
        View::EventDetail => {
            if let Some(event) = app.selected_event() {
                frame.render_widget(
                    EventDetailWidget::new(event, app.event_detail_scroll).search(
                        app.search_query.as_deref(),
                        app.search_current_match,
                        app.search_match_lines.len(),
                    ),
                    layout[3],
                );
            }
        }
        View::Insights => {
            let filtered = if app.search_query.is_some() && !app.search_filtered_indices.is_empty()
            {
                Some(app.search_filtered_indices.as_slice())
            } else {
                None
            };
            frame.render_stateful_widget(
                InsightsWidget::new(&app.insights)
                    .filtered_indices(filtered)
                    .progress(app.insights_progress.as_ref())
                    .scanning(app.insights_scanning)
                    .date_label(app.active_date_range_label.as_deref()),
                layout[3],
                &mut app.insights_table_state,
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
                let sel_entity = if finding.affected_entities.is_empty() {
                    None
                } else {
                    Some(app.insight_entity_index)
                };
                frame.render_widget(
                    InsightDetailWidget::new(finding, app.insight_detail_scroll)
                        .selected_entity(sel_entity)
                        .search(
                            app.search_query.as_deref(),
                            app.search_current_match,
                            app.search_match_lines.len(),
                        ),
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
        View::ActivityDetail => "Activity Detail",
        View::EventLog => "Event Log",
        View::EventDetail => "Event Detail",
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
            Effect::LoadCounts { filter } => {
                cli_handle.load_counts(filter);
            }
            Effect::LoadWorkflows { gen } => {
                cli_handle.load_workflows(app.filter.clone(), default_limit, gen);
            }
            Effect::LoadTypeStats => {
                cli_handle.load_type_stats(app.filter.clone(), default_limit);
            }
            Effect::LoadWorkflowDetail(id, run_id) => {
                cli_handle.load_detail(id, run_id);
            }
            Effect::LoadHistory(workflow_id, run_id) => {
                cli_handle.load_history(workflow_id, run_id);
            }
            Effect::LoadInsights { filter, limit, gen } => {
                cli_handle.load_insights(filter, limit, gen);
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
