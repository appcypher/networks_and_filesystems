use anyhow::Result;
use default_net;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use tun::AbstractDevice;

//-------------------------------------------------------------------------------------------------
// Types
//-------------------------------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct TunDevice {
    pub name: String,
    pub ip_addr: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub broadcast: Ipv4Addr,
}

#[derive(Debug, Deserialize)]
pub struct CreateTunRequest {
    pub name: Option<String>,
}

//-------------------------------------------------------------------------------------------------
// Functions
//-------------------------------------------------------------------------------------------------

pub fn find_available_subnet() -> Result<(Ipv4Addr, Ipv4Addr, Ipv4Addr)> {
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

pub fn create_tun_device(name: Option<String>) -> Result<TunDevice> {
    let (ip_addr, netmask, broadcast) = find_available_subnet()?;

    let mut config = tun::Configuration::default();
    if let Some(name) = name.as_ref() {
        config.tun_name(name);
    }

    config
        .address(ip_addr)
        .destination(ip_addr)
        .netmask(netmask)
        .up();

    let dev = tun::create(&config)?;
    let name = dev.tun_name()?;

    Ok(TunDevice {
        name,
        ip_addr,
        netmask,
        broadcast,
    })
}
