//! LXTUI - Terminal User Interface for LXC/LXD
//!
//! Main entry point for the LXTUI application.

mod app;
mod lxc;
mod lxd_api;
mod ui;

use anyhow::Result;
use app::{
    App, CommandMenu, ConfirmAction, InputCallback, InputMode, StatusModalType, WizardState,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{debug, error, info};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};
use tokio::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger - defaults to OFF to prevent terminal corruption
    // Set RUST_LOG=debug for debugging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();

    info!("Starting LXTUI application");

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let mut app = App::new();
    app.initialize().await;
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        error!("Error: {:?}", err);
        println!("{:?}", err)
    }

    // Handle exec if requested
    if let Some(container_name) = app.exec_container {
        info!("Executing shell in container: {}", container_name);
        // Run lxc exec directly - this will use the current TTY
        let status = std::process::Command::new("lxc")
            .args(["exec", &container_name, "--", "/bin/bash"])
            .status();

        // If bash fails, try sh
        if let Ok(s) = status {
            if !s.success() {
                let _ = std::process::Command::new("lxc")
                    .args(["exec", &container_name, "--", "/bin/sh"])
                    .status();
            }
        }
    }

    info!("LXTUI application terminated");
    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        // Poll for completed background tasks
        app.poll_background_tasks().await;

        // Update operations and maybe auto-refresh
        app.update_operations().await;
        app.maybe_auto_refresh().await;

        terminal.draw(|frame| ui::draw(frame, app))?;

        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                debug!("Key pressed: {:?} in mode: {:?}", key, app.input_mode);

                // Clear message after any key press in normal mode
                if matches!(app.input_mode, InputMode::Normal) && app.message.is_some() {
                    app.clear_message();
                }

                // Track if we need an immediate redraw after handling
                let mut needs_redraw = false;

                match &app.input_mode {
                    InputMode::Normal => handle_normal_mode(app, key).await,
                    InputMode::CommandMenu(menu) => {
                        let menu = menu.clone();
                        handle_command_menu(app, key, menu).await;
                    }
                    InputMode::StatusModal(modal_type) => {
                        let modal_type = modal_type.clone();
                        handle_status_modal(app, key, modal_type).await;
                    }
                    InputMode::Confirmation { action, .. } => {
                        let action = action.clone();
                        // Check if user confirmed the action
                        if matches!(
                            key.code,
                            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y')
                        ) {
                            needs_redraw = true;
                        }
                        handle_confirmation(app, key, action).await;
                    }
                    InputMode::Input {
                        callback_action, ..
                    } => {
                        let callback = callback_action.clone();
                        handle_input(app, key, callback).await;
                    }
                    InputMode::Wizard(state) => {
                        let state = state.clone();
                        handle_wizard(app, key, state).await;
                    }
                }

                // Force immediate redraw if needed
                if needs_redraw {
                    terminal.draw(|frame| ui::draw(frame, app))?;
                }
            }
        }

        if app.should_quit {
            info!("Application quit requested");
            return Ok(());
        }
    }
}

async fn handle_normal_mode(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            // Show container operations menu when Enter is pressed on a container
            if app.get_selected_container().await.is_some() {
                app.show_command_menu(CommandMenu::Container);
            }
        }
        KeyCode::Char(' ') => {
            // Space shows system menu
            app.show_command_menu(CommandMenu::System);
        }
        KeyCode::Char('?') | KeyCode::Char('h') => {
            app.show_help();
        }
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            app.should_quit = true;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.next().await;
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.previous().await;
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        KeyCode::Char('O') | KeyCode::Char('o') => {
            app.show_operation_sidebar = !app.show_operation_sidebar;
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            app.show_info("Refreshing container list...".to_string(), true);
            let _ = app.refresh_containers().await;
        }
        // Quick container actions (direct shortcuts)
        KeyCode::Char('s') => {
            // Quick start
            app.start_selected().await;
        }
        KeyCode::Char('S') => {
            // Quick stop
            app.stop_selected().await;
        }
        KeyCode::Char('d') => {
            // Quick delete
            app.delete_selected().await;
        }
        KeyCode::Char('n') => {
            // Quick new container
            app.start_new_container_wizard();
        }
        _ => {}
    }
}

