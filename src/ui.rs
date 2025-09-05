//! Terminal UI rendering
//!
//! This module handles all UI rendering using Ratatui, including
//! the main container list, modals, menus, and status displays.

use crate::app::{
    App, CommandMenu, ConfirmAction, InputCallback, InputMode, InputType, StatusModalType,
    WizardState,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn draw(frame: &mut Frame, app: &App) {
    // Main layout - simplified to 3 panels
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(3), // Title & Status Bar
            Constraint::Min(10),   // Container List (main focus)
            Constraint::Length(2), // Command hints
        ])
        .split(frame.area());

    // Draw main UI components
    draw_title_and_status(frame, chunks[0], app);

    // Check if we need to show operation sidebar
    if app.show_operation_sidebar {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(40),
                Constraint::Length(30), // Sidebar width
            ])
            .split(chunks[1]);

        draw_container_list(frame, main_chunks[0], app);
        draw_operation_sidebar(frame, main_chunks[1], app);
    } else {
        draw_container_list(frame, chunks[1], app);
    }

    draw_command_hints(frame, chunks[2], app);

    // Draw modals and overlays based on input mode
    match &app.input_mode {
        InputMode::CommandMenu(menu) => {
            draw_command_menu(frame, menu, app.menu_selected);
        }
        InputMode::StatusModal(modal_type) => {
            draw_status_modal(frame, modal_type, app);
        }
        InputMode::Confirmation { message, action } => {
            draw_confirmation_modal(frame, message, action);
        }
        InputMode::Input {
            prompt,
            input_type,
            callback_action,
        } => {
            draw_input_modal(
                frame,
                prompt,
                &app.input_buffer,
                input_type,
                callback_action,
            );
        }
        InputMode::Wizard(state) => {
            draw_wizard(frame, state, app);
        }
        InputMode::Normal => {}
    }
}

fn draw_title_and_status(frame: &mut Frame, area: Rect, app: &App) {
    let container_count = app.containers.try_read().map(|c| c.len()).unwrap_or(0);
    let lxd_status = if app.lxd_status {
        "Running"
    } else {
        "Not Running"
    };
    let _lxd_color = if app.lxd_status {
        Color::Green
    } else {
        Color::Red
    };

    let status_text = if app.active_operation_count > 0 {
        format!("⚡ {} operations active", app.active_operation_count)
    } else {
        "⚡ Ready".to_string()
    };

    let title_text = format!(
        " LXTUI │ {} containers │ LXD: {} │ {} ",
        container_count, lxd_status, status_text
    );

    let title = Paragraph::new(title_text)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White))
                .border_type(BorderType::Rounded),
        )
        .alignment(Alignment::Center);

    frame.render_widget(title, area);
}

fn draw_container_list(frame: &mut Frame, area: Rect, app: &App) {
    let containers = if let Ok(containers) = app.containers.try_read() {
        containers.clone()
    } else {
        Vec::new()
    };

    if containers.is_empty() {
        let empty_msg = Paragraph::new("No containers found. Press Space for commands.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White))
                    .border_type(BorderType::Rounded)
                    .title(" Containers "),
            );

        frame.render_widget(empty_msg, area);
        return;
    }

    let containers_list: Vec<ListItem> = containers
        .iter()
        .enumerate()
        .map(|(i, container)| {
            let status_color = match container.status.as_str() {
                "Running" => Color::Green,
                "Stopped" => Color::Red,
                _ => Color::Yellow,
            };

            let status_style = Style::default().fg(status_color);

            let ip = container
                .ipv4
                .first()
                .cloned()
                .unwrap_or_else(|| "-".to_string());

            let content = vec![Line::from(vec![
                Span::raw(format!("{:20} ", container.name)),
                Span::styled(format!("{:10} ", container.status), status_style),
                Span::raw(format!("{:15} ", ip)),
                Span::raw(&container.container_type),
            ])];

            if i == app.selected {
                ListItem::new(content).style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ListItem::new(content)
            }
        })
        .collect();

    let header = Line::from(vec![
        Span::styled(
            "Name                 ",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        ),
        Span::styled(
            "Status     ",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        ),
        Span::styled(
            "IPv4            ",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        ),
        Span::styled(
            "Type",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        ),
    ]);

    let containers_widget = List::new(containers_list)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White))
                .border_type(BorderType::Rounded)
                .title(" Containers "),
        )
        .style(Style::default().fg(Color::White));

    // Render header separately
    let inner = area.inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });
    frame.render_widget(Paragraph::new(header), inner);

    // Render list below header
    let list_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };

    frame.render_widget(containers_widget, list_area);
}

