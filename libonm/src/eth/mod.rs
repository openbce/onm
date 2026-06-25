mod types;

use std::fs;
use std::path::Path;

pub use types::{
    ArpSettings, ConntrackSettings, ConntrackStats, EthError, EthInterface, InterfaceStats,
    LinkState, NetworkStats, NetworkSysctl, RpFilterSettings, SocketBufferSettings, SocketStats,
    SoftnetCpuStats, SoftnetStats, TcpSettings,
};

const SYS_CLASS_NET: &str = "/sys/class/net";
const PROC_SYS: &str = "/proc/sys";
const PROC_NET: &str = "/proc/net";

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

    let txqueuelen = read_sysfs_file(path, "tx_queue_len")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);

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
        txqueuelen,
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

pub fn get_network_stats() -> NetworkStats {
    NetworkStats {
        conntrack: get_conntrack_stats(),
        softnet: get_softnet_stats(),
        sockets: get_socket_stats(),
    }
}

fn get_conntrack_stats() -> ConntrackStats {
    let current = read_proc_file("net/nf_conntrack_count");
    let max = read_sysctl_u64("net.netfilter.nf_conntrack_max");

    let usage_percent = match (current, max) {
        (Some(c), Some(m)) if m > 0 => Some((c as f64 / m as f64) * 100.0),
        _ => None,
    };

    ConntrackStats {
        current,
        max,
        usage_percent,
    }
}

fn get_softnet_stats() -> SoftnetStats {
    let path = Path::new(PROC_NET).join("softnet_stat");
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return SoftnetStats::default(),
    };

    let mut cpus = Vec::new();
    let mut total_processed = 0u64;
    let mut total_dropped = 0u64;
    let mut total_time_squeeze = 0u64;

    for (cpu_id, line) in content.lines().enumerate() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 3 {
            let processed = u64::from_str_radix(fields[0], 16).unwrap_or(0);
            let dropped = u64::from_str_radix(fields[1], 16).unwrap_or(0);
            let time_squeeze = u64::from_str_radix(fields[2], 16).unwrap_or(0);
            let cpu_collision = fields
                .get(3)
                .and_then(|s| u64::from_str_radix(s, 16).ok())
                .unwrap_or(0);
            let received_rps = fields
                .get(4)
                .and_then(|s| u64::from_str_radix(s, 16).ok())
                .unwrap_or(0);
            let flow_limit_count = fields
                .get(5)
                .and_then(|s| u64::from_str_radix(s, 16).ok())
                .unwrap_or(0);

            total_processed += processed;
            total_dropped += dropped;
            total_time_squeeze += time_squeeze;

            cpus.push(SoftnetCpuStats {
                cpu: cpu_id as u32,
                processed,
                dropped,
                time_squeeze,
                cpu_collision,
                received_rps,
                flow_limit_count,
            });
        }
    }

    SoftnetStats {
        cpus,
        total_processed,
        total_dropped,
        total_time_squeeze,
    }
}

fn get_socket_stats() -> SocketStats {
    let path = Path::new(PROC_NET).join("sockstat");
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return SocketStats::default(),
    };

    let mut stats = SocketStats::default();

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "TCP:" => {
                stats.tcp_inuse = find_stat_value(&parts, "inuse");
                stats.tcp_orphan = find_stat_value(&parts, "orphan");
                stats.tcp_tw = find_stat_value(&parts, "tw");
                stats.tcp_alloc = find_stat_value(&parts, "alloc");
                stats.tcp_mem = find_stat_value(&parts, "mem");
            }
            "UDP:" => {
                stats.udp_inuse = find_stat_value(&parts, "inuse");
                stats.udp_mem = find_stat_value(&parts, "mem");
            }
            "RAW:" => {
                stats.raw_inuse = find_stat_value(&parts, "inuse");
            }
            "FRAG:" => {
                stats.frag_inuse = find_stat_value(&parts, "inuse");
                stats.frag_memory = find_stat_value(&parts, "memory");
            }
            _ => {}
        }
    }

    stats
}

fn find_stat_value(parts: &[&str], key: &str) -> Option<u64> {
    for i in 0..parts.len() - 1 {
        if parts[i] == key {
            return parts[i + 1].parse().ok();
        }
    }
    None
}

fn read_proc_file(path: &str) -> Option<u64> {
    let full_path = Path::new("/proc").join(path);
    fs::read_to_string(&full_path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn get_interface_stats(name: &str) -> Result<InterfaceStats, EthError> {
    let path = Path::new(SYS_CLASS_NET).join(name).join("statistics");
    if !path.exists() {
        return Err(EthError::NotFound(name.to_string()));
    }

    Ok(InterfaceStats {
        rx_bytes: read_stat_file(&path, "rx_bytes"),
        rx_packets: read_stat_file(&path, "rx_packets"),
        rx_errors: read_stat_file(&path, "rx_errors"),
        rx_dropped: read_stat_file(&path, "rx_dropped"),
        rx_fifo: read_stat_file(&path, "rx_fifo_errors"),
        rx_frame: read_stat_file(&path, "rx_frame_errors"),
        tx_bytes: read_stat_file(&path, "tx_bytes"),
        tx_packets: read_stat_file(&path, "tx_packets"),
        tx_errors: read_stat_file(&path, "tx_errors"),
        tx_dropped: read_stat_file(&path, "tx_dropped"),
        tx_fifo: read_stat_file(&path, "tx_fifo_errors"),
        tx_carrier: read_stat_file(&path, "tx_carrier_errors"),
        tx_collisions: read_stat_file(&path, "collisions"),
    })
}

fn read_stat_file(base: &Path, file: &str) -> u64 {
    fs::read_to_string(base.join(file))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}
