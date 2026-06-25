mod types;

use std::fs;
use std::path::Path;

pub use types::{
    ArpSettings, ConntrackSettings, EthError, EthInterface, LinkState, NetworkSysctl,
    RpFilterSettings, SocketBufferSettings, TcpSettings,
};

const SYS_CLASS_NET: &str = "/sys/class/net";
const PROC_SYS: &str = "/proc/sys";

pub fn list_interfaces() -> Result<Vec<EthInterface>, EthError> {
    let mut interfaces = Vec::new();

    let entries = fs::read_dir(SYS_CLASS_NET)?;

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        if name == "lo" {
            continue;
        }

        let iface_path = entry.path();
        if !is_ethernet_device(&iface_path) {
            continue;
        }

        let iface = read_interface(&name, &iface_path)?;
        interfaces.push(iface);
    }

    interfaces.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(interfaces)
}

pub fn get_interface(name: &str) -> Result<EthInterface, EthError> {
    let iface_path = Path::new(SYS_CLASS_NET).join(name);
    if !iface_path.exists() {
        return Err(EthError::NotFound(name.to_string()));
    }

    read_interface(name, &iface_path)
}

fn is_ethernet_device(path: &Path) -> bool {
    let device_path = path.join("device");
    if !device_path.exists() {
        return false;
    }

    let type_path = path.join("type");
    if let Ok(type_content) = fs::read_to_string(&type_path) {
        if let Ok(dev_type) = type_content.trim().parse::<u32>() {
            return dev_type == 1;
        }
    }

    false
}

fn read_interface(name: &str, path: &Path) -> Result<EthInterface, EthError> {
    let mac_address = read_sysfs_file(path, "address").unwrap_or_default();
    let mtu = read_sysfs_file(path, "mtu")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1500);

    let state = match read_sysfs_file(path, "operstate").as_deref() {
        Some("up") => LinkState::Up,
        Some("down") => LinkState::Down,
        _ => LinkState::Unknown,
    };

    let speed = read_sysfs_file(path, "speed")
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&s| s > 0 && s < 1000000);

    let driver = path
        .join("device/driver")
        .read_link()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));

    let pci_slot = path
        .join("device")
        .read_link()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));

    Ok(EthInterface {
        name: name.to_string(),
        mac_address,
        mtu,
        state,
        speed,
        driver,
        pci_slot,
    })
}

fn read_sysfs_file(base: &Path, file: &str) -> Option<String> {
    fs::read_to_string(base.join(file))
        .ok()
        .map(|s| s.trim().to_string())
}

fn read_sysctl(key: &str) -> Option<String> {
    let path = Path::new(PROC_SYS).join(key.replace('.', "/"));
    fs::read_to_string(&path).ok().map(|s| s.trim().to_string())
}

fn read_sysctl_u64(key: &str) -> Option<u64> {
    read_sysctl(key).and_then(|s| s.parse().ok())
}

pub fn get_network_sysctl() -> NetworkSysctl {
    NetworkSysctl {
        conntrack: ConntrackSettings {
            max: read_sysctl_u64("net.netfilter.nf_conntrack_max"),
            buckets: read_sysctl_u64("net.netfilter.nf_conntrack_buckets"),
            tcp_timeout_established: read_sysctl_u64(
                "net.netfilter.nf_conntrack_tcp_timeout_established",
            ),
            tcp_timeout_time_wait: read_sysctl_u64(
                "net.netfilter.nf_conntrack_tcp_timeout_time_wait",
            ),
            tcp_timeout_close_wait: read_sysctl_u64(
                "net.netfilter.nf_conntrack_tcp_timeout_close_wait",
            ),
            tcp_timeout_fin_wait: read_sysctl_u64(
                "net.netfilter.nf_conntrack_tcp_timeout_fin_wait",
            ),
            tcp_max_retrans: read_sysctl_u64("net.netfilter.nf_conntrack_tcp_max_retrans"),
        },
        socket_buffer: SocketBufferSettings {
            rmem_max: read_sysctl_u64("net.core.rmem_max"),
            wmem_max: read_sysctl_u64("net.core.wmem_max"),
            rmem_default: read_sysctl_u64("net.core.rmem_default"),
            wmem_default: read_sysctl_u64("net.core.wmem_default"),
            tcp_rmem: read_sysctl("net.ipv4.tcp_rmem"),
            tcp_wmem: read_sysctl("net.ipv4.tcp_wmem"),
            udp_rmem_min: read_sysctl_u64("net.ipv4.udp_rmem_min"),
            udp_wmem_min: read_sysctl_u64("net.ipv4.udp_wmem_min"),
        },
        tcp: TcpSettings {
            somaxconn: read_sysctl_u64("net.core.somaxconn"),
            max_syn_backlog: read_sysctl_u64("net.ipv4.tcp_max_syn_backlog"),
            tw_reuse: read_sysctl_u64("net.ipv4.tcp_tw_reuse"),
            fin_timeout: read_sysctl_u64("net.ipv4.tcp_fin_timeout"),
            keepalive_time: read_sysctl_u64("net.ipv4.tcp_keepalive_time"),
            keepalive_probes: read_sysctl_u64("net.ipv4.tcp_keepalive_probes"),
            keepalive_intvl: read_sysctl_u64("net.ipv4.tcp_keepalive_intvl"),
            ip_local_port_range: read_sysctl("net.ipv4.ip_local_port_range"),
        },
        arp: ArpSettings {
            gc_thresh1: read_sysctl_u64("net.ipv4.neigh.default.gc_thresh1"),
            gc_thresh2: read_sysctl_u64("net.ipv4.neigh.default.gc_thresh2"),
            gc_thresh3: read_sysctl_u64("net.ipv4.neigh.default.gc_thresh3"),
            arp_ignore: read_sysctl_u64("net.ipv4.conf.all.arp_ignore"),
            arp_announce: read_sysctl_u64("net.ipv4.conf.all.arp_announce"),
        },
        rp_filter: RpFilterSettings {
            all: read_sysctl_u64("net.ipv4.conf.all.rp_filter"),
            default: read_sysctl_u64("net.ipv4.conf.default.rp_filter"),
        },
    }
}