fn draw_command_hints(frame: &mut Frame, area: Rect, app: &App) {
    let hints = match &app.input_mode {
        InputMode::Normal => {
            vec![Line::from(vec![
                Span::styled("[Enter] ", Style::default().fg(Color::Green)),
                Span::raw("Actions  "),
                Span::styled("[Space] ", Style::default().fg(Color::Yellow)),
                Span::raw("System  "),
                Span::styled("[j/k ↑/↓] ", Style::default().fg(Color::Yellow)),
                Span::raw("Navigate  "),
                Span::styled("[s/S] ", Style::default().fg(Color::Yellow)),
                Span::raw("Start/Stop  "),
                Span::styled("[n] ", Style::default().fg(Color::Yellow)),
                Span::raw("New  "),
                Span::styled("[?] ", Style::default().fg(Color::Cyan)),
                Span::raw("Help  "),
                Span::styled("[q] ", Style::default().fg(Color::Red)),
                Span::raw("Quit"),
            ])]
        }
        InputMode::CommandMenu(_) => {
            vec![Line::from(vec![
                Span::styled("[↑/↓] ", Style::default().fg(Color::Yellow)),
                Span::raw("Navigate  "),
                Span::styled("[Enter] ", Style::default().fg(Color::Green)),
                Span::raw("Select  "),
                Span::styled("[Esc] ", Style::default().fg(Color::Red)),
                Span::raw("Back"),
            ])]
        }
        InputMode::Confirmation { .. } => {
            vec![Line::from(vec![
                Span::styled("[Enter/Y] ", Style::default().fg(Color::Green)),
                Span::raw("Confirm  "),
                Span::styled("[Esc/N] ", Style::default().fg(Color::Red)),
                Span::raw("Cancel"),
            ])]
        }
        InputMode::Input { .. } => {
            vec![Line::from(vec![
                Span::styled("[Enter] ", Style::default().fg(Color::Green)),
                Span::raw("Submit  "),
                Span::styled("[Esc] ", Style::default().fg(Color::Red)),
                Span::raw("Cancel"),
            ])]
        }
        InputMode::StatusModal(modal_type) => match modal_type {
            StatusModalType::Progress { .. } => {
                vec![Line::from(vec![
                    Span::styled("[Esc] ", Style::default().fg(Color::Red)),
                    Span::raw("Cancel Operation"),
                ])]
            }
            _ => {
                vec![Line::from(vec![
                    Span::styled("[Any Key] ", Style::default().fg(Color::Yellow)),
                    Span::raw("Close"),
                ])]
            }
        },
        InputMode::Wizard(_) => {
            vec![Line::from(vec![
                Span::styled("[Tab] ", Style::default().fg(Color::Yellow)),
                Span::raw("Next  "),
                Span::styled("[Shift+Tab] ", Style::default().fg(Color::Yellow)),
                Span::raw("Previous  "),
                Span::styled("[Enter] ", Style::default().fg(Color::Green)),
                Span::raw("Confirm  "),
                Span::styled("[Esc] ", Style::default().fg(Color::Red)),
                Span::raw("Cancel"),
            ])]
        }
    };

    let hints_widget = Paragraph::new(hints)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .alignment(Alignment::Center);

    frame.render_widget(hints_widget, area);
}

