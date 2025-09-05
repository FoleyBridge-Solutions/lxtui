//! Application state and business logic
//!
//! This module contains the core application state management and business logic
//! for LXTUI. It handles container operations, UI state, and background tasks.

use crate::lxc::{Container, Image, LxcClient, Operation};
use anyhow::Result;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

// Type for background task results
pub type TaskResult = (String, bool, Option<String>, String); // (op_id, success, error_msg, container_name)

// LXD Operation Tracker
#[derive(Debug, Clone)]
pub struct LxdOperationTracker {
    pub ui_operation_id: String,    // Our internal UI operation ID
    pub lxd_operation_path: String, // LXD's operation path (e.g., "/1.0/operations/uuid")
    pub description: String,
    pub container_name: String,
    pub action: String, // "start", "stop", "restart", "delete"
    pub started_at: Instant,
    pub last_checked: Instant,
    pub status_code: i32,      // LXD status code
    pub progress: Option<i32>, // Progress percentage if available
}

#[derive(Debug, Clone)]
pub enum WizardState {
    Name,
    SelectImage,
    SelectType,
    Confirm,
}

#[derive(Debug, Clone)]
pub struct WizardData {
    pub name: String,
    pub image: String,
    pub is_vm: bool,
    pub selected_image_index: usize,
}