async fn handle_command_menu(app: &mut App, key: event::KeyEvent, menu: CommandMenu) {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        _ => {
            match menu {
                CommandMenu::Container => handle_container_menu(app, key).await,
                CommandMenu::System => handle_system_menu(app, key).await,
                CommandMenu::Main | CommandMenu::Closed => {
                    // Main menu no longer used, close if somehow reached
                    app.input_mode = InputMode::Normal;
                }
            }
        }
    }
}

// Main menu no longer used - we go directly to Container or System menu

async fn handle_container_menu(app: &mut App, key: event::KeyEvent) {
    const MENU_ITEMS: usize = 7; // Number of menu items

    match key.code {
        // Navigation
        KeyCode::Down | KeyCode::Char('j') => {
            app.menu_next(MENU_ITEMS);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.menu_previous(MENU_ITEMS);
        }
        // Execute selected item
        KeyCode::Enter => {
            match app.menu_selected {
                0 => {
                    // Smart action
                    app.input_mode = InputMode::Normal;
                    if let Some(container) = app.get_selected_container().await {
                        if container.status == "Running" {
                            app.stop_selected().await;
                        } else {
                            app.start_selected().await;
                        }
                    }
                }
                1 => {
                    // Start
                    app.input_mode = InputMode::Normal;
                    app.start_selected().await;
                }
                2 => {
                    // Stop
                    app.input_mode = InputMode::Normal;
                    app.stop_selected().await;
                }
                3 => {
                    // Restart
                    app.input_mode = InputMode::Normal;
                    app.restart_selected().await;
                }
                4 => {
                    // Delete
                    app.input_mode = InputMode::Normal;
                    app.delete_selected().await;
                }
                5 => {
                    // Clone
                    app.input_mode = InputMode::Normal;
                    app.start_clone().await;
                }
                6 => {
                    // Exec shell
                    app.input_mode = InputMode::Normal;
                    if let Some(container) = app.get_selected_container().await {
                        if container.status == "Running" {
                            app.exec_container = Some(container.name.clone());
                            app.should_quit = true;
                            info!("Exec requested for container: {}", container.name);
                        } else {
                            app.show_error(
                                "Container not running".to_string(),
                                format!(
                                    "Container '{}' must be running to exec into it",
                                    container.name
                                ),
                                vec!["Start the container first".to_string()],
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        // Hotkeys (still work as shortcuts)
        KeyCode::Char('s') | KeyCode::Char('1') => {
            app.input_mode = InputMode::Normal;
            app.start_selected().await;
        }
        KeyCode::Char('S') | KeyCode::Char('2') => {
            app.input_mode = InputMode::Normal;
            app.stop_selected().await;
        }
        KeyCode::Char('r') | KeyCode::Char('3') => {
            app.input_mode = InputMode::Normal;
            app.restart_selected().await;
        }
        KeyCode::Char('d') | KeyCode::Char('4') => {
            app.input_mode = InputMode::Normal;
            app.delete_selected().await;
        }
        KeyCode::Char('c') | KeyCode::Char('5') => {
            app.input_mode = InputMode::Normal;
            app.start_clone().await;
        }
        KeyCode::Char('e') | KeyCode::Char('E') => {
            app.input_mode = InputMode::Normal;
            if let Some(container) = app.get_selected_container().await {
                if container.status == "Running" {
                    app.exec_container = Some(container.name.clone());
                    app.should_quit = true;
                    info!("Exec requested for container: {}", container.name);
                } else {
                    app.show_error(
                        "Container not running".to_string(),
                        format!(
                            "Container '{}' must be running to exec into it",
                            container.name
                        ),
                        vec!["Start the container first".to_string()],
                    );
                }
            }
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}

async fn handle_system_menu(app: &mut App, key: event::KeyEvent) {
    const MENU_ITEMS: usize = 6; // Number of menu items (excluding Esc)

    match key.code {
        // Navigation with arrow keys and vim keys
        KeyCode::Down | KeyCode::Char('j') => {
            app.menu_next(MENU_ITEMS);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.menu_previous(MENU_ITEMS);
        }
        // Execute selected action with Enter
        KeyCode::Enter => {
            match app.menu_selected {
                0 => {
                    // Refresh
                    app.input_mode = InputMode::Normal;
                    app.show_info("Refreshing container list...".to_string(), true);
                    let _ = app.refresh_containers().await;
                }
                1 => {
                    // Reload LXD
                    app.input_mode = InputMode::Normal;
                    app.ensure_lxd_and_refresh().await;
                }
                2 => {
                    // New Container
                    app.input_mode = InputMode::Normal;
                    app.start_new_container_wizard();
                }
                3 => {
                    // Toggle Operations
                    app.input_mode = InputMode::Normal;
                    app.show_operation_sidebar = !app.show_operation_sidebar;
                }
                4 => {
                    // Help
                    app.input_mode = InputMode::Normal;
                    app.show_help();
                }
                5 => {
                    // Quit
                    app.should_quit = true;
                }
                _ => {}
            }
        }
        // Direct hotkeys still work
        KeyCode::Char('r') | KeyCode::Char('1') => {
            app.input_mode = InputMode::Normal;
            app.show_info("Refreshing container list...".to_string(), true);
            let _ = app.refresh_containers().await;
        }
        KeyCode::Char('l') | KeyCode::Char('2') => {
            app.input_mode = InputMode::Normal;
            app.ensure_lxd_and_refresh().await;
        }
        KeyCode::Char('n') | KeyCode::Char('3') => {
            app.input_mode = InputMode::Normal;
            app.start_new_container_wizard();
        }
        KeyCode::Char('o') | KeyCode::Char('4') => {
            app.input_mode = InputMode::Normal;
            app.show_operation_sidebar = !app.show_operation_sidebar;
        }
        KeyCode::Char('h') | KeyCode::Char('?') | KeyCode::Char('5') => {
            app.input_mode = InputMode::Normal;
            app.show_help();
        }
        KeyCode::Char('q') | KeyCode::Char('6') => {
            app.should_quit = true;
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}

async fn handle_status_modal(app: &mut App, key: event::KeyEvent, modal_type: StatusModalType) {
    match modal_type {
        StatusModalType::Progress { operation_id } => {
            if key.code == KeyCode::Esc {
                app.lxc_client.cancel_all_operations();
                app.cancel_operation(&operation_id);
                app.input_mode = InputMode::Normal;
            }
        }
        StatusModalType::Success { started_at, .. } => {
            // Auto-close after 2 seconds or on any key
            if started_at.elapsed() > Duration::from_secs(2) {
                app.input_mode = InputMode::Normal;
            } else {
                match key.code {
                    _ => app.input_mode = InputMode::Normal,
                }
            }
        }
        _ => {
            // Close on any key for Info and Error modals
            app.input_mode = InputMode::Normal;
        }
    }
}

async fn handle_confirmation(app: &mut App, key: event::KeyEvent, action: ConfirmAction) {
    match key.code {
        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
            use app::LxdOperationTracker;

            // Immediately show progress modal BEFORE executing the action
            let (operation_desc, container_name, action_str) = match &action {
                ConfirmAction::StartContainer(name) => {
                    (format!("Start container '{}'", name), name.clone(), "start")
                }
                ConfirmAction::StopContainer(name) => {
                    (format!("Stop container '{}'", name), name.clone(), "stop")
                }
                ConfirmAction::RestartContainer(name) => (
                    format!("Restart container '{}'", name),
                    name.clone(),
                    "restart",
                ),
                ConfirmAction::DeleteContainer(name) => (
                    format!("Delete container '{}'", name),
                    name.clone(),
                    "delete",
                ),
            };

            // Register UI operation and show progress modal immediately
            let ui_operation_id =
                app.register_operation(operation_desc.clone(), Some(container_name.clone()));
            app.show_status_modal(StatusModalType::Progress {
                operation_id: ui_operation_id.clone(),
            });

            // Clear pending action since we're executing it
            app.pending_action = None;

            // Mark operation as started
            app.start_operation(&ui_operation_id);

            // Use the new non-blocking LXD operations
            let lxd_operation_result = match action {
                ConfirmAction::StartContainer(_) => {
                    app.lxc_client.start_container_async(&container_name).await
                }
                ConfirmAction::StopContainer(_) => {
                    app.lxc_client.stop_container_async(&container_name).await
                }
                ConfirmAction::RestartContainer(_) => {
                    app.lxc_client
                        .restart_container_async(&container_name)
                        .await
                }
                ConfirmAction::DeleteContainer(_) => {
                    app.lxc_client.delete_container_async(&container_name).await
                }
            };

            match lxd_operation_result {
                Ok(lxd_operation_path) => {
                    info!("LXD operation started: {}", lxd_operation_path);

                    // Track the LXD operation
                    let tracker = LxdOperationTracker {
                        ui_operation_id: ui_operation_id.clone(),
                        lxd_operation_path,
                        description: operation_desc,
                        container_name,
                        action: action_str.to_string(),
                        started_at: Instant::now(),
                        last_checked: Instant::now(),
                        status_code: 103, // Running
                        progress: None,
                    };

                    app.lxd_operations.insert(ui_operation_id, tracker);

                    // The operation will be polled in the main event loop
                }
                Err(e) => {
                    error!("Failed to start LXD operation: {:?}", e);
                    app.complete_operation(&ui_operation_id, false, Some(e.to_string()));
                    app.show_error(
                        format!("Failed to {} '{}'", action_str, container_name),
                        e.to_string(),
                        vec!["Check if LXD is running".to_string()],
                    );
                }
            }
        }
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            app.cancel_dialog();
        }
        _ => {}
    }
}

async fn handle_input(app: &mut App, key: event::KeyEvent, callback: InputCallback) {
    match key.code {
        KeyCode::Enter => {
            if !app.input_buffer.is_empty() {
                match callback {
                    InputCallback::CloneContainer(source) => {
                        let destination = app.input_buffer.clone();
                        app.input_mode = InputMode::Normal;
                        app.clone_container(&source, &destination).await;
                    }
                    InputCallback::CreateContainer => {
                        // This would be handled in wizard flow
                    }
                }
            }
        }
        KeyCode::Esc => {
            app.cancel_input();
        }
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) if c.is_alphanumeric() || c == '-' || c == '_' => {
            app.input_buffer.push(c);
        }
        _ => {}
    }
}

async fn handle_wizard(app: &mut App, key: event::KeyEvent, state: WizardState) {
    match state {
        WizardState::Name => match key.code {
            KeyCode::Tab => {
                if !app.input_buffer.is_empty() {
                    app.wizard_data.name = app.input_buffer.clone();
                    app.input_buffer.clear();
                    app.input_mode = InputMode::Wizard(WizardState::SelectImage);
                }
            }
            KeyCode::Esc => {
                app.cancel_input();
            }
            KeyCode::Backspace => {
                app.input_buffer.pop();
            }
            KeyCode::Char(c) if c.is_alphanumeric() || c == '-' => {
                app.input_buffer.push(c);
            }
            _ => {}
        },
        WizardState::SelectImage => match key.code {
            KeyCode::Up => {
                app.previous_wizard_image();
            }
            KeyCode::Down => {
                app.next_wizard_image();
            }
            KeyCode::Tab => {
                app.input_mode = InputMode::Wizard(WizardState::SelectType);
            }
            KeyCode::BackTab => {
                app.input_buffer = app.wizard_data.name.clone();
                app.input_mode = InputMode::Wizard(WizardState::Name);
            }
            KeyCode::Esc => {
                app.cancel_input();
            }
            _ => {}
        },
        WizardState::SelectType => match key.code {
            KeyCode::Char('c') | KeyCode::Char('C') => {
                app.wizard_data.is_vm = false;
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                app.wizard_data.is_vm = true;
            }
            KeyCode::Tab => {
                app.input_mode = InputMode::Wizard(WizardState::Confirm);
            }
            KeyCode::BackTab => {
                app.input_mode = InputMode::Wizard(WizardState::SelectImage);
            }
            KeyCode::Esc => {
                app.cancel_input();
            }
            _ => {}
        },
        WizardState::Confirm => match key.code {
            KeyCode::Enter => {
                app.create_container().await;
            }
            KeyCode::BackTab => {
                app.input_mode = InputMode::Wizard(WizardState::SelectType);
            }
            KeyCode::Esc => {
                app.cancel_input();
            }
            _ => {}
        },
    }
}