fn draw_operation_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let mut content = Vec::new();

    // Active operations
    if app.active_operation_count > 0 {
        content.push(Line::from(vec![Span::styled(
            "Active Operations",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]));
        content.push(Line::from(""));
    }

    // Recent operations
    let recent_ops: Vec<_> = app.user_operations.iter().rev().take(10).collect();
    if !recent_ops.is_empty() {
        for op in recent_ops {
            let status_icon = match &op.status {
                crate::app::OperationStatus::Registered => "⏳",
                crate::app::OperationStatus::Running => "🚀",
                crate::app::OperationStatus::Retrying(_) => "🔄",
                crate::app::OperationStatus::Success => "✅",
                crate::app::OperationStatus::Failed(_) => "❌",
                crate::app::OperationStatus::Cancelled => "🚫",
            };

            let duration = if let Some(started) = op.started_at {
                if let Some(completed) = op.completed_at {
                    format!(" ({}s)", (completed - started).as_secs())
                } else {
                    format!(" ({}s)", started.elapsed().as_secs())
                }
            } else {
                String::new()
            };

            let line = match &op.status {
                crate::app::OperationStatus::Failed(err) if !err.is_empty() => {
                    format!("{} {}{}", status_icon, op.description, duration)
                }
                crate::app::OperationStatus::Retrying(_) => {
                    format!(
                        "{} {} (retry {})",
                        status_icon, op.description, op.retry_count
                    )
                }
                _ => format!("{} {}{}", status_icon, op.description, duration),
            };

            content.push(Line::from(line));
        }
    } else {
        content.push(Line::from("No operations yet"));
    }

    let sidebar = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::LEFT)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Operations "),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(sidebar, area);
}

