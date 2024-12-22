use anyhow::Result;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use daemonize::Daemonize;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::Level;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{filter::filter_fn, fmt, Layer};
use tun::Device;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
struct TunDevice {
    name: String,
    ip_addr: Ipv4Addr,
    netmask: Ipv4Addr,
    broadcast: Ipv4Addr,
}

#[derive(Debug, Deserialize)]
struct CreateTunRequest {
    name: Option<String>,
}

type DeviceStore = Arc<Mutex<HashMap<String, TunDevice>>>;

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

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
        let pid_file = "/var/run/tun-daemon.pid";
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
            .open("/var/log/tun-daemon.log")
            .unwrap();
        let stderr = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/var/log/tun-daemon.err")
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

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

fn find_available_subnet() -> Result<(Ipv4Addr, Ipv4Addr, Ipv4Addr)> {
    let interfaces = default_net::get_interfaces();

    // Try subnets from 10.0.0.0 to 10.255.0.0
    for i in 0..=255 {
        let subnet = format!("10.{}.0", i);
        let mut in_use = false;

        // Check if this subnet is already in use
        for interface in &interfaces {
            for addr in &interface.ipv4 {
                if addr.addr.to_string().starts_with(&subnet) {
                    in_use = true;
                    break;
                }
            }
            if in_use {
                break;
            }
        }

        if !in_use {
            return Ok((
                format!("10.{}.0.1", i).parse().unwrap(),   // IP address
                format!("255.255.255.0").parse().unwrap(),  // Netmask
                format!("10.{}.0.255", i).parse().unwrap(), // Broadcast address
            ));
        }
    }

    anyhow::bail!("No available subnets found in the 10.0.0.0/8 range")
}

async fn create_tun(
    State(devices): State<DeviceStore>,
    Json(req): Json<CreateTunRequest>,
) -> Result<Json<TunDevice>, String> {
    tracing::debug!("Attempting to create TUN device with config: {:?}", req);

    let (ip_addr, netmask, broadcast) = match find_available_subnet() {
        Ok((ip, mask, bc)) => {
            tracing::debug!(
                "Found available subnet - IP: {}, Netmask: {}, Broadcast: {}",
                ip,
                mask,
                bc
            );
            (ip, mask, bc)
        }
        Err(e) => {
            tracing::error!("Subnet allocation failed: {}", e);
            return Err(format!("Failed to find available subnet: {}", e));
        }
    };

    let mut config = tun::Configuration::default();
    if let Some(name) = req.name.as_ref() {
        tracing::debug!("Using requested device name: {}", name);
        config.name(name);
    }

    config
        .address(ip_addr)
        .destination(ip_addr)
        .netmask(netmask)
        .mtu(1500)
        .up();

    tracing::debug!("Creating TUN device with configuration: {:?}", config);

    let dev = match tun::create(&config) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to create TUN device: {}", e);
            return Err(e.to_string());
        }
    };

    let name = match dev.name() {
        Ok(n) => n,
        Err(e) => {
            tracing::error!("Failed to get device name: {}", e);
            return Err(e.to_string());
        }
    };

    let device = TunDevice {
        name: name.clone(),
        ip_addr,
        netmask,
        broadcast,
    };

    tracing::debug!("Created TUN device: {:?}", device);

    let mut devices = devices.lock().await;
    devices.insert(name.clone(), device.clone());

    tracing::info!(
        "Successfully created and registered TUN device '{}' with IP {}",
        name,
        ip_addr
    );

    Ok(Json(device))
}

async fn list_tuns(State(devices): State<DeviceStore>) -> Json<Vec<TunDevice>> {
    let devices = devices.lock().await;
    Json(devices.values().cloned().collect())
}
