use anyhow::{anyhow, Result};
use default_net;
use ipnet::Ipv4Net;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::process::Command;
use std::str::FromStr;

lazy_static! {
    static ref ALLOWED_NETWORK: Ipv4Net =
        Ipv4Net::new(Ipv4Addr::new(10, 0, 0, 0), 8).expect("Invalid allowed network");
    static ref PROTECTED_NETWORKS: Vec<Ipv4Net> = vec![
        // localhost
        Ipv4Net::new(Ipv4Addr::new(127, 0, 0, 0), 8).expect("Invalid localhost network"),
        // link-local
        Ipv4Net::new(Ipv4Addr::new(169, 254, 0, 0), 16).expect("Invalid link-local network"),
    ];
}

//-------------------------------------------------------------------------------------------------
// Types
//-------------------------------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct Subnet {
    pub cidr: String,
    pub interface: String,
    pub network: Ipv4Net,
}

#[derive(Debug, Deserialize)]
pub struct CreateSubnetRequest {
    pub cidr: String,
}

//-------------------------------------------------------------------------------------------------
// Functions: Validation
//-------------------------------------------------------------------------------------------------

fn validate_network(network: &Ipv4Net) -> Result<()> {
    // Check if network is within allowed range
    if !ALLOWED_NETWORK.contains(&network.addr()) {
        return Err(anyhow!(
            "Network {} is not within allowed range {}",
            network,
            *ALLOWED_NETWORK
        ));
    }

    // Check if network overlaps with protected networks
    for protected in PROTECTED_NETWORKS.iter() {
        // Two networks overlap if either contains the other's network address
        if protected.contains(&network.addr()) || network.contains(&protected.addr()) {
            return Err(anyhow!(
                "Network {} overlaps with protected network {}",
                network,
                protected
            ));
        }
    }

    Ok(())
}

//-------------------------------------------------------------------------------------------------
// Functions: Detection & Configuration
//-------------------------------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub fn detect_existing_subnets() -> Result<Vec<Subnet>> {
    let mut subnets = Vec::new();
    let output = Command::new("ifconfig").arg("lo0").output()?;
    let output_str = String::from_utf8_lossy(&output.stdout);

    tracing::debug!("Parsing ifconfig output:\n{}", output_str);

    // Parse ifconfig output to find aliases
    for line in output_str.lines() {
        if line.trim().starts_with("inet ") && !line.contains("127.0.0.1") {
            tracing::debug!("Found non-localhost inet line: {}", line);
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let ip = parts[1];
                let netmask = parts[3];

                tracing::debug!("Extracted IP: {}, Netmask: {}", ip, netmask);

                // Convert hex netmask to prefix length
                let netmask_hex = netmask.trim_start_matches("0x");
                let netmask_u32 = u32::from_str_radix(netmask_hex, 16)
                    .map_err(|e| anyhow!("Invalid netmask format: {}", e))?;
                let prefix_len = (!netmask_u32).leading_zeros() as u8;

                tracing::debug!(
                    "Converted netmask {} to prefix length {}",
                    netmask,
                    prefix_len
                );

                let cidr = format!("{}/{}", ip, prefix_len);
                tracing::debug!("Constructed CIDR: {}", cidr);

                if let Ok(network) = Ipv4Net::from_str(&cidr) {
                    // Only include networks in the allowed range
                    if ALLOWED_NETWORK.contains(&network.addr()) {
                        tracing::info!("Found subnet: {} on lo0", cidr);
                        subnets.push(Subnet {
                            cidr,
                            interface: "lo0".to_string(),
                            network,
                        });
                    } else {
                        tracing::debug!("Ignoring subnet {} (not in allowed range)", cidr);
                    }
                } else {
                    tracing::warn!("Failed to parse CIDR: {}", cidr);
                }
            }
        }
    }

    tracing::info!("Detected {} subnets", subnets.len());
    Ok(subnets)
}

#[cfg(target_os = "linux")]
pub fn detect_existing_subnets() -> Result<Vec<Subnet>> {
    let mut subnets = Vec::new();
    let output = Command::new("ip").args(["addr", "show"]).output()?;
    let output_str = String::from_utf8_lossy(&output.stdout);

    let mut current_interface = String::new();
    for line in output_str.lines() {
        if line.contains("dummy") {
            // Extract interface name from the line like "3: dummy0:"
            if let Some(name) = line.split_whitespace().nth(1) {
                current_interface = name.trim_end_matches(':').to_string();
            }
        } else if !current_interface.is_empty() && line.trim().starts_with("inet ") {
            // Extract CIDR from lines like "inet 10.0.0.0/24"
            if let Some(cidr) = line.split_whitespace().nth(1) {
                if let Ok(network) = Ipv4Net::from_str(cidr) {
                    // Only include networks in the allowed range
                    if ALLOWED_NETWORK.contains(&network.addr()) {
                        subnets.push(Subnet {
                            cidr: cidr.to_string(),
                            interface: current_interface.clone(),
                            network,
                        });
                    }
                }
            }
        }
    }

    Ok(subnets)
}