fn centered_rect(width_percent: u16, height_percent: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_command_menu(frame: &mut Frame, menu: &CommandMenu, selected: usize) {
    let area = centered_rect(60, 40, frame.area());
    frame.render_widget(Clear, area);

    let (title, items) = match menu {
        CommandMenu::Closed | CommandMenu::Main => return,
        CommandMenu::Container => (
            " Container Actions ",
            vec![
                (
                    "Enter/s",
                    "Smart Action",
                    "Start if stopped, Stop if running",
                ),
                ("1", "Start Container", "Start the selected container"),
                ("2", "Stop Container", "Stop the selected container"),
                ("3", "Restart Container", "Restart the selected container"),
                ("4", "Delete Container", "Delete the selected container"),
                ("5", "Clone Container", "Create a copy of the container"),
                ("e", "Exec Shell", "Open shell in running container"),
                ("Esc", "Cancel", "Return to container list"),
            ],
        ),
        CommandMenu::System => (
            " System Menu ",
            vec![
                ("1/r", "Refresh List", "Reload container list"),
                ("2/l", "Check LXD Service", "Ensure LXD service is running"),
                ("3/n", "New Container", "Create a new container"),
                ("4/o", "Toggle Operations", "Show/hide operations sidebar"),
                ("5/h", "Help", "Show keyboard shortcuts"),
                ("6/q", "Quit", "Exit LXTUI"),
                ("Esc", "Cancel", "Return to container list"),
            ],
        ),
    };

    let mut content = vec![Line::from("")];

    // Skip the "Esc" option when counting (it's always last)
    let selectable_items = items.len() - 1;

    for (idx, (key, label, desc)) in items.iter().enumerate() {
        // Don't highlight Esc option
        let is_selected = idx < selectable_items && idx == selected;

        if is_selected {
            // Highlighted selection with arrow indicator
            content.push(Line::from(vec![
                Span::styled(
                    " ▶ ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("[{}] ", key),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<20}", label),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc.to_string(), Style::default().fg(Color::White)),
            ]));
        } else {
            // Normal item
            content.push(Line::from(vec![
                Span::styled("   ", Style::default()), // Space for arrow
                Span::styled(
                    format!("[{}] ", key),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:<20}", label), Style::default().fg(Color::White)),
                Span::styled(desc.to_string(), Style::default().fg(Color::DarkGray)),
            ]));
        }
        content.push(Line::from(""));
    }

    // Add navigation hint at the bottom
    content.push(Line::from(vec![
        Span::styled(" Use ", Style::default().fg(Color::DarkGray)),
        Span::styled("↑/↓ or j/k", Style::default().fg(Color::Cyan)),
        Span::styled(" to navigate, ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::styled(" to select", Style::default().fg(Color::DarkGray)),
    ]));

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .border_type(BorderType::Rounded);

    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_status_modal(frame: &mut Frame, modal_type: &StatusModalType, app: &App) {
    let area = centered_rect(70, 50, frame.area());
    frame.render_widget(Clear, area);

    match modal_type {
        StatusModalType::Info {
            message,
            auto_close,
        } => {
            draw_info_modal(frame, area, message, *auto_close);
        }
        StatusModalType::Progress { operation_id } => {
            if let Some(operation) = app.user_operations.iter().find(|op| op.id == *operation_id) {
                draw_progress_modal(frame, area, operation);
            }
        }
        StatusModalType::Error {
            title,
            details,
            suggestions,
        } => {
            draw_error_modal(frame, area, title, details, suggestions);
        }
        StatusModalType::Success {
            message,
            started_at,
        } => {
            draw_success_modal(frame, area, message, started_at);
        }
    }
}

fn draw_info_modal(frame: &mut Frame, area: Rect, message: &str, auto_close: bool) {
    let block = Block::default()
        .title(" Information ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .border_type(BorderType::Rounded);

    let mut lines: Vec<Line> = vec![Line::from("")];
    for line in message.lines() {
        lines.push(Line::from(line));
    }
    lines.push(Line::from(""));

    if !auto_close {
        lines.push(Line::from(vec![Span::styled(
            "Press any key to continue",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_progress_modal(frame: &mut Frame, area: Rect, operation: &crate::app::UserOperation) {
    let elapsed_secs = if let Some(started) = operation.started_at {
        started.elapsed().as_secs()
    } else {
        0
    };

    let spinner = match elapsed_secs % 4 {
        0 => "⠋",
        1 => "⠙",
        2 => "⠹",
        _ => "⠸",
    };

    let status_line = match &operation.status {
        crate::app::OperationStatus::Registered => format!("⏳ Preparing..."),
        crate::app::OperationStatus::Running => format!("{} In Progress...", spinner),
        crate::app::OperationStatus::Retrying(count) => {
            format!("🔄 Retrying... (attempt {}/3)", count)
        }
        _ => format!("Processing..."),
    };

    let block = Block::default()
        .title(" Operation Progress ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .border_type(BorderType::Rounded);

    let content = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            &operation.description,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            status_line,
            Style::default().fg(Color::Cyan),
        )]),
        Line::from(""),
        Line::from(format!("Elapsed: {} seconds", elapsed_secs)),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to cancel", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(content)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_error_modal(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    details: &str,
    suggestions: &[String],
) {
    let block = Block::default()
        .title(format!(" ❌ {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .border_type(BorderType::Rounded);

    let mut content = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Error Details:",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
    ];

    // Add error details
    for line in details.lines() {
        content.push(Line::from(vec![Span::styled(
            line,
            Style::default().fg(Color::White),
        )]));
    }

    if !suggestions.is_empty() {
        content.push(Line::from(""));
        content.push(Line::from(vec![Span::styled(
            "Suggestions:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]));
        content.push(Line::from(""));

        for suggestion in suggestions {
            content.push(Line::from(vec![
                Span::styled("• ", Style::default().fg(Color::Yellow)),
                Span::raw(suggestion),
            ]));
        }
    }

    content.push(Line::from(""));
    content.push(Line::from(vec![Span::styled(
        "Press any key to continue",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )]));

    let paragraph = Paragraph::new(content)
        .block(block)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_success_modal(
    frame: &mut Frame,
    area: Rect,
    message: &str,
    _started_at: &tokio::time::Instant,
) {
    let block = Block::default()
        .title(" ✅ Success ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(BorderType::Rounded);

    let content = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            message,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "(Press any key to continue)",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]),
    ];

    let paragraph = Paragraph::new(content)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_confirmation_modal(frame: &mut Frame, message: &str, action: &ConfirmAction) {
    let area = centered_rect(60, 30, frame.area());
    frame.render_widget(Clear, area);

    let title = match action {
        ConfirmAction::StartContainer(_) => " Start Container ",
        ConfirmAction::StopContainer(_) => " Stop Container ",
        ConfirmAction::RestartContainer(_) => " Restart Container ",
        ConfirmAction::DeleteContainer(_) => " ⚠️  Delete Container ",
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .border_type(BorderType::Rounded);

    let content = vec![
        Line::from(""),
        Line::from(message),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::White)),
            Span::styled(
                "Enter/Y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to confirm or ", Style::default().fg(Color::White)),
            Span::styled(
                "Esc/N",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to cancel", Style::default().fg(Color::White)),
        ]),
    ];

    let paragraph = Paragraph::new(content)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_input_modal(
    frame: &mut Frame,
    prompt: &str,
    input: &str,
    input_type: &InputType,
    callback: &InputCallback,
) {
    let area = centered_rect(60, 20, frame.area());
    frame.render_widget(Clear, area);

    let title = match callback {
        InputCallback::CloneContainer(_) => " Clone Container ",
        InputCallback::CreateContainer => " New Container ",
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .border_type(BorderType::Rounded);

    let hint = match input_type {
        InputType::ContainerName => "Container names must be alphanumeric with dashes allowed",
        InputType::ImageName => "Enter image name (e.g., ubuntu:22.04)",
    };

    let content = vec![
        Line::from(""),
        Line::from(prompt),
        Line::from(""),
        Line::from(format!("{}_", input)),
        Line::from(""),
        Line::from(vec![Span::styled(
            hint,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]),
    ];

    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_wizard(frame: &mut Frame, state: &WizardState, app: &App) {
    let area = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, area);

    match state {
        WizardState::Name => draw_wizard_name(frame, area, &app.input_buffer),
        WizardState::SelectImage => draw_wizard_image(frame, area, app),
        WizardState::SelectType => draw_wizard_type(frame, area, app),
        WizardState::Confirm => draw_wizard_confirm(frame, area, app),
    }
}

fn draw_wizard_name(frame: &mut Frame, area: Rect, input: &str) {
    let block = Block::default()
        .title(" New Container - Step 1: Name ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(BorderType::Rounded);

    let text = vec![
        Line::from("Enter a name for your new container:"),
        Line::from(""),
        Line::from(format!("Name: {}_", input)),
        Line::from(""),
        Line::from("Container names must be alphanumeric with dashes allowed."),
    ];

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_wizard_image(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" New Container - Step 2: Select Image ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(BorderType::Rounded);

    let items: Vec<ListItem> = app
        .available_images
        .iter()
        .enumerate()
        .map(|(i, image)| {
            let content = format!("{} - {}", image.alias, image.description);
            if i == app.wizard_data.selected_image_index {
                ListItem::new(content).style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ListItem::new(content)
            }
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(list, area);
}

fn draw_wizard_type(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" New Container - Step 3: Container Type ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(BorderType::Rounded);

    let container_style = if !app.wizard_data.is_vm {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let vm_style = if app.wizard_data.is_vm {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let text = vec![
        Line::from("Select container type:"),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "[C] Container (lightweight, shares kernel)",
                container_style,
            ),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[V] Virtual Machine (full virtualization)", vm_style),
        ]),
        Line::from(""),
        Line::from("Press C or V to select, Tab to continue"),
    ];

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_wizard_confirm(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" New Container - Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .border_type(BorderType::Rounded);

    let container_type = if app.wizard_data.is_vm {
        "Virtual Machine"
    } else {
        "Container"
    };

    let text = vec![
        Line::from("Review your container configuration:"),
        Line::from(""),
        Line::from(format!("  Name:  {}", app.wizard_data.name)),
        Line::from(format!("  Image: {}", app.wizard_data.image)),
        Line::from(format!("  Type:  {}", container_type)),
        Line::from(""),
        Line::from("Press Enter to create or Esc to cancel"),
    ];

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}
