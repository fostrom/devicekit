use serde::Serialize;
use std::net::IpAddr;
use sysinfo::{NetworkData, Networks};

#[derive(Debug, Clone, Serialize)]
pub struct Network {
    pub name: String,
    pub ip_addrs: Vec<String>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
}

pub fn fetch_networks(networks: &Networks) -> Vec<Network> {
    let mut interfaces = networks
        .iter()
        .filter_map(|(name, data)| {
            let ip_networks = usable_ip_networks(data);
            if ip_networks.is_empty() {
                return None;
            }

            Some(Network {
                name: name.clone(),
                ip_addrs: ip_networks,
                rx_bytes: data.received(),
                tx_bytes: data.transmitted(),
                rx_errors: data.errors_on_received(),
                tx_errors: data.errors_on_transmitted(),
            })
        })
        .collect::<Vec<_>>();

    interfaces.sort_by(|a, b| a.name.cmp(&b.name));
    interfaces
}

pub fn diff_networks(old: &Vec<Network>, new: &mut Vec<Network>) -> Vec<Network> {
    [].to_vec()
}

fn usable_ip_networks(data: &NetworkData) -> Vec<String> {
    let mut ip_networks = data
        .ip_networks()
        .iter()
        .filter(|network| is_usable_ip_addr(network.addr))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    ip_networks.sort();
    ip_networks
}

fn is_usable_ip_addr(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(addr) => {
            let octets = addr.octets();
            addr.is_unspecified()
                || addr.is_loopback()
                || addr.is_multicast()
                || addr.is_broadcast()
                || (octets[0] == 169 && octets[1] == 254)
        }
        IpAddr::V6(addr) => {
            let segments = addr.segments();
            !addr.is_unspecified()
                && !addr.is_loopback()
                && !addr.is_multicast()
                && (segments[0] & 0xffc0) != 0xfe80
        }
    }
}