pub fn is_subnet_available(network: &Ipv4Net) -> Result<bool> {
    // First validate the network
    validate_network(network)?;

    let interfaces = default_net::get_interfaces();
    for interface in interfaces {
        for addr in interface.ipv4 {
            if network.contains(&addr.addr) {
                return Ok(false);
            }
        }
    }

    Ok(true)
}

#[cfg(target_os = "macos")]
pub fn configure_subnet(cidr: String) -> Result<Subnet> {
    let network = Ipv4Net::from_str(&cidr).map_err(|e| anyhow!("Invalid CIDR format: {}", e))?;

    // Validate the network before proceeding
    validate_network(&network)?;

    if !is_subnet_available(&network)? {
        return Err(anyhow!("Subnet {} is already in use", cidr));
    }

    // Find an available loopback interface alias
    let mut alias_num = 0;
    while alias_num < 255 {
        let interface = format!("lo0:{}", alias_num);
        let status = Command::new("ifconfig").args([&interface]).output()?;

        if !status.status.success() {
            // Interface alias doesn't exist, we can use it
            break;
        }
        alias_num += 1;
    }

    if alias_num == 255 {
        return Err(anyhow!("No available loopback interface aliases"));
    }

    let interface = format!("lo0:{}", alias_num);

    // Configure the interface with the network address
    let status = Command::new("sudo")
        .args([
            "ifconfig",
            "lo0",
            "alias",
            &network.addr().to_string(),
            "netmask",
            &network.netmask().to_string(),
        ])
        .status()?;

    if !status.success() {
        return Err(anyhow!("Failed to configure subnet on {}", interface));
    }

    Ok(Subnet {
        cidr,
        interface,
        network,
    })
}

#[cfg(target_os = "linux")]
pub fn configure_subnet(cidr: String) -> Result<Subnet> {
    let network = Ipv4Net::from_str(&cidr).map_err(|e| anyhow!("Invalid CIDR format: {}", e))?;

    // Validate the network before proceeding
    validate_network(&network)?;

    if !is_subnet_available(&network)? {
        return Err(anyhow!("Subnet {} is already in use", cidr));
    }

    // Find an available dummy interface
    let mut interface_num = 0;
    while interface_num < 255 {
        let interface = format!("dummy{}", interface_num);
        let status = Command::new("ip")
            .args(["link", "show", &interface])
            .output()?;

        if !status.status.success() {
            // Interface doesn't exist, we can create it
            let create_status = Command::new("sudo")
                .args(["ip", "link", "add", &interface, "type", "dummy"])
                .status()?;

            if !create_status.success() {
                return Err(anyhow!("Failed to create dummy interface {}", interface));
            }

            // Configure the interface
            let addr_status = Command::new("sudo")
                .args(["ip", "addr", "add", &cidr, "dev", &interface])
                .status()?;

            if !addr_status.success() {
                return Err(anyhow!("Failed to configure address on {}", interface));
            }

            // Bring up the interface
            let up_status = Command::new("sudo")
                .args(["ip", "link", "set", &interface, "up"])
                .status()?;

            if !up_status.success() {
                return Err(anyhow!("Failed to bring up interface {}", interface));
            }

            return Ok(Subnet {
                cidr,
                interface,
                network,
            });
        }
        interface_num += 1;
    }

    Err(anyhow!("No available dummy interfaces"))
}

#[cfg(target_os = "macos")]
pub fn remove_subnet(subnet: &Subnet) -> Result<()> {
    // Validate the network before proceeding
    validate_network(&subnet.network)?;

    // On macOS, we remove the alias from lo0
    let status = Command::new("sudo")
        .args([
            "ifconfig",
            "lo0",
            "-alias",
            &subnet.network.addr().to_string(),
        ])
        .status()?;

    if !status.success() {
        return Err(anyhow!("Failed to remove subnet from {}", subnet.interface));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn remove_subnet(subnet: &Subnet) -> Result<()> {
    // Validate the network before proceeding
    validate_network(&subnet.network)?;

    // First remove the IP address
    let addr_status = Command::new("sudo")
        .args(["ip", "addr", "del", &subnet.cidr, "dev", &subnet.interface])
        .status()?;

    if !addr_status.success() {
        return Err(anyhow!(
            "Failed to remove address from {}",
            subnet.interface
        ));
    }

    // Then remove the dummy interface
    let del_status = Command::new("sudo")
        .args(["ip", "link", "del", &subnet.interface])
        .status()?;

    if !del_status.success() {
        return Err(anyhow!("Failed to remove interface {}", subnet.interface));
    }

    Ok(())
}
