use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use daemonize::Daemonize;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::Level;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{filter::filter_fn, fmt, Layer};

use network::subnet_interface::{
    configure_subnet, detect_existing_subnets, remove_subnet, CreateSubnetRequest, Subnet,
};

//-------------------------------------------------------------------------------------------------
// Types
//-------------------------------------------------------------------------------------------------

type SubnetStore = Arc<Mutex<HashMap<String, Subnet>>>;

// Custom error type for our API
struct ApiError(anyhow::Error);

//-------------------------------------------------------------------------------------------------
// Methods
//-------------------------------------------------------------------------------------------------

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

//-------------------------------------------------------------------------------------------------
// Functions: main
//-------------------------------------------------------------------------------------------------

fn main() -> Result<()> {
    // Initialize logging first, before any operations
    let stdout_layer = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_writer(std::io::stdout)
        .with_filter(filter_fn(|metadata| {
            matches!(
                metadata.level(),
                &Level::INFO | &Level::DEBUG | &Level::TRACE
            )
        }));

    let stderr_layer = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_writer(std::io::stderr)
        .with_filter(filter_fn(|metadata| {
            matches!(metadata.level(), &Level::ERROR | &Level::WARN)
        }));

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(stderr_layer)
        .init();

    tracing::info!("Starting Subnet daemon initialization...");

    // Setup daemon
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        // Clean up any stale pid file
        let pid_file = "/var/run/subnet_daemon.pid";
        if std::path::Path::new(pid_file).exists() {
            if let Err(e) = std::fs::remove_file(pid_file) {
                tracing::error!("Failed to remove stale pid file: {}", e);
                std::process::exit(1);
            }
        }

        // Open log files for stdout and stderr
        let stdout = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/var/log/subnet_daemon.log")
            .unwrap();
        let stderr = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/var/log/subnet_daemon.err")
            .unwrap();

        let daemonize = Daemonize::new()
            .pid_file(pid_file)
            .chown_pid_file(true)
            .working_directory("/tmp")
            .user("root")
            .group(if cfg!(target_os = "macos") {
                "wheel"
            } else {
                "root"
            })
            .umask(0o027)
            .stdout(stdout)
            .stderr(stderr);

        tracing::info!("Attempting to daemonize process...");
        match daemonize.start() {
            Ok(_) => {
                tracing::info!("Successfully daemonized");
                // Create a new runtime after daemonization
                let runtime = tokio::runtime::Runtime::new().unwrap();
                if let Err(e) = runtime.block_on(run_server()) {
                    tracing::error!("Server error: {}", e);
                    std::process::exit(1);
                }
            }
            Err(e) => {
                tracing::error!("Error starting daemon: {}", e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

//-------------------------------------------------------------------------------------------------
// Functions
//-------------------------------------------------------------------------------------------------

async fn run_server() -> Result<()> {
    // Setup state
    let subnets: SubnetStore = Arc::new(Mutex::new(HashMap::new()));
    let subnets_for_shutdown = subnets.clone();

    // Detect and register existing subnets
    match detect_existing_subnets() {
        Ok(existing_subnets) => {
            let mut subnet_store = subnets.lock().await;
            tracing::info!("Found {} existing subnets", existing_subnets.len());
            for subnet in existing_subnets {
                tracing::info!(
                    "Registering existing subnet: {} (interface: {})",
                    subnet.cidr,
                    subnet.interface
                );
                subnet_store.insert(subnet.cidr.clone(), subnet);
            }

            // Print all registered subnets
            if !subnet_store.is_empty() {
                tracing::info!("Currently registered subnets:");
                for (cidr, subnet) in subnet_store.iter() {
                    tracing::info!("  - {} on {}", cidr, subnet.interface);
                }
            } else {
                tracing::info!("No subnets currently registered");
            }
        }
        Err(e) => {
            tracing::warn!("Failed to detect existing subnets: {}", e);
        }
    }

    // Build router
    let app = Router::new()
        .route("/subnet", post(create_subnet))
        .route("/subnet", get(list_subnets))
        .route("/subnet/:cidr", delete(remove_subnet_handler))
        .with_state(subnets);

    // Run server
    let addr = "127.0.0.1:3031";
    tracing::info!("Attempting to bind to {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Subnet daemon successfully bound to {}", addr);

    // Setup shutdown signal handler
    let (tx, rx) = tokio::sync::oneshot::channel();
    let shutdown_complete = Arc::new(tokio::sync::Notify::new());
    let shutdown_complete_wait = shutdown_complete.clone();

    // Spawn shutdown signal handler
    tokio::spawn(async move {
        if let Ok(()) = rx.await {
            tracing::info!("Shutdown signal received, cleaning up subnets...");
            let subnets = subnets_for_shutdown.lock().await;

            for subnet in subnets.values() {
                if let Err(e) = remove_subnet(subnet) {
                    tracing::error!("Failed to remove subnet {}: {}", subnet.cidr, e);
                } else {
                    tracing::info!("Successfully removed subnet {}", subnet.cidr);
                }
            }

            shutdown_complete.notify_one();
        }
    });

    // Handle SIGTERM for daemon shutdown
    let shutdown_tx = tx;
    tokio::spawn(async move {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
                tracing::info!("SIGTERM received, initiating shutdown...");
                let _ = shutdown_tx.send(());
            }
            Err(e) => {
                tracing::error!("Failed to install SIGTERM handler: {}", e);
            }
        }
    });

    // Run the server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_complete_wait.notified().await;
        })
        .await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}

//-------------------------------------------------------------------------------------------------
// Functions: API
//-------------------------------------------------------------------------------------------------

async fn create_subnet(
    State(subnets): State<SubnetStore>,
    Json(req): Json<CreateSubnetRequest>,
) -> Result<Json<Subnet>, ApiError> {
    tracing::debug!("Attempting to create subnet with config: {:?}", req);

    let subnet = configure_subnet(req.cidr)?;

    // Store the subnet
    let mut subnets = subnets.lock().await;
    subnets.insert(subnet.cidr.clone(), subnet.clone());

    tracing::info!("Created subnet: {:?}", subnet);
    Ok(Json(subnet))
}

async fn list_subnets(State(subnets): State<SubnetStore>) -> Json<Vec<Subnet>> {
    let subnets = subnets.lock().await;
    Json(subnets.values().cloned().collect())
}

async fn remove_subnet_handler(
    State(subnets): State<SubnetStore>,
    Path(cidr): Path<String>,
) -> Result<(), ApiError> {
    let mut subnets = subnets.lock().await;

    if let Some(subnet) = subnets.remove(&cidr) {
        remove_subnet(&subnet)?;
        tracing::info!("Removed subnet: {}", cidr);
        Ok(())
    } else {
        Err(ApiError(anyhow::anyhow!("Subnet {} not found", cidr)))
    }
}
