use anyhow::{Context, Result};
use futures::StreamExt;
use std::net::Ipv4Addr;
use tun::Configuration;

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
                format!("10.{}.0.1", i).parse().unwrap(),    // IP address
                format!("255.255.255.0").parse().unwrap(),   // Netmask
                format!("10.{}.0.255", i).parse().unwrap(),  // Broadcast address
            ));
        }
    }

    anyhow::bail!("No available subnets found in the 10.0.0.0/8 range")
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Creating TUN interface...");

    // Find an available subnet
    let (ip_addr, netmask, broadcast) = find_available_subnet()?;
    println!("Using subnet configuration:");
    println!("  IP Address: {}", ip_addr);
    println!("  Netmask: {}", netmask);
    println!("  Broadcast: {}", broadcast);

    let mut config = Configuration::default();
    config
        .address(ip_addr)
        .destination(ip_addr)
        .netmask(netmask)
        .up();

    #[cfg(target_os = "macos")]
    config.platform_config(|config| {
        config.packet_information(true);
    });

    #[cfg(target_os = "linux")]
    config.platform_config(|config| {
        config.ensure_root_privileges(true);
    });

    let dev = tun::create_as_async(&config).context("Failed to create TUN interface")?;
    println!("TUN interface created successfully!");

    // Keep the interface alive and print received packets
    let mut framed = dev.into_framed();
    while let Some(packet) = framed.next().await {
        match packet {
            Ok(packet) => {
                println!("Received packet: {:?}", packet);
            }
            Err(e) => {
                eprintln!("Error receiving packet: {}", e);
            }
        }
    }

    Ok(())
}
