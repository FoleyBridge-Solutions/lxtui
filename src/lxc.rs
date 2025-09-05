//! LXC/LXD client implementation
//!
//! This module provides the interface to LXC/LXD operations, handling
//! container management, state monitoring, and async operations.

use crate::lxd_api::{
    ContainerState as ApiContainerState, LxdApiClient, LxdApiError, LxdContainer, LxdOperation,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct Image {
    pub alias: String,
    pub description: String,
}

#[derive(Debug, Error)]
pub enum LxcError {
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Timeout waiting for operation: {0}")]
    Timeout(String),
    #[error("Container not found: {0}")]
    ContainerNotFound(String),
    #[error("Invalid container state: expected {expected}, got {actual}")]
    InvalidState { expected: String, actual: String },
    #[error("LXD service not available")]
    ServiceUnavailable,
    #[error("Operation cancelled")]
    Cancelled,
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<LxdApiError> for LxcError {
    fn from(err: LxdApiError) -> Self {
        match err {
            LxdApiError::Timeout(msg) => LxcError::Timeout(msg),
            LxdApiError::ApiError(msg) => LxcError::ApiError(msg),
            LxdApiError::OperationFailed(msg) => LxcError::ApiError(msg),
            _ => LxcError::ApiError(err.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum OperationStatus {
    Pending,
    Running,
    Success,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Operation {
    pub id: String,
    pub container: String,
    pub operation_type: String,
    pub status: OperationStatus,
    pub started_at: std::time::Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub name: String,
    pub status: String,
    pub state: ContainerState,
    #[serde(default)]
    pub ipv4: Vec<String>,
    #[serde(default)]
    pub ipv6: Vec<String>,
    #[serde(rename = "type")]
    pub container_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerState {
    pub status: String,
    pub status_code: i32,
}

#[derive(Clone)]
pub struct LxcClient {
    api_client: Arc<Mutex<LxdApiClient>>,
    operations: Arc<RwLock<Vec<Operation>>>,
    cancellation_token: CancellationToken,
    operation_lock: Arc<Mutex<()>>,
}

impl LxcClient {
    pub fn new() -> Self {
        // Create API client - handle error by creating a dummy client if socket not found
        let api_client = LxdApiClient::new().unwrap_or_else(|_| {
            // This will be handled when actual operations are attempted
            // For now, create a client with an invalid socket path
            LxdApiClient::new().unwrap_or_else(|_| {
                // Panic here is fine as this should not happen in practice
                panic!("Failed to create LXD API client")
            })
        });

        Self {
            api_client: Arc::new(Mutex::new(api_client)),
            operations: Arc::new(RwLock::new(Vec::new())),
            cancellation_token: CancellationToken::new(),
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn get_operations(&self) -> Vec<Operation> {
        self.operations.read().await.clone()
    }

    pub async fn add_operation(&self, operation: Operation) -> String {
        let mut ops = self.operations.write().await;
        let id = operation.id.clone();
        ops.push(operation);
        if ops.len() > 50 {
            ops.drain(0..10);
        }
        id
    }

    pub async fn update_operation_status(&self, id: &str, status: OperationStatus) {
        let mut ops = self.operations.write().await;
        if let Some(op) = ops.iter_mut().find(|o| o.id == id) {
            op.status = status;
        }
    }

    pub fn cancel_all_operations(&self) {
        self.cancellation_token.cancel();
    }

    pub async fn ensure_lxd_running(&self) -> Result<bool, LxcError> {
        let client = self.api_client.lock().await;

        // Check if LXD is accessible via API
        if client.check_lxd_running().await {
            return Ok(true);
        }

        // If not running, we can't start it via API
        // User needs to start it manually with systemctl
        Err(LxcError::ServiceUnavailable)
    }

    pub async fn list_containers(&self) -> Result<Vec<Container>, LxcError> {
        let client = self.api_client.lock().await;

        let api_containers = client.list_containers().await?;

        let mut containers = Vec::new();
        for api_container in api_containers {
            // Get the state for IP addresses
            let state = client.get_container_state(&api_container.name).await.ok();

            let mut ipv4_addresses = Vec::new();
            if let Some(state) = &state {
                if let Some(network) = &state.network {
                    for (_name, interface) in network {
                        for addr in &interface.addresses {
                            if addr.family == "inet" && addr.address != "127.0.0.1" {
                                ipv4_addresses.push(addr.address.clone());
                            }
                        }
                    }
                }
            }

            containers.push(Container {
                name: api_container.name,
                status: api_container.status.clone(),
                state: ContainerState {
                    status: api_container.status,
                    status_code: api_container.status_code,
                },
                ipv4: ipv4_addresses,
                ipv6: Vec::new(),
                container_type: api_container.container_type,
            });
        }

        Ok(containers)
    }

    pub async fn start_container(&self, name: &str) -> Result<(), LxcError> {
        let _lock = self.operation_lock.lock().await;

        // Check if container exists and is not already running
        let client = self.api_client.lock().await;
        let state = client.get_container_state(name).await?;

        if state.status == "Running" {
            return Ok(());
        }

        // Start the container
        client.start_container(name).await?;

        // Wait for it to be running
        self.wait_for_state(name, "Running", Duration::from_secs(30))
            .await?;

        Ok(())
    }

    pub async fn stop_container(&self, name: &str) -> Result<(), LxcError> {
        let _lock = self.operation_lock.lock().await;

        let client = self.api_client.lock().await;
        let state = client.get_container_state(name).await?;

        if state.status == "Stopped" {
            return Ok(());
        }

        client.stop_container(name).await?;

        // Wait for it to be stopped
        self.wait_for_state(name, "Stopped", Duration::from_secs(30))
            .await?;

        Ok(())
    }

    pub async fn restart_container(&self, name: &str) -> Result<(), LxcError> {
        let _lock = self.operation_lock.lock().await;

        let client = self.api_client.lock().await;
        client.restart_container(name).await?;

        // Wait for it to be running again
        self.wait_for_state(name, "Running", Duration::from_secs(60))
            .await?;

        Ok(())
    }

    pub async fn delete_container(&self, name: &str) -> Result<(), LxcError> {
        let _lock = self.operation_lock.lock().await;

        let client = self.api_client.lock().await;
        client.delete_container(name).await?;

        Ok(())
    }

    pub async fn create_container(
        &self,
        name: &str,
        image: &str,
        is_vm: bool,
    ) -> Result<(), LxcError> {
        let _lock = self.operation_lock.lock().await;

        let client = self.api_client.lock().await;
        client.create_container(name, image, is_vm).await?;

        // Container should be started automatically by the API
        self.wait_for_state(name, "Running", Duration::from_secs(120))
            .await?;

        Ok(())
    }

    pub async fn clone_container(&self, source: &str, destination: &str) -> Result<(), LxcError> {
        let _lock = self.operation_lock.lock().await;

        let client = self.api_client.lock().await;
        client.clone_container(source, destination).await?;

        Ok(())
    }

    async fn wait_for_state(
        &self,
        name: &str,
        expected_state: &str,
        timeout_duration: Duration,
    ) -> Result<(), LxcError> {
        let start = tokio::time::Instant::now();
        let poll_interval = Duration::from_millis(500);

        loop {
            if start.elapsed() > timeout_duration {
                return Err(LxcError::Timeout(format!(
                    "Timeout waiting for container {} to reach state {}",
                    name, expected_state
                )));
            }

            let client = self.api_client.lock().await;
            match client.get_container_state(name).await {
                Ok(state) => {
                    if state.status == expected_state {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Container might not exist yet (during creation)
                    if expected_state == "Running" {
                        // Keep waiting
                    } else {
                        // For other states, this is an error
                        return Err(LxcError::ContainerNotFound(name.to_string()));
                    }
                }
            }

            sleep(poll_interval).await;
        }
    }

    #[allow(dead_code)]
    pub async fn get_container_info(&self, name: &str) -> Result<String, LxcError> {
        let client = self.api_client.lock().await;
        let container = client.get_container(name).await?;
        Ok(serde_json::to_string_pretty(&container)?)
    }

    #[allow(dead_code)]
    pub async fn list_images(&self) -> Result<Vec<String>, LxcError> {
        // This would require implementing image listing in the API client
        // For now, return a static list
        Ok(vec![
            "ubuntu:20.04".to_string(),
            "ubuntu:22.04".to_string(),
            "debian:11".to_string(),
            "debian:12".to_string(),
            "alpine:3.19".to_string(),
            "alpine:3.20".to_string(),
        ])
    }

    // Non-blocking operation methods
    pub async fn start_container_async(&self, name: &str) -> Result<String, LxcError> {
        let client = self.api_client.lock().await;
        client
            .start_container_async(name)
            .await
            .map_err(|e| LxcError::ApiError(e.to_string()))
    }

    pub async fn stop_container_async(&self, name: &str) -> Result<String, LxcError> {
        let client = self.api_client.lock().await;
        client
            .stop_container_async(name)
            .await
            .map_err(|e| LxcError::ApiError(e.to_string()))
    }

    pub async fn restart_container_async(&self, name: &str) -> Result<String, LxcError> {
        let client = self.api_client.lock().await;
        client
            .restart_container_async(name)
            .await
            .map_err(|e| LxcError::ApiError(e.to_string()))
    }

    pub async fn delete_container_async(&self, name: &str) -> Result<String, LxcError> {
        let client = self.api_client.lock().await;
        client
            .delete_container_async(name)
            .await
            .map_err(|e| LxcError::ApiError(e.to_string()))
    }

    pub async fn get_lxd_operation(&self, operation_path: &str) -> Result<LxdOperation, LxcError> {
        let client = self.api_client.lock().await;
        client
            .get_operation(operation_path)
            .await
            .map_err(|e| LxcError::ApiError(e.to_string()))
    }
}
