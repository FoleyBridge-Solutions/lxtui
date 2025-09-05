//! LXD REST API client
//!
//! Low-level API client for communicating with the LXD daemon
//! over the Unix socket using the REST API.

use anyhow::Result;
use hyper::{Body, Client, Method, Request};
use hyperlocal::{UnixClientExt, UnixConnector, Uri};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;
use tokio::time::{sleep, timeout};

#[derive(Debug, Error)]
pub enum LxdApiError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] hyper::Error),
    #[error("HTTP builder error: {0}")]
    HttpBuilderError(#[from] hyper::http::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Operation failed: {0}")]
    OperationFailed(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Socket not found: {0}")]
    SocketNotFound(String),
}

// API Response structures
#[derive(Debug, Deserialize, Serialize)]
pub struct LxdResponse<T> {
    #[serde(rename = "type")]
    pub response_type: String,
    pub status: String,
    pub status_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<T>,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LxdOperation {
    pub id: String,
    pub class: String,
    #[serde(default)]
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub status: String,
    pub status_code: i32,
    #[serde(default)]
    pub resources: Option<serde_json::Value>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    pub may_cancel: bool,
    #[serde(default)]
    pub err: String,
    #[serde(default)]
    pub location: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LxdContainer {
    pub architecture: String,
    pub config: HashMap<String, String>,
    pub created_at: String,
    pub devices: HashMap<String, HashMap<String, String>>,
    pub ephemeral: bool,
    pub expanded_config: Option<HashMap<String, String>>,
    pub expanded_devices: Option<HashMap<String, HashMap<String, String>>>,
    pub last_used_at: String,
    pub name: String,
    pub profiles: Vec<String>,
    pub stateful: bool,
    pub status: String,
    pub status_code: i32,
    #[serde(rename = "type")]
    pub container_type: String,
    pub state: Option<ContainerState>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ContainerState {
    pub status: String,
    pub status_code: i32,
    pub network: Option<HashMap<String, NetworkInterface>>,
    pub pid: i64,
    pub processes: i64,
    pub cpu: Option<CpuUsage>,
    pub memory: Option<MemoryUsage>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NetworkInterface {
    pub addresses: Vec<Address>,
    pub counters: HashMap<String, i64>,
    pub hwaddr: String,
    pub mtu: i64,
    pub state: String,
    #[serde(rename = "type")]
    pub interface_type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Address {
    pub address: String,
    pub family: String,
    pub netmask: String,
    pub scope: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CpuUsage {
    pub usage: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MemoryUsage {
    pub usage: i64,
    pub usage_peak: i64,
    pub swap_usage: i64,
    pub swap_usage_peak: i64,
}

pub struct LxdApiClient {
    client: Client<UnixConnector>,
    socket_path: String,
}

impl LxdApiClient {
    pub fn new() -> Result<Self, LxdApiError> {
        // Try standard locations for LXD socket
        let socket_paths = vec![
            "/var/lib/lxd/unix.socket",
            "/var/snap/lxd/common/lxd/unix.socket",
        ];

        let socket_path = socket_paths
            .into_iter()
            .find(|path| Path::new(path).exists())
            .ok_or_else(|| {
                LxdApiError::SocketNotFound(
                    "LXD socket not found at standard locations".to_string(),
                )
            })?;

        let client = Client::unix();

        Ok(Self {
            client,
            socket_path: socket_path.to_string(),
        })
    }

    async fn request<T, B>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
    ) -> Result<T, LxdApiError>
    where
        T: for<'de> Deserialize<'de>,
        B: Serialize,
    {
        let uri: hyper::Uri = Uri::new(&self.socket_path, path).into();

        let mut request = Request::builder().method(method).uri(uri);

        let req = if let Some(body) = body {
            let json_body = serde_json::to_string(&body)?;
            request
                .header("Content-Type", "application/json")
                .body(Body::from(json_body))?
        } else {
            request.body(Body::empty())?
        };

        let response = self.client.request(req).await?;
        let body = hyper::body::to_bytes(response.into_body()).await?;
        let text = String::from_utf8_lossy(&body);

        // Parse the response
        let lxd_response: LxdResponse<T> = serde_json::from_str(&text)?;

        if lxd_response.status_code >= 400 {
            return Err(LxdApiError::ApiError(
                lxd_response
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        lxd_response
            .metadata
            .ok_or_else(|| LxdApiError::ApiError("No metadata in response".to_string()))
    }

    pub async fn list_containers(&self) -> Result<Vec<LxdContainer>, LxdApiError> {
        // Use recursion=1 to get full container details
        self.request(Method::GET, "/1.0/instances?recursion=1", None::<()>)
            .await
    }

    pub async fn get_container(&self, name: &str) -> Result<LxdContainer, LxdApiError> {
        let path = format!("/1.0/instances/{}", name);
        self.request(Method::GET, &path, None::<()>).await
    }

    pub async fn get_container_state(&self, name: &str) -> Result<ContainerState, LxdApiError> {
        let path = format!("/1.0/instances/{}/state", name);
        self.request(Method::GET, &path, None::<()>).await
    }

    pub async fn start_container(&self, name: &str) -> Result<(), LxdApiError> {
        let path = format!("/1.0/instances/{}/state", name);
        let body = json!({
            "action": "start",
            "timeout": 30
        });

        let response: LxdResponse<serde_json::Value> =
            self.request_raw(Method::PUT, &path, Some(body)).await?;

        // If it's an async operation, wait for it
        if let Some(operation_path) = response.operation {
            self.wait_for_operation(&operation_path).await?;
        }

        Ok(())
    }

    pub async fn stop_container(&self, name: &str) -> Result<(), LxdApiError> {
        let path = format!("/1.0/instances/{}/state", name);
        let body = json!({
            "action": "stop",
            "timeout": 30,
            "force": false
        });

        let response: LxdResponse<serde_json::Value> =
            self.request_raw(Method::PUT, &path, Some(body)).await?;

        if let Some(operation_path) = response.operation {
            self.wait_for_operation(&operation_path).await?;
        }

        Ok(())
    }

    pub async fn restart_container(&self, name: &str) -> Result<(), LxdApiError> {
        let path = format!("/1.0/instances/{}/state", name);
        let body = json!({
            "action": "restart",
            "timeout": 30
        });

        let response: LxdResponse<serde_json::Value> =
            self.request_raw(Method::PUT, &path, Some(body)).await?;

        if let Some(operation_path) = response.operation {
            self.wait_for_operation(&operation_path).await?;
        }

        Ok(())
    }

    pub async fn delete_container(&self, name: &str) -> Result<(), LxdApiError> {
        // First stop if running
        let state = self.get_container_state(name).await?;
        if state.status == "Running" {
            self.stop_container(name).await?;
        }

        let path = format!("/1.0/instances/{}", name);
        let response: LxdResponse<serde_json::Value> =
            self.request_raw(Method::DELETE, &path, None::<()>).await?;

        if let Some(operation_path) = response.operation {
            self.wait_for_operation(&operation_path).await?;
        }

        Ok(())
    }

    pub async fn create_container(
        &self,
        name: &str,
        image: &str,
        is_vm: bool,
    ) -> Result<(), LxdApiError> {
        let container_type = if is_vm {
            "virtual-machine"
        } else {
            "container"
        };

        let body = json!({
            "name": name,
            "source": {
                "type": "image",
                "alias": image
            },
            "type": container_type,
            "config": {
                "limits.cpu": "2",
                "limits.memory": "2GB"
            }
        });

        let response: LxdResponse<serde_json::Value> = self
            .request_raw(Method::POST, "/1.0/instances", Some(body))
            .await?;

        if let Some(operation_path) = response.operation {
            self.wait_for_operation(&operation_path).await?;
        }

        // Auto-start after creation
        self.start_container(name).await?;

        Ok(())
    }

    pub async fn clone_container(
        &self,
        source: &str,
        destination: &str,
    ) -> Result<(), LxdApiError> {
        let source_path = format!("/1.0/instances/{}", source);

        let body = json!({
            "name": destination,
            "source": {
                "type": "copy",
                "source": source_path
            }
        });

        let response: LxdResponse<serde_json::Value> = self
            .request_raw(Method::POST, "/1.0/instances", Some(body))
            .await?;

        if let Some(operation_path) = response.operation {
            self.wait_for_operation(&operation_path).await?;
        }

        Ok(())
    }

    async fn request_raw<B>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
    ) -> Result<LxdResponse<serde_json::Value>, LxdApiError>
    where
        B: Serialize,
    {
        let uri: hyper::Uri = Uri::new(&self.socket_path, path).into();

        let mut request = Request::builder().method(method).uri(uri);

        let req = if let Some(body) = body {
            let json_body = serde_json::to_string(&body)?;
            request
                .header("Content-Type", "application/json")
                .body(Body::from(json_body))?
        } else {
            request.body(Body::empty())?
        };

        let response = self.client.request(req).await?;
        let body = hyper::body::to_bytes(response.into_body()).await?;
        let text = String::from_utf8_lossy(&body);

        serde_json::from_str(&text).map_err(LxdApiError::from)
    }

    async fn wait_for_operation(&self, operation_path: &str) -> Result<(), LxdApiError> {
        let max_wait = Duration::from_secs(180);
        let poll_interval = Duration::from_millis(500);

        let start = tokio::time::Instant::now();

        loop {
            if start.elapsed() > max_wait {
                return Err(LxdApiError::Timeout(format!(
                    "Operation {} timed out after {}s",
                    operation_path,
                    max_wait.as_secs()
                )));
            }

            let operation: LxdOperation = self
                .request(Method::GET, operation_path, None::<()>)
                .await?;

            match operation.status_code {
                // Success
                200 => return Ok(()),
                // Cancelled
                401 => {
                    return Err(LxdApiError::OperationFailed(
                        "Operation was cancelled".to_string(),
                    ))
                }
                // Failed
                400 => {
                    let err = if !operation.err.is_empty() {
                        operation.err
                    } else {
                        "Operation failed".to_string()
                    };
                    return Err(LxdApiError::OperationFailed(err));
                }
                // Still running
                103 | 105 | 106 | 107 | 108 | 109 => {
                    sleep(poll_interval).await;
                }
                // Unknown status
                _ => {
                    sleep(poll_interval).await;
                }
            }
        }
    }

    pub async fn check_lxd_running(&self) -> bool {
        // Try to get API version as a health check
        self.request::<Vec<String>, ()>(Method::GET, "/", None)
            .await
            .is_ok()
    }

    // ============== Non-blocking Operation Methods ==============
    // These methods return operation IDs/paths immediately without waiting

    pub async fn start_container_async(&self, name: &str) -> Result<String, LxdApiError> {
        let path = format!("/1.0/instances/{}/state", name);
        let body = json!({
            "action": "start",
            "timeout": 30
        });

        let response: LxdResponse<serde_json::Value> =
            self.request_raw(Method::PUT, &path, Some(body)).await?;

        response
            .operation
            .ok_or_else(|| LxdApiError::ApiError("No operation returned".to_string()))
    }

    pub async fn stop_container_async(&self, name: &str) -> Result<String, LxdApiError> {
        let path = format!("/1.0/instances/{}/state", name);
        let body = json!({
            "action": "stop",
            "timeout": 30
        });

        let response: LxdResponse<serde_json::Value> =
            self.request_raw(Method::PUT, &path, Some(body)).await?;

        response
            .operation
            .ok_or_else(|| LxdApiError::ApiError("No operation returned".to_string()))
    }

    pub async fn restart_container_async(&self, name: &str) -> Result<String, LxdApiError> {
        let path = format!("/1.0/instances/{}/state", name);
        let body = json!({
            "action": "restart",
            "timeout": 30
        });

        let response: LxdResponse<serde_json::Value> =
            self.request_raw(Method::PUT, &path, Some(body)).await?;

        response
            .operation
            .ok_or_else(|| LxdApiError::ApiError("No operation returned".to_string()))
    }

    pub async fn delete_container_async(&self, name: &str) -> Result<String, LxdApiError> {
        let path = format!("/1.0/instances/{}", name);

        let response: LxdResponse<serde_json::Value> =
            self.request_raw(Method::DELETE, &path, None::<()>).await?;

        response
            .operation
            .ok_or_else(|| LxdApiError::ApiError("No operation returned".to_string()))
    }

    pub async fn get_operation(&self, operation_path: &str) -> Result<LxdOperation, LxdApiError> {
        // operation_path is like "/1.0/operations/uuid"
        self.request::<LxdOperation, ()>(Method::GET, operation_path, None)
            .await
    }

    pub async fn get_operations(&self) -> Result<Vec<String>, LxdApiError> {
        let response: LxdResponse<serde_json::Value> = self
            .request_raw(Method::GET, "/1.0/operations", None::<()>)
            .await?;

        if let Some(metadata) = response.metadata {
            if let Some(obj) = metadata.as_object() {
                let mut all_ops = Vec::new();
                for (_status, ops_value) in obj {
                    if let Some(ops_array) = ops_value.as_array() {
                        for op in ops_array {
                            if let Some(op_str) = op.as_str() {
                                all_ops.push(op_str.to_string());
                            }
                        }
                    }
                }
                Ok(all_ops)
            } else {
                Ok(Vec::new())
            }
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn cancel_operation(&self, operation_path: &str) -> Result<(), LxdApiError> {
        self.request_raw::<()>(Method::DELETE, operation_path, None)
            .await?;
        Ok(())
    }
}
