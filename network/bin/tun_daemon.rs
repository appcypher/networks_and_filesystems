use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use daemonize::Daemonize;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::Level;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{filter::filter_fn, fmt, Layer};

use network::tun_interface::{create_tun_device, CreateTunRequest, TunDevice};

//-------------------------------------------------------------------------------------------------
// Types
//-------------------------------------------------------------------------------------------------

type DeviceStore = Arc<Mutex<HashMap<String, TunDevice>>>;

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

    tracing::info!("Starting TUN daemon initialization...");

    // Setup daemon
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        // Clean up any stale pid file
        let pid_file = "/var/run/tun_daemon.pid";
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
            .open("/var/log/tun_daemon.log")
            .unwrap();
        let stderr = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/var/log/tun_daemon.err")
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
    let devices: DeviceStore = Arc::new(Mutex::new(HashMap::new()));

    // Build router
    let app = Router::new()
        .route("/tun", post(create_tun))
        .route("/tun", get(list_tuns))
        .with_state(devices);

    // Run server
    let addr = "127.0.0.1:3030";
    tracing::info!("Attempting to bind to {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("TUN daemon successfully bound to {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn create_tun(
    State(devices): State<DeviceStore>,
    Json(req): Json<CreateTunRequest>,
) -> Result<Json<TunDevice>, ApiError> {
    tracing::debug!("Attempting to create TUN device with config: {:?}", req);

    let device = create_tun_device(req.name)?;

    // Store the device
    let mut devices = devices.lock().await;
    devices.insert(device.name.clone(), device.clone());

    tracing::info!("Created TUN device: {:?}", device);
    Ok(Json(device))
}

async fn list_tuns(State(devices): State<DeviceStore>) -> Json<Vec<TunDevice>> {
    let devices = devices.lock().await;
    Json(devices.values().cloned().collect())
}
