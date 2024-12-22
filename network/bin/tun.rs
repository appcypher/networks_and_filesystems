use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
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

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
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

    tracing::info!("Starting TUN service...");

    // Setup state
    let devices: DeviceStore = Arc::new(Mutex::new(HashMap::new()));

    // Build router
    let app = Router::new()
        .route("/tun", post(create_tun))
        .route("/tun", get(list_tuns))
        .with_state(devices);

    // Run server
    let addr = "127.0.0.1:3030";
    tracing::info!("Binding to {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("TUN service successfully bound to {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}

//-------------------------------------------------------------------------------------------------
// Functions: API
//-------------------------------------------------------------------------------------------------

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