impl Default for WizardData {
    fn default() -> Self {
        WizardData {
            name: String::new(),
            image: "ubuntu:24.04".to_string(),
            is_vm: false,
            selected_image_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    StartContainer(String),
    StopContainer(String),
    RestartContainer(String),
    DeleteContainer(String),
}

#[derive(Debug, Clone)]
pub enum CommandMenu {
    Closed,
    Main,
    Container,
    System,
}

#[derive(Debug, Clone)]
pub enum StatusModalType {
    Info {
        message: String,
        auto_close: bool,
    },
    Progress {
        operation_id: String,
    },
    Error {
        title: String,
        details: String,
        suggestions: Vec<String>,
    },
    Success {
        message: String,
        started_at: Instant,
    },
}

#[derive(Debug, Clone)]
pub enum OperationStatus {
    Registered,
    Running,
    Retrying(u32),
    Success,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct UserOperation {
    pub id: String,
    pub description: String,
    pub container: Option<String>,
    pub status: OperationStatus,
    pub started_at: Option<Instant>,
    pub completed_at: Option<Instant>,
    pub retry_count: u32,
}

#[derive(Debug)]
pub enum InputMode {
    Normal,
    CommandMenu(CommandMenu),
    StatusModal(StatusModalType),
    Confirmation {
        message: String,
        action: ConfirmAction,
    },
    Input {
        prompt: String,
        input_type: InputType,
        callback_action: InputCallback,
    },
    Wizard(WizardState),
}

#[derive(Debug, Clone)]
pub enum InputType {
    ContainerName,
    ImageName,
}

#[derive(Debug, Clone)]
pub enum InputCallback {
    CloneContainer(String), // source name
    CreateContainer,
}

pub struct App {
    pub containers: Arc<RwLock<Vec<Container>>>,
    pub selected: usize,
    pub lxc_client: LxcClient,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub wizard_data: WizardData,
    pub available_images: Vec<Image>,
    pub message: Option<String>,
    pub should_quit: bool,
    pub exec_container: Option<String>,
    pub operations: Vec<Operation>,
    pub user_operations: Vec<UserOperation>,
    pub last_refresh: Option<Instant>,
    pub pending_action: Option<ConfirmAction>,
    pub command_feedback: Option<String>,
    pub active_operation_count: usize,
    pub show_operation_sidebar: bool,
    pub last_lxd_check: Option<Instant>,
    pub lxd_status: bool,
    pub background_tasks: HashMap<String, JoinHandle<()>>, // Track background operations (simplified)
    pub task_result_tx: mpsc::UnboundedSender<TaskResult>, // Channel to send results from background tasks
    pub task_result_rx: mpsc::UnboundedReceiver<TaskResult>, // Channel to receive results in main thread
    pub lxd_operations: HashMap<String, LxdOperationTracker>, // Track LXD operations
    pub menu_selected: usize,                                // Currently selected menu item
}

impl App {
    pub fn new() -> Self {
        // Create the channel for background task results
        let (task_result_tx, task_result_rx) = mpsc::unbounded_channel();

        App {
            containers: Arc::new(RwLock::new(Vec::new())),
            selected: 0,
            lxc_client: LxcClient::new(),
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            wizard_data: WizardData::default(),
            available_images: Vec::new(),
            message: None,
            should_quit: false,
            exec_container: None,
            operations: Vec::new(),
            user_operations: Vec::new(),
            last_refresh: None,
            pending_action: None,
            command_feedback: None,
            active_operation_count: 0,
            show_operation_sidebar: false,
            last_lxd_check: None,
            lxd_status: false,
            background_tasks: HashMap::new(),
            task_result_tx,
            task_result_rx,
            lxd_operations: HashMap::new(),
            menu_selected: 0,
        }
    }

    pub async fn initialize(&mut self) {
        info!("Initializing application");

        // Load available images
        self.load_available_images();

        // Try to ensure LXD is running and refresh containers
        self.ensure_lxd_and_refresh().await;
    }

    pub fn load_available_images(&mut self) {
        // Predefined popular images
        self.available_images = vec![
            Image {
                alias: "ubuntu:24.04".to_string(),
                description: "Ubuntu 24.04 LTS".to_string(),
            },
            Image {
                alias: "ubuntu:22.04".to_string(),
                description: "Ubuntu 22.04 LTS".to_string(),
            },
            Image {
                alias: "debian:12".to_string(),
                description: "Debian 12 (Bookworm)".to_string(),
            },
            Image {
                alias: "debian:11".to_string(),
                description: "Debian 11 (Bullseye)".to_string(),
            },
            Image {
                alias: "alpine:3.20".to_string(),
                description: "Alpine Linux 3.20".to_string(),
            },
            Image {
                alias: "alpine:3.19".to_string(),
                description: "Alpine Linux 3.19".to_string(),
            },
            Image {
                alias: "fedora:40".to_string(),
                description: "Fedora 40".to_string(),
            },
            Image {
                alias: "rockylinux:9".to_string(),
                description: "Rocky Linux 9".to_string(),
            },
            Image {
                alias: "archlinux:current".to_string(),
                description: "Arch Linux (Current)".to_string(),
            },
        ];
    }

    pub async fn ensure_lxd_and_refresh(&mut self) {
        match self.lxc_client.ensure_lxd_running().await {
            Ok(started) => {
                self.lxd_status = started;
                self.last_lxd_check = Some(Instant::now());
                if started {
                    self.show_info("LXD service is running".to_string(), true);
                    let _ = self.refresh_containers().await;
                } else {
                    self.show_error(
                        "LXD service not running".to_string(),
                        "Could not start LXD service".to_string(),
                        vec![
                            "Try running with sudo".to_string(),
                            "Check systemctl status lxd".to_string(),
                        ],
                    );
                }
            }
            Err(e) => {
                error!("Error starting LXD service: {:?}", e);
                self.lxd_status = false;
                self.last_lxd_check = Some(Instant::now());
                self.show_error(
                    "LXD Service Error".to_string(),
                    e.to_string(),
                    vec![
                        "Check LXD installation".to_string(),
                        "Run 'sudo systemctl status lxd'".to_string(),
                    ],
                );
            }
        }
    }

    pub async fn refresh_containers(&mut self) -> Result<()> {
        debug!("Refreshing container list");

        match self.lxc_client.list_containers().await {
            Ok(containers) => {
                let container_count = containers.len();
                *self.containers.write().await = containers;

                let containers_read = self.containers.read().await;
                if self.selected >= containers_read.len() && !containers_read.is_empty() {
                    self.selected = containers_read.len() - 1;
                }
                drop(containers_read);

                self.last_refresh = Some(Instant::now());
                self.message = Some(format!("Refreshed - {} containers found", container_count));
                info!("Container list refreshed - {} containers", container_count);
                Ok(())
            }
            Err(e) => {
                error!("Failed to refresh containers: {:?}", e);
                self.message = Some(format!("Cannot connect to LXD: {}", e));
                *self.containers.write().await = Vec::new();
                Ok(())
            }
        }
    }

    pub async fn next(&mut self) {
        let containers = self.containers.read().await;
        if !containers.is_empty() {
            self.selected = (self.selected + 1) % containers.len();
        }
    }

    pub async fn previous(&mut self) {
        let containers = self.containers.read().await;
        if !containers.is_empty() {
            if self.selected > 0 {
                self.selected -= 1;
            } else {
                self.selected = containers.len() - 1;
            }
        }
    }

    pub async fn get_selected_container(&self) -> Option<Container> {
        let containers = self.containers.read().await;
        containers.get(self.selected).cloned()
    }

    pub fn show_confirm_dialog(&mut self, message: String, action: ConfirmAction) {
        self.pending_action = Some(action.clone());
        self.input_mode = InputMode::Confirmation { message, action };
    }

    pub fn show_status_modal(&mut self, modal_type: StatusModalType) {
        self.input_mode = InputMode::StatusModal(modal_type);
    }

    pub fn show_command_menu(&mut self, menu: CommandMenu) {
        self.menu_selected = 0; // Reset selection when opening menu
        self.input_mode = InputMode::CommandMenu(menu);
    }

    pub fn menu_next(&mut self, item_count: usize) {
        if item_count > 0 {
            self.menu_selected = (self.menu_selected + 1) % item_count;
        }
    }

    pub fn menu_previous(&mut self, item_count: usize) {
        if item_count > 0 {
            if self.menu_selected > 0 {
                self.menu_selected -= 1;
            } else {
                self.menu_selected = item_count - 1;
            }
        }
    }

    pub fn show_info(&mut self, message: String, auto_close: bool) {
        self.show_status_modal(StatusModalType::Info {
            message,
            auto_close,
        });
    }

    pub fn show_error(&mut self, title: String, details: String, suggestions: Vec<String>) {
        self.show_status_modal(StatusModalType::Error {
            title,
            details,
            suggestions,
        });
    }

    pub fn show_success(&mut self, message: String) {
        self.show_status_modal(StatusModalType::Success {
            message,
            started_at: Instant::now(),
        });
    }

    pub async fn start_selected(&mut self) {
        if let Some(container) = self.get_selected_container().await {
            let name = container.name.clone();
            self.show_confirm_dialog(
                format!("Start container '{}'?", name),
                ConfirmAction::StartContainer(name),
            );
        }
    }

    // execute_pending_action has been removed - the logic is now in handle_confirmation in main.rs
    // to ensure immediate UI updates when the user confirms an action

    pub async fn _unused_execute_pending_action(&mut self) {
        if let Some(action) = self.pending_action.clone() {
            self.pending_action = None;

            // This method is kept for reference but not used
            match action {
                ConfirmAction::StartContainer(name) => {
                    let operation_id = self.register_operation(
                        format!("Start container '{}'", name),
                        Some(name.clone()),
                    );

                    self.show_status_modal(StatusModalType::Progress {
                        operation_id: operation_id.clone(),
                    });
                    self.start_operation(&operation_id);

                    match self.lxc_client.start_container(&name).await {
                        Ok(_) => {
                            self.complete_operation(&operation_id, true, None);
                            self.show_success(format!("Container '{}' started successfully", name));
                            let _ = self.refresh_containers().await;
                        }
                        Err(e) => {
                            error!("Failed to start container {}: {:?}", name, e);
                            self.complete_operation(&operation_id, false, Some(e.to_string()));
                            self.show_error(
                                format!("Failed to start '{}'", name),
                                e.to_string(),
                                vec![
                                    "Check if the container exists".to_string(),
                                    "Verify LXD service is running".to_string(),
                                    "Check container logs with 'lxc info'".to_string(),
                                ],
                            );
                        }
                    }
                }
                ConfirmAction::StopContainer(name) => {
                    let operation_id = self.register_operation(
                        format!("Stop container '{}'", name),
                        Some(name.clone()),
                    );

                    self.show_status_modal(StatusModalType::Progress {
                        operation_id: operation_id.clone(),
                    });
                    self.start_operation(&operation_id);

                    match self.lxc_client.stop_container(&name).await {
                        Ok(_) => {
                            self.complete_operation(&operation_id, true, None);
                            self.show_success(format!("Container '{}' stopped successfully", name));
                            let _ = self.refresh_containers().await;
                        }
                        Err(e) => {
                            error!("Failed to stop container {}: {:?}", name, e);
                            self.complete_operation(&operation_id, false, Some(e.to_string()));
                            self.show_error(
                                format!("Failed to stop '{}'", name),
                                e.to_string(),
                                vec![
                                    "Try force stopping with 'lxc stop -f'".to_string(),
                                    "Check if processes are hung inside container".to_string(),
                                ],
                            );
                        }
                    }
                }
                ConfirmAction::RestartContainer(name) => {
                    let operation_id = self.register_operation(
                        format!("Restart container '{}'", name),
                        Some(name.clone()),
                    );

                    self.show_status_modal(StatusModalType::Progress {
                        operation_id: operation_id.clone(),
                    });
                    self.start_operation(&operation_id);

                    match self.lxc_client.restart_container(&name).await {
                        Ok(_) => {
                            self.complete_operation(&operation_id, true, None);
                            self.show_success(format!(
                                "Container '{}' restarted successfully",
                                name
                            ));
                            let _ = self.refresh_containers().await;
                        }
                        Err(e) => {
                            error!("Failed to restart container {}: {:?}", name, e);
                            self.complete_operation(&operation_id, false, Some(e.to_string()));
                            self.show_error(
                                format!("Failed to restart '{}'", name),
                                e.to_string(),
                                vec![
                                    "Check container status first".to_string(),
                                    "Try stopping then starting manually".to_string(),
                                ],
                            );
                        }
                    }
                }
                ConfirmAction::DeleteContainer(name) => {
                    let operation_id = self.register_operation(
                        format!("Delete container '{}'", name),
                        Some(name.clone()),
                    );

                    self.show_status_modal(StatusModalType::Progress {
                        operation_id: operation_id.clone(),
                    });
                    self.start_operation(&operation_id);

                    match self.lxc_client.delete_container(&name).await {
                        Ok(_) => {
                            self.complete_operation(&operation_id, true, None);
                            self.show_success(format!("Container '{}' deleted successfully", name));
                            let _ = self.refresh_containers().await;
                        }
                        Err(e) => {
                            error!("Failed to delete container {}: {:?}", name, e);
                            self.complete_operation(&operation_id, false, Some(e.to_string()));
                            self.show_error(
                                format!("Failed to delete '{}'", name),
                                e.to_string(),
                                vec![
                                    "Stop the container first if it's running".to_string(),
                                    "Check for dependent snapshots".to_string(),
                                ],
                            );
                        }
                    }
                }
            }
        }
    }

    pub async fn stop_selected(&mut self) {
        if let Some(container) = self.get_selected_container().await {
            let name = container.name.clone();
            self.show_confirm_dialog(
                format!("Stop container '{}'?", name),
                ConfirmAction::StopContainer(name),
            );
        }
    }

    pub async fn restart_selected(&mut self) {
        if let Some(container) = self.get_selected_container().await {
            let name = container.name.clone();
            self.show_confirm_dialog(
                format!("Restart container '{}'?", name),
                ConfirmAction::RestartContainer(name),
            );
        }
    }

    pub async fn delete_selected(&mut self) {
        if let Some(container) = self.get_selected_container().await {
            let name = container.name.clone();
            self.show_confirm_dialog(
                format!("Delete container '{}'? This action cannot be undone!", name),
                ConfirmAction::DeleteContainer(name),
            );
        }
    }

    pub fn cancel_dialog(&mut self) {
        self.pending_action = None;
        self.input_mode = InputMode::Normal;
        self.message = Some("Operation cancelled".to_string());
    }

    pub fn clear_message(&mut self) {
        self.message = None;
    }

    pub async fn start_clone(&mut self) {
        if let Some(container) = self.get_selected_container().await {
            self.input_mode = InputMode::Input {
                prompt: format!("Clone '{}' to:", container.name),
                input_type: InputType::ContainerName,
                callback_action: InputCallback::CloneContainer(container.name.clone()),
            };
            self.input_buffer.clear();
        }
    }

    pub fn start_new_container_wizard(&mut self) {
        self.wizard_data = WizardData::default();
        self.input_buffer.clear();
        self.input_mode = InputMode::Wizard(WizardState::Name);
    }

    pub async fn clone_container(&mut self, source: &str, destination: &str) {
        let operation_id = self.register_operation(
            format!("Clone '{}' to '{}'", source, destination),
            Some(destination.to_string()),
        );

        self.show_status_modal(StatusModalType::Progress {
            operation_id: operation_id.clone(),
        });
        self.start_operation(&operation_id);

        match self.lxc_client.clone_container(source, destination).await {
            Ok(_) => {
                self.complete_operation(&operation_id, true, None);
                self.show_success(format!(
                    "Successfully cloned '{}' to '{}'",
                    source, destination
                ));
                let _ = self.refresh_containers().await;
                self.input_buffer.clear();
            }
            Err(e) => {
                error!(
                    "Failed to clone container {} to {}: {:?}",
                    source, destination, e
                );
                self.complete_operation(&operation_id, false, Some(e.to_string()));
                self.show_error(
                    format!("Failed to clone '{}'", source),
                    e.to_string(),
                    vec![
                        "Check if destination name is valid".to_string(),
                        "Ensure destination doesn't already exist".to_string(),
                        "Verify sufficient disk space".to_string(),
                    ],
                );
                self.input_buffer.clear();
            }
        }
    }

    pub async fn create_container(&mut self) {
        let name = self.wizard_data.name.clone();
        let image = self.wizard_data.image.clone();
        let is_vm = self.wizard_data.is_vm;

        let operation_id = self.register_operation(
            format!(
                "Create {} '{}' from '{}'",
                if is_vm { "VM" } else { "container" },
                name,
                image
            ),
            Some(name.clone()),
        );

        self.show_status_modal(StatusModalType::Progress {
            operation_id: operation_id.clone(),
        });
        self.start_operation(&operation_id);

        match self.lxc_client.create_container(&name, &image, is_vm).await {
            Ok(_) => {
                self.complete_operation(&operation_id, true, None);
                self.show_success(format!(
                    "Successfully created {} '{}'",
                    if is_vm { "VM" } else { "container" },
                    name
                ));
                let _ = self.refresh_containers().await;
                self.wizard_data = WizardData::default();
                self.input_buffer.clear();
            }
            Err(e) => {
                error!("Failed to create container {}: {:?}", name, e);
                self.complete_operation(&operation_id, false, Some(e.to_string()));
                self.show_error(
                    format!("Failed to create '{}'", name),
                    e.to_string(),
                    vec![
                        "Check if image exists and is available".to_string(),
                        "Verify network connectivity".to_string(),
                        "Ensure sufficient resources".to_string(),
                    ],
                );
                self.wizard_data = WizardData::default();
                self.input_buffer.clear();
            }
        }
    }

    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.wizard_data = WizardData::default();
        self.message = Some("Operation cancelled".to_string());
    }

    pub fn next_wizard_image(&mut self) {
        if self.wizard_data.selected_image_index < self.available_images.len() - 1 {
            self.wizard_data.selected_image_index += 1;
            self.wizard_data.image = self.available_images[self.wizard_data.selected_image_index]
                .alias
                .clone();
        }
    }

    pub fn previous_wizard_image(&mut self) {
        if self.wizard_data.selected_image_index > 0 {
            self.wizard_data.selected_image_index -= 1;
            self.wizard_data.image = self.available_images[self.wizard_data.selected_image_index]
                .alias
                .clone();
        }
    }

    pub fn show_help(&mut self) {
        self.show_info(
            "Keyboard Shortcuts:\n\
            \n\
            Navigation:\n\
              ↑/↓ or j/k  - Select container\n\
              Enter       - Container actions menu\n\
            \n\
            Quick Actions:\n\
              s           - Start container\n\
              S           - Stop container\n\
              d           - Delete container\n\
              n           - New container\n\
              r/F5        - Refresh list\n\
            \n\
            System:\n\
              Space       - System menu\n\
              o/O         - Toggle operations sidebar\n\
              ?/h         - This help\n\
              q/Q         - Quit"
                .to_string(),
            false,
        );
    }

    pub fn close_modal(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub async fn update_operations(&mut self) {
        self.operations = self.lxc_client.get_operations().await;
    }

    pub fn should_auto_refresh(&self) -> bool {
        if let Some(last_refresh) = self.last_refresh {
            last_refresh.elapsed() > Duration::from_secs(10)
        } else {
            true
        }
    }

    pub fn register_operation(&mut self, description: String, container: Option<String>) -> String {
        let operation_id = Uuid::new_v4().to_string();
        let operation = UserOperation {
            id: operation_id.clone(),
            description: description.clone(),
            container,
            status: OperationStatus::Registered,
            started_at: None,
            completed_at: None,
            retry_count: 0,
        };

        self.user_operations.push(operation);
        self.command_feedback = Some(format!("⏳ Command registered: {}", description));
        self.active_operation_count += 1;

        // Limit operation history to last 10 items
        if self.user_operations.len() > 10 {
            self.user_operations.remove(0);
        }

        operation_id
    }

    pub fn start_operation(&mut self, operation_id: &str) {
        if let Some(op) = self
            .user_operations
            .iter_mut()
            .find(|o| o.id == operation_id)
        {
            op.status = OperationStatus::Running;
            op.started_at = Some(Instant::now());
            self.command_feedback = Some(format!("🚀 Starting: {}", op.description));
        }
    }

    #[allow(dead_code)]
    pub fn update_operation_retry(&mut self, operation_id: &str, retry_count: u32) {
        if let Some(op) = self
            .user_operations
            .iter_mut()
            .find(|o| o.id == operation_id)
        {
            op.status = OperationStatus::Retrying(retry_count);
            op.retry_count = retry_count;
            self.command_feedback = Some(format!(
                "🔄 Retrying ({}/3): {}",
                retry_count, op.description
            ));
        }
    }

    pub fn complete_operation(
        &mut self,
        operation_id: &str,
        success: bool,
        error_msg: Option<String>,
    ) {
        if let Some(op) = self
            .user_operations
            .iter_mut()
            .find(|o| o.id == operation_id)
        {
            op.status = if success {
                OperationStatus::Success
            } else {
                OperationStatus::Failed(error_msg.clone().unwrap_or_default())
            };
            op.completed_at = Some(Instant::now());

            if self.active_operation_count > 0 {
                self.active_operation_count -= 1;
            }

            let duration = if let Some(started) = op.started_at {
                format!(" ({}s)", started.elapsed().as_secs())
            } else {
                String::new()
            };

            if success {
                self.command_feedback =
                    Some(format!("✅ Completed: {}{}", op.description, duration));
            } else {
                self.command_feedback = Some(format!("❌ Failed: {}{}", op.description, duration));
                if let Some(msg) = error_msg {
                    self.message = Some(format!("Error: {}", msg));
                }
            }
        }
    }

    pub fn cancel_operation(&mut self, operation_id: &str) {
        if let Some(op) = self
            .user_operations
            .iter_mut()
            .find(|o| o.id == operation_id)
        {
            op.status = OperationStatus::Cancelled;
            op.completed_at = Some(Instant::now());

            if self.active_operation_count > 0 {
                self.active_operation_count -= 1;
            }

            self.command_feedback = Some(format!("🚫 Cancelled: {}", op.description));
        }
    }

    pub async fn maybe_auto_refresh(&mut self) {
        if self.should_auto_refresh() && matches!(self.input_mode, InputMode::Normal) {
            let _ = self.refresh_containers().await;
        }

        // Clear command feedback after 3 seconds if no active operations
        if self.active_operation_count == 0 {
            if self.command_feedback.is_some() {
                // Check if the last completed operation was more than 3 seconds ago
                let should_clear = self
                    .user_operations
                    .iter()
                    .filter(|op| {
                        matches!(
                            op.status,
                            OperationStatus::Success
                                | OperationStatus::Failed(_)
                                | OperationStatus::Cancelled
                        )
                    })
                    .next_back()
                    .and_then(|op| op.completed_at)
                    .map(|completed| completed.elapsed().as_secs() > 3)
                    .unwrap_or(true);

                if should_clear {
                    self.command_feedback = None;
                }
            }
        }
    }

    pub async fn poll_lxd_operations(&mut self) {
        let mut completed_ops = Vec::new();
        let mut operations_to_check = Vec::new();

        // First pass: collect operations that need checking
        for (ui_op_id, tracker) in &mut self.lxd_operations {
            // Poll every 500ms
            if tracker.last_checked.elapsed() > Duration::from_millis(500) {
                tracker.last_checked = Instant::now();
                operations_to_check.push((ui_op_id.clone(), tracker.lxd_operation_path.clone()));
            }
        }

        // Second pass: check operations without holding mutable borrow
        for (ui_op_id, lxd_op_path) in operations_to_check {
            // Get operation status from LXD
            match self.lxc_client.get_lxd_operation(&lxd_op_path).await {
                Ok(lxd_op) => {
                    // Update tracker status if it exists
                    if let Some(tracker) = self.lxd_operations.get_mut(&ui_op_id) {
                        tracker.status_code = lxd_op.status_code;

                        // Parse progress if available
                        if let Some(metadata) = &lxd_op.metadata {
                            if let Some(progress) =
                                metadata.get("progress").and_then(|p| p.as_i64())
                            {
                                tracker.progress = Some(progress as i32);
                            }
                        }
                    }

                    // Get tracker info for processing (clone to avoid borrow issues)
                    let tracker_info = self
                        .lxd_operations
                        .get(&ui_op_id)
                        .map(|t| (t.container_name.clone(), t.action.clone()));

                    match lxd_op.status_code {
                        200 => {
                            // Success!
                            info!("LXD operation {} completed successfully", ui_op_id);
                            self.complete_operation(&ui_op_id, true, None);

                            if let Some((container_name, action)) = tracker_info {
                                self.show_success(format!(
                                    "Container '{}' {} successfully",
                                    container_name,
                                    match action.as_str() {
                                        "start" => "started",
                                        "stop" => "stopped",
                                        "restart" => "restarted",
                                        "delete" => "deleted",
                                        _ => "operation completed",
                                    }
                                ));
                            }
                            completed_ops.push(ui_op_id.clone());
                            let _ = self.refresh_containers().await;
                        }
                        400 | 401 => {
                            // Failed or cancelled
                            error!("LXD operation {} failed: {}", ui_op_id, lxd_op.err);
                            self.complete_operation(&ui_op_id, false, Some(lxd_op.err.clone()));

                            if let Some((container_name, action)) = tracker_info {
                                let (title, suggestions) = match action.as_str() {
                                    "start" => (
                                        format!("Failed to start '{}'", container_name),
                                        vec![
                                            "Check if the container exists".to_string(),
                                            "Verify LXD service is running".to_string(),
                                            "Check container logs with 'lxc info'".to_string(),
                                        ],
                                    ),
                                    "stop" => (
                                        format!("Failed to stop '{}'", container_name),
                                        vec![
                                            "Try force stopping with 'lxc stop -f'".to_string(),
                                            "Check if processes are hung inside container"
                                                .to_string(),
                                        ],
                                    ),
                                    "restart" => (
                                        format!("Failed to restart '{}'", container_name),
                                        vec![
                                            "Check container status first".to_string(),
                                            "Try stopping then starting manually".to_string(),
                                        ],
                                    ),
                                    "delete" => (
                                        format!("Failed to delete '{}'", container_name),
                                        vec![
                                            "Stop the container first if it's running".to_string(),
                                            "Check for dependent snapshots".to_string(),
                                        ],
                                    ),
                                    _ => (
                                        format!("Operation failed for '{}'", container_name),
                                        vec!["Check LXD logs for details".to_string()],
                                    ),
                                };

                                self.show_error(title, lxd_op.err, suggestions);
                            }
                            completed_ops.push(ui_op_id.clone());
                        }
                        103..=109 => {
                            // Still running - could update progress UI here
                            debug!(
                                "LXD operation {} still running (code: {})",
                                ui_op_id, lxd_op.status_code
                            );
                        }
                        _ => {
                            // Unknown status
                            warn!("Unknown LXD operation status code: {}", lxd_op.status_code);
                        }
                    }
                }
                Err(e) => {
                    // Error checking operation - maybe it's gone?
                    warn!("Error checking LXD operation {}: {:?}", ui_op_id, e);
                    // Don't remove it yet, will retry on next poll
                }
            }
        }

        // Remove completed operations
        for op_id in completed_ops {
            self.lxd_operations.remove(&op_id);
        }
    }

    pub async fn poll_background_tasks(&mut self) {
        // Poll LXD operations first
        self.poll_lxd_operations().await;

        // Clean up finished task handles
        let mut completed = Vec::new();
        for (id, handle) in &self.background_tasks {
            if handle.is_finished() {
                completed.push(id.clone());
            }
        }
        for id in completed {
            self.background_tasks.remove(&id);
        }

        // Process results from the channel (for non-LXD operations if any)
        while let Ok((op_id, success, error_msg, container_name)) = self.task_result_rx.try_recv() {
            info!("Received task result for {}: success={}", op_id, success);

            // Update operation status
            self.complete_operation(&op_id, success, error_msg.clone());

            // Show appropriate status modal
            if success {
                // Determine the operation type from the description
                let op_desc = self
                    .user_operations
                    .iter()
                    .find(|op| op.id == op_id)
                    .map(|op| op.description.clone())
                    .unwrap_or_default();

                if op_desc.contains("Start") {
                    self.show_success(format!(
                        "Container '{}' started successfully",
                        container_name
                    ));
                } else if op_desc.contains("Stop") {
                    self.show_success(format!(
                        "Container '{}' stopped successfully",
                        container_name
                    ));
                } else if op_desc.contains("Restart") {
                    self.show_success(format!(
                        "Container '{}' restarted successfully",
                        container_name
                    ));
                } else if op_desc.contains("Delete") {
                    self.show_success(format!(
                        "Container '{}' deleted successfully",
                        container_name
                    ));
                }

                // Refresh container list
                let _ = self.refresh_containers().await;
            } else {
                // Show error
                let op_desc = self
                    .user_operations
                    .iter()
                    .find(|op| op.id == op_id)
                    .map(|op| op.description.clone())
                    .unwrap_or_default();

                let (title, suggestions) = if op_desc.contains("Start") {
                    (
                        format!("Failed to start '{}'", container_name),
                        vec![
                            "Check if the container exists".to_string(),
                            "Verify LXD service is running".to_string(),
                            "Check container logs with 'lxc info'".to_string(),
                        ],
                    )
                } else if op_desc.contains("Stop") {
                    (
                        format!("Failed to stop '{}'", container_name),
                        vec![
                            "Try force stopping with 'lxc stop -f'".to_string(),
                            "Check if processes are hung inside container".to_string(),
                        ],
                    )
                } else if op_desc.contains("Restart") {
                    (
                        format!("Failed to restart '{}'", container_name),
                        vec![
                            "Check container status first".to_string(),
                            "Try stopping then starting manually".to_string(),
                        ],
                    )
                } else {
                    (
                        format!("Failed to delete '{}'", container_name),
                        vec![
                            "Stop the container first if it's running".to_string(),
                            "Check for dependent snapshots".to_string(),
                        ],
                    )
                };

                self.show_error(title, error_msg.unwrap_or_default(), suggestions);
            }
        }
    }
}
