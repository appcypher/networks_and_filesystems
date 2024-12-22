use anyhow::Result;
use log::{info, warn};
use std::path::PathBuf;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use bytes::BytesMut;

mod protocol;
mod server;
mod rpc;

use crate::server::NfsServer;
use crate::protocol::{NFS_VERSION, NFS_PROGRAM};
use crate::rpc::{RpcMsg, RpcMsgBody, read_rpc_message};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    info!("Starting NFSv4 server...");

    let bind_addr = "127.0.0.1:2049";
    let export_path = PathBuf::from("/tmp/nfs_root");

    // Ensure we have root privileges (NFS typically requires port 2049)
    if !sudo::check() {
        warn!("NFSv4 server typically requires root privileges to bind to port 2049");
        warn!("Please run with sudo");
        std::process::exit(1);
    }

    // Create export directory if it doesn't exist
    std::fs::create_dir_all(&export_path)?;

    // Initialize NFS server
    let nfs_server = NfsServer::new(export_path.clone());

    info!("Binding to {}", bind_addr);
    let listener = TcpListener::bind(bind_addr).await?;
    info!("NFSv4 server listening on {}", bind_addr);
    info!("Exporting directory: {:?}", export_path);

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                info!("New connection from: {}", addr);
                let server = nfs_server.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(socket, server).await {
                        warn!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                warn!("Error accepting connection: {}", e);
            }
        }
    }
}

async fn handle_client(mut socket: tokio::net::TcpStream, server: NfsServer) -> Result<()> {
    let mut buf = BytesMut::with_capacity(4096);

    loop {
        // Read data into buffer
        let n = socket.read_buf(&mut buf).await?;
        if n == 0 {
            // Connection closed
            return Ok(());
        }

        // Process RPC messages
        while let Some(msg_result) = read_rpc_message(&mut buf) {
            let msg = msg_result?;

            match msg.body {
                RpcMsgBody::Call(call) if call.prog == NFS_PROGRAM && call.prog_vers == NFS_VERSION => {
                    // Decode and handle the NFS request
                    let request = serde_xdr::from_bytes(&call.data)?;
                    let response = server.handle_compound(request).await?;

                    // Encode and send the response
                    let response_data = serde_xdr::to_bytes(&response)?;
                    let response_msg = RpcMsg::new_success_reply(msg.xid, response_data);
                    let encoded = response_msg.encode()?;

                    let msg_len = (encoded.len() as u32).to_be_bytes();
                    socket.write_all(&msg_len).await?;
                    socket.write_all(&encoded).await?;
                }
                _ => {
                    // Send error response for unsupported operations
                    let response_msg = RpcMsg::new_prog_mismatch_reply(msg.xid);
                    let encoded = response_msg.encode()?;

                    let msg_len = (encoded.len() as u32).to_be_bytes();
                    socket.write_all(&msg_len).await?;
                    socket.write_all(&encoded).await?;
                }
            }
        }
    }
}
