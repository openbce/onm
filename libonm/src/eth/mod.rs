mod nat;
mod types;

use futures::TryStreamExt;
use std::fs;
use std::path::Path;
use std::time::Duration;

pub use nat::get_nat_rules;
pub use types::{
    ArpSettings, ConntrackSettings, ConntrackStats, EthError, EthInterface, EthtoolCoalesce,
    EthtoolOffload, EthtoolRing, EthtoolSettings, ForwardingSettings, InterfaceStats,
    InterfaceType, KubeProxyStats, LinkSettings, LinkState, NatRule, NatTable, NatType,
    NeighborStats, NetworkStats, NetworkSysctl, RouteEntry, RouteProtocol, RouteScope, RouteTable,
    RouteType, RpFilterSettings, SocketBufferSettings, SocketStats, SoftnetCpuStats, SoftnetStats,
    TcpSettings, UdpSettings,
};

const SYS_CLASS_NET: &str = "/sys/class/net";
const PROC_SYS: &str = "/proc/sys";
const PROC_NET: &str = "/proc/net";
const NETLINK_TIMEOUT: Duration = Duration::from_secs(5);

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
        if !is_network_device(&iface_path) {
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

fn is_network_device(path: &Path) -> bool {
    let type_path = path.join("type");
    if let Ok(type_content) = fs::read_to_string(&type_path) {
        if let Ok(dev_type) = type_content.trim().parse::<u32>() {
            return matches!(
                dev_type,
                1 |      // ARPHRD_ETHER - Ethernet
                65534 |  // ARPHRD_NONE - tun/wireguard/tailscale
                768 |    // ARPHRD_TUNNEL - ipip
                769 |    // ARPHRD_TUNNEL6 - ip6tnl
                776 |    // ARPHRD_SIT - sit (IPv6-in-IPv4)
                778 |    // ARPHRD_IPGRE - gre
                823 // ARPHRD_IP6GRE - ip6gre
            );
        }
    }

    false
}

fn get_interface_type(path: &Path, name: &str, kind: Option<&str>) -> InterfaceType {
    let device_path = path.join("device");
    let has_physical_device = device_path.exists()
        && device_path
            .canonicalize()
            .map(|resolved| !resolved.to_string_lossy().contains("/virtual/"))
            .unwrap_or(true);
    let arphrd = read_sysfs_file(path, "type").and_then(|value| value.parse::<u32>().ok());

    classify_interface_type(name, kind, has_physical_device, arphrd)
}

fn classify_interface_type(
    name: &str,
    kind: Option<&str>,
    has_physical_device: bool,
    arphrd: Option<u32>,
) -> InterfaceType {
    match kind {
        Some("veth") => return InterfaceType::Veth,
        Some("bridge") => return InterfaceType::Bridge,
        Some("bond") => return InterfaceType::Bond,
        Some("vlan") => return InterfaceType::Vlan,
        Some("vxlan") => return InterfaceType::Vxlan,
        Some("wireguard") => return InterfaceType::WireGuard,
        Some("tun" | "tap") => return InterfaceType::Tun,
        Some("macvlan") => return InterfaceType::Macvlan,
        Some("ipvlan") => return InterfaceType::Ipvlan,
        Some("dummy") => return InterfaceType::Dummy,
        Some("gre" | "gretap" | "ipip" | "sit" | "ip6tnl" | "geneve" | "erspan") => {
            return InterfaceType::Tunnel;
        }
        _ => {}
    }

    if name.starts_with("veth") || name.starts_with("cali") {
        return InterfaceType::Veth;
    }
    if name.starts_with("vxlan") {
        return InterfaceType::Vxlan;
    }
    if name.starts_with("wg") {
        return InterfaceType::WireGuard;
    }
    if name.starts_with("tun") || name.starts_with("tap") || name.starts_with("tailscale") {
        return InterfaceType::Tun;
    }
    if name.starts_with("macvlan") {
        return InterfaceType::Macvlan;
    }
    if name.starts_with("ipvlan") {
        return InterfaceType::Ipvlan;
    }
    if name.starts_with("dummy") {
        return InterfaceType::Dummy;
    }

    match arphrd {
        Some(772) => return InterfaceType::Loopback,
        Some(768 | 769 | 776 | 778 | 823) => return InterfaceType::Tunnel,
        _ => {}
    }

    if has_physical_device {
        InterfaceType::Physical
    } else {
        InterfaceType::Virtual
    }
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

    let duplex = read_sysfs_file(path, "duplex");

    let carrier = read_sysfs_file(path, "carrier")
        .and_then(|s| s.parse::<u32>().ok())
        .map(|c| c == 1);

    let numa_node =
        read_sysfs_file(&path.join("device"), "numa_node").and_then(|s| s.parse::<i32>().ok());

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

    let master = path
        .join("master")
        .read_link()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));

    let kind = detect_interface_kind(path, name).or_else(|| read_uevent_value(path, "DEVTYPE"));
    let interface_type = get_interface_type(path, name, kind.as_deref());

    Ok(EthInterface {
        name: name.to_string(),
        mac_address,
        mtu,
        txqueuelen,
        state,
        speed,
        duplex,
        carrier,
        numa_node,
        driver,
        pci_slot,
        interface_type,
        addresses: Vec::new(),
        master,
        kind,
    })
}

fn detect_interface_kind(path: &Path, name: &str) -> Option<String> {
    if path.join("brif").exists() {
        return Some("bridge".to_string());
    }
    if path.join("bonding").exists() {
        return Some("bond".to_string());
    }
    if path.join("brport").exists() {
        return Some("bridge_slave".to_string());
    }
    if path.join("bonding_slave").exists() {
        return Some("bond_slave".to_string());
    }
    if Path::new("/proc/net/vlan").join(name).exists() {
        return Some("vlan".to_string());
    }
    if name.starts_with("veth") || name.starts_with("cali") {
        return Some("veth".to_string());
    }
    if name.starts_with("vxlan") {
        return Some("vxlan".to_string());
    }
    if name.starts_with("wg") {
        return Some("wireguard".to_string());
    }
    if name.starts_with("tun") || name.starts_with("tap") || name.starts_with("tailscale") {
        return Some("tun".to_string());
    }
    if name.starts_with("macvlan") {
        return Some("macvlan".to_string());
    }
    if name.starts_with("ipvlan") {
        return Some("ipvlan".to_string());
    }
    if name.starts_with("dummy") {
        return Some("dummy".to_string());
    }
    None
}

fn read_sysfs_file(base: &Path, file: &str) -> Option<String> {
    fs::read_to_string(base.join(file))
        .ok()
        .map(|s| s.trim().to_string())
}

fn read_uevent_value(path: &Path, key: &str) -> Option<String> {
    let content = fs::read_to_string(path.join("uevent")).ok()?;
    for line in content.lines() {
        if let Some((k, v)) = line.split_once('=') {
            if k == key {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn read_sysctl(key: &str) -> Option<String> {
    let path = Path::new(PROC_SYS).join(key.replace('.', "/"));
    fs::read_to_string(&path).ok().map(|s| s.trim().to_string())
}

fn read_sysctl_u64(key: &str) -> Option<u64> {
    read_sysctl(key).and_then(|s| s.parse().ok())
}

fn get_interface_rp_filters() -> Vec<(String, u64)> {
    let path = Path::new(PROC_SYS).join("net/ipv4/conf");
    let mut values = fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if matches!(name.as_str(), "all" | "default") {
                return None;
            }
            let value = fs::read_to_string(entry.path().join("rp_filter"))
                .ok()?
                .trim()
                .parse()
                .ok()?;
            Some((name, value))
        })
        .collect::<Vec<_>>();
    values.sort_by(|a, b| a.0.cmp(&b.0));
    values
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
            udp_timeout: read_sysctl_u64("net.netfilter.nf_conntrack_udp_timeout"),
            udp_timeout_stream: read_sysctl_u64("net.netfilter.nf_conntrack_udp_timeout_stream"),
        },
        socket_buffer: SocketBufferSettings {
            rmem_max: read_sysctl_u64("net.core.rmem_max"),
            wmem_max: read_sysctl_u64("net.core.wmem_max"),
            rmem_default: read_sysctl_u64("net.core.rmem_default"),
            wmem_default: read_sysctl_u64("net.core.wmem_default"),
            tcp_rmem: read_sysctl("net.ipv4.tcp_rmem"),
            tcp_wmem: read_sysctl("net.ipv4.tcp_wmem"),
            netdev_max_backlog: read_sysctl_u64("net.core.netdev_max_backlog"),
            netdev_budget: read_sysctl_u64("net.core.netdev_budget"),
            netdev_budget_usecs: read_sysctl_u64("net.core.netdev_budget_usecs"),
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
            ip_local_reserved_ports: read_sysctl("net.ipv4.ip_local_reserved_ports"),
        },
        udp: UdpSettings {
            rmem_min: read_sysctl_u64("net.ipv4.udp_rmem_min"),
            wmem_min: read_sysctl_u64("net.ipv4.udp_wmem_min"),
            udp_mem: read_sysctl("net.ipv4.udp_mem"),
        },
        arp: ArpSettings {
            gc_thresh1: read_sysctl_u64("net.ipv4.neigh.default.gc_thresh1"),
            gc_thresh2: read_sysctl_u64("net.ipv4.neigh.default.gc_thresh2"),
            gc_thresh3: read_sysctl_u64("net.ipv4.neigh.default.gc_thresh3"),
            arp_ignore: read_sysctl_u64("net.ipv4.conf.all.arp_ignore"),
            arp_announce: read_sysctl_u64("net.ipv4.conf.all.arp_announce"),
            ipv6_gc_thresh1: read_sysctl_u64("net.ipv6.neigh.default.gc_thresh1"),
            ipv6_gc_thresh2: read_sysctl_u64("net.ipv6.neigh.default.gc_thresh2"),
            ipv6_gc_thresh3: read_sysctl_u64("net.ipv6.neigh.default.gc_thresh3"),
        },
        rp_filter: RpFilterSettings {
            all: read_sysctl_u64("net.ipv4.conf.all.rp_filter"),
            default: read_sysctl_u64("net.ipv4.conf.default.rp_filter"),
            interfaces: get_interface_rp_filters(),
        },
        forwarding: ForwardingSettings {
            ipv4: read_sysctl_u64("net.ipv4.ip_forward"),
            ipv6: read_sysctl_u64("net.ipv6.conf.all.forwarding"),
        },
    }
}

pub fn get_network_stats() -> NetworkStats {
    NetworkStats {
        conntrack: get_conntrack_stats(),
        softnet: get_softnet_stats(),
        sockets: get_socket_stats(),
        neighbors: get_neighbor_stats(),
        kube_proxy: get_kube_proxy_stats(),
    }
}

fn get_conntrack_stats() -> ConntrackStats {
    let current = read_sysctl_u64("net.netfilter.nf_conntrack_count");
    let max = read_sysctl_u64("net.netfilter.nf_conntrack_max");
    let buckets = read_sysctl_u64("net.netfilter.nf_conntrack_buckets");

    let usage_percent = match (current, max) {
        (Some(c), Some(m)) if m > 0 => Some((c as f64 / m as f64) * 100.0),
        _ => None,
    };
    let entries_per_bucket = match (current, buckets) {
        (Some(entries), Some(bucket_count)) if bucket_count > 0 => {
            Some(entries as f64 / bucket_count as f64)
        }
        _ => None,
    };
    let (insert_failed, drop, early_drop) =
        fs::read_to_string(Path::new(PROC_NET).join("stat/nf_conntrack"))
            .ok()
            .map(|content| parse_conntrack_counters(&content))
            .unwrap_or((None, None, None));

    ConntrackStats {
        current,
        max,
        buckets,
        usage_percent,
        entries_per_bucket,
        insert_failed,
        drop,
        early_drop,
    }
}

fn parse_conntrack_counters(content: &str) -> (Option<u64>, Option<u64>, Option<u64>) {
    let mut lines = content.lines();
    let Some(header) = lines.next() else {
        return (None, None, None);
    };
    let columns = header.split_whitespace().collect::<Vec<_>>();
    let index = |name: &str| columns.iter().position(|column| *column == name);
    let insert_failed_index = index("insert_failed");
    let drop_index = index("drop");
    let early_drop_index = index("early_drop");

    let mut insert_failed = insert_failed_index.map(|_| 0u64);
    let mut drop = drop_index.map(|_| 0u64);
    let mut early_drop = early_drop_index.map(|_| 0u64);
    for line in lines {
        let values = line.split_whitespace().collect::<Vec<_>>();
        let add = |target: &mut Option<u64>, column: Option<usize>| {
            if let (Some(total), Some(value)) = (
                target.as_mut(),
                column
                    .and_then(|position| values.get(position))
                    .and_then(|value| u64::from_str_radix(value, 16).ok()),
            ) {
                *total = total.saturating_add(value);
            }
        };
        add(&mut insert_failed, insert_failed_index);
        add(&mut drop, drop_index);
        add(&mut early_drop, early_drop_index);
    }
    (insert_failed, drop, early_drop)
}

fn get_softnet_stats() -> SoftnetStats {
    let path = Path::new(PROC_NET).join("softnet_stat");
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return SoftnetStats::default(),
    };

    parse_softnet_stats(&content)
}

fn parse_softnet_stats(content: &str) -> SoftnetStats {
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
            // Since Linux 2.6.23 these values occupy columns 8-10.  Columns
            // 3-7 are internal fields which must not be presented as these
            // public counters.
            let cpu_collision = fields
                .get(8)
                .and_then(|s| u64::from_str_radix(s, 16).ok())
                .unwrap_or(0);
            let received_rps = fields
                .get(9)
                .and_then(|s| u64::from_str_radix(s, 16).ok())
                .unwrap_or(0);
            let flow_limit_count = fields
                .get(10)
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

    if let Ok(netstat) = fs::read_to_string(Path::new(PROC_NET).join("netstat")) {
        stats.listen_overflows = parse_netstat_value(&netstat, "TcpExt:", "ListenOverflows");
        stats.listen_drops = parse_netstat_value(&netstat, "TcpExt:", "ListenDrops");
        stats.req_q_full_drop = parse_netstat_value(&netstat, "TcpExt:", "TCPReqQFullDrop");
        stats.req_q_full_do_cookies =
            parse_netstat_value(&netstat, "TcpExt:", "TCPReqQFullDoCookies");
        stats.abort_on_memory = parse_netstat_value(&netstat, "TcpExt:", "TCPAbortOnMemory");
        stats.time_wait_overflow = parse_netstat_value(&netstat, "TcpExt:", "TCPTimeWaitOverflow");
    }

    stats
}

fn get_neighbor_stats() -> NeighborStats {
    use std::process::Command;

    let mut stats = NeighborStats::default();
    for (family, is_ipv6) in ["-4", "-6"].into_iter().zip([false, true]) {
        let Ok(output) = Command::new("ip")
            .args(["-j", family, "neighbor", "show"])
            .output()
        else {
            continue;
        };
        if output.status.success() {
            parse_neighbor_json(
                &String::from_utf8_lossy(&output.stdout),
                is_ipv6,
                &mut stats,
            );
        }
    }
    stats
}

fn parse_neighbor_json(content: &str, is_ipv6: bool, stats: &mut NeighborStats) {
    let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(content) else {
        return;
    };

    for entry in entries {
        if is_ipv6 {
            stats.ipv6_total += 1;
        } else {
            stats.ipv4_total += 1;
        }

        let states = entry
            .get("state")
            .and_then(|state| state.as_array())
            .into_iter()
            .flatten()
            .filter_map(|state| state.as_str());
        for state in states {
            match state.to_ascii_uppercase().as_str() {
                "REACHABLE" => stats.reachable += 1,
                "STALE" => stats.stale += 1,
                "INCOMPLETE" => stats.incomplete += 1,
                "FAILED" => stats.failed += 1,
                _ => {}
            }
        }
    }
}

fn get_kube_proxy_stats() -> KubeProxyStats {
    use std::process::Command;

    let nft4 = Command::new("nft")
        .args(["-j", "list", "table", "ip", "kube-proxy"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| count_nft_rules(&String::from_utf8_lossy(&output.stdout)));
    let nft6 = Command::new("nft")
        .args(["-j", "list", "table", "ip6", "kube-proxy"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| count_nft_rules(&String::from_utf8_lossy(&output.stdout)));

    if nft4.is_some() || nft6.is_some() {
        return KubeProxyStats {
            mode: Some("nftables".to_string()),
            ipv4_rules: nft4.unwrap_or(0),
            ipv6_rules: nft6.unwrap_or(0),
        };
    }

    let iptables_rules = count_command_rules("iptables");
    let ip6tables_rules = count_command_rules("ip6tables");
    if iptables_rules > 0 || ip6tables_rules > 0 {
        return KubeProxyStats {
            mode: Some("iptables".to_string()),
            ipv4_rules: iptables_rules,
            ipv6_rules: ip6tables_rules,
        };
    }

    let ipvs_active = fs::read_to_string(Path::new(PROC_NET).join("ip_vs"))
        .map(|content| {
            content
                .lines()
                .any(|line| line.starts_with("TCP") || line.starts_with("UDP"))
        })
        .unwrap_or(false);
    KubeProxyStats {
        mode: ipvs_active.then(|| "ipvs".to_string()),
        ..KubeProxyStats::default()
    }
}

fn count_command_rules(command: &str) -> u64 {
    use std::process::Command;

    Command::new(command)
        .args(["-t", "nat", "-S"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| line.contains("KUBE-") && line.starts_with("-A "))
                .count() as u64
        })
        .unwrap_or(0)
}

fn count_nft_rules(content: &str) -> u64 {
    serde_json::from_str::<serde_json::Value>(content)
        .ok()
        .and_then(|root| {
            root.get("nftables")
                .and_then(|items| items.as_array())
                .cloned()
        })
        .map(|items| {
            items
                .iter()
                .filter(|item| item.get("rule").is_some())
                .count() as u64
        })
        .unwrap_or(0)
}

fn parse_netstat_value(content: &str, section: &str, key: &str) -> Option<u64> {
    let mut lines = content.lines();
    while let Some(header) = lines.next() {
        let Some(values) = lines.next() else {
            break;
        };
        if !header.starts_with(section) || !values.starts_with(section) {
            continue;
        }
        let keys = header.split_whitespace().skip(1).collect::<Vec<_>>();
        let values = values.split_whitespace().skip(1).collect::<Vec<_>>();
        let position = keys.iter().position(|candidate| *candidate == key)?;
        return values.get(position)?.parse().ok();
    }
    None
}

fn find_stat_value(parts: &[&str], key: &str) -> Option<u64> {
    for i in 0..parts.len() - 1 {
        if parts[i] == key {
            return parts[i + 1].parse().ok();
        }
    }
    None
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
        rx_missed: read_stat_file(&path, "rx_missed_errors"),
        rx_nohandler: read_stat_file(&path, "rx_nohandler"),
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

pub async fn get_ethtool_settings(name: &str) -> Result<EthtoolSettings, EthError> {
    use ethtool::{EthtoolAttr, EthtoolCoalesceAttr, EthtoolFeatureAttr, EthtoolRingAttr};

    let (conn, mut handle, _) = ethtool::new_connection()
        .map_err(|e| EthError::Internal(format!("Failed to create ethtool connection: {}", e)))?;
    let conn_handle = tokio::spawn(conn);

    let result = tokio::time::timeout(NETLINK_TIMEOUT, async {
        let mut settings = EthtoolSettings::default();

        let mut rings = handle.ring().get(Some(name)).execute().await;
        while let Some(msg) = rings.try_next().await.map_err(|e| {
            EthError::Internal(format!(
                "failed to query ethtool ring settings for {name}: {e}"
            ))
        })? {
            for attr in msg.payload.nlas {
                if let EthtoolAttr::Ring(ring_attr) = attr {
                    match ring_attr {
                        EthtoolRingAttr::RxMax(v) => settings.ring.rx_max = Some(v),
                        EthtoolRingAttr::Rx(v) => settings.ring.rx = Some(v),
                        EthtoolRingAttr::TxMax(v) => settings.ring.tx_max = Some(v),
                        EthtoolRingAttr::Tx(v) => settings.ring.tx = Some(v),
                        _ => {}
                    }
                }
            }
        }

        let mut coalesces = handle.coalesce().get(Some(name)).execute().await;
        while let Some(msg) = coalesces.try_next().await.map_err(|e| {
            EthError::Internal(format!(
                "failed to query ethtool coalesce settings for {name}: {e}"
            ))
        })? {
            for attr in msg.payload.nlas {
                if let EthtoolAttr::Coalesce(coalesce_attr) = attr {
                    match coalesce_attr {
                        EthtoolCoalesceAttr::RxUsecs(v) => settings.coalesce.rx_usecs = Some(v),
                        EthtoolCoalesceAttr::TxUsecs(v) => settings.coalesce.tx_usecs = Some(v),
                        _ => {}
                    }
                }
            }
        }

        let mut features = handle.feature().get(Some(name)).execute().await;
        while let Some(msg) = features.try_next().await.map_err(|e| {
            EthError::Internal(format!("failed to query ethtool features for {name}: {e}"))
        })? {
            for attr in msg.payload.nlas {
                if let EthtoolAttr::Feature(EthtoolFeatureAttr::Active(bits)) = attr {
                    for bit in bits {
                        match bit.name.as_str() {
                            "tx-tcp-segmentation" => settings.offload.tso = Some(bit.value),
                            "tx-generic-segmentation" => settings.offload.gso = Some(bit.value),
                            "rx-gro" => settings.offload.gro = Some(bit.value),
                            _ => {}
                        }
                    }
                }
            }
        }

        Ok::<_, EthError>(settings)
    })
    .await;

    conn_handle.abort();

    match result {
        Ok(settings) => settings,
        Err(_) => Err(EthError::Internal(format!(
            "ethtool query timed out for {}",
            name
        ))),
    }
}

pub async fn get_link_settings(name: &str) -> Result<LinkSettings, EthError> {
    use netlink_packet_route::link::LinkAttribute;

    let (conn, handle, _) = rtnetlink::new_connection()
        .map_err(|e| EthError::Internal(format!("Failed to create rtnetlink connection: {}", e)))?;
    let conn_handle = tokio::spawn(conn);

    let result = tokio::time::timeout(NETLINK_TIMEOUT, async {
        let mut settings = LinkSettings::default();

        let mut links = handle.link().get().match_name(name.to_string()).execute();
        if let Some(link) = links.try_next().await.map_err(|e| {
            EthError::Internal(format!("failed to query link settings for {name}: {e}"))
        })? {
            for attr in link.attributes {
                match attr {
                    LinkAttribute::Mtu(v) => settings.mtu = Some(v),
                    LinkAttribute::MinMtu(v) => settings.min_mtu = Some(v),
                    LinkAttribute::MaxMtu(v) => settings.max_mtu = Some(v),
                    LinkAttribute::TxQueueLen(v) => settings.txqueuelen = Some(v),
                    LinkAttribute::NumTxQueues(v) => settings.num_tx_queues = Some(v),
                    LinkAttribute::NumRxQueues(v) => settings.num_rx_queues = Some(v),
                    LinkAttribute::GsoMaxSize(v) => settings.gso_max_size = Some(v),
                    LinkAttribute::GsoMaxSegs(v) => settings.gso_max_segs = Some(v),
                    LinkAttribute::GroMaxSize(v) => settings.gro_max_size = Some(v),
                    LinkAttribute::TsoMaxSize(v) => settings.tso_max_size = Some(v),
                    LinkAttribute::TsoMaxSegs(v) => settings.tso_max_segs = Some(v),
                    LinkAttribute::Qdisc(v) => settings.qdisc = Some(v),
                    LinkAttribute::Group(v) => settings.group = Some(v),
                    _ => {}
                }
            }
        } else {
            return Err(EthError::NotFound(name.to_string()));
        }

        Ok::<_, EthError>(settings)
    })
    .await;

    conn_handle.abort();

    match result {
        Ok(settings) => settings,
        Err(_) => Err(EthError::Internal(format!(
            "rtnetlink query timed out for {}",
            name
        ))),
    }
}

pub async fn get_interface_addresses(name: &str) -> Result<Vec<String>, EthError> {
    use netlink_packet_route::address::AddressAttribute;

    let (conn, handle, _) = rtnetlink::new_connection()
        .map_err(|e| EthError::Internal(format!("Failed to create rtnetlink connection: {}", e)))?;
    let conn_handle = tokio::spawn(conn);

    let ifindex = get_interface_index(name).ok_or_else(|| EthError::NotFound(name.to_string()))?;

    let result =
        tokio::time::timeout(NETLINK_TIMEOUT, async {
            let mut addresses = Vec::new();

            let mut addr_stream = handle.address().get().execute();
            while let Some(msg) = addr_stream.try_next().await.map_err(|e| {
                EthError::Internal(format!("failed to query addresses for {name}: {e}"))
            })? {
                if msg.header.index != ifindex {
                    continue;
                }

                let prefix_len = msg.header.prefix_len;
                for attr in &msg.attributes {
                    match attr {
                        AddressAttribute::Address(addr) => {
                            addresses.push(format!("{}/{}", addr, prefix_len));
                        }
                        _ => {}
                    }
                }
            }

            Ok::<_, EthError>(addresses)
        })
        .await;

    conn_handle.abort();

    match result {
        Ok(addrs) => addrs,
        Err(_) => Err(EthError::Internal(format!(
            "rtnetlink address query timed out for {}",
            name
        ))),
    }
}

fn get_interface_index(name: &str) -> Option<u32> {
    let path = Path::new(SYS_CLASS_NET).join(name).join("ifindex");
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub async fn get_routes() -> Result<RouteTable, EthError> {
    use netlink_packet_route::route::{
        RouteAddress, RouteAttribute, RouteProtocol as RtProto, RouteScope as RtScope,
        RouteType as RtType,
    };
    use std::net::Ipv4Addr;

    let (conn, handle, _) = rtnetlink::new_connection()
        .map_err(|e| EthError::Internal(format!("Failed to create rtnetlink connection: {}", e)))?;
    let conn_handle = tokio::spawn(conn);

    let result = tokio::time::timeout(NETLINK_TIMEOUT, async {
        let mut table = RouteTable::default();

        let ipv4_msg = rtnetlink::RouteMessageBuilder::<Ipv4Addr>::new().build();
        let mut ipv4_routes = handle.route().get(ipv4_msg).execute();
        while let Some(route) = ipv4_routes
            .try_next()
            .await
            .map_err(|e| EthError::Internal(format!("failed to query IPv4 routes: {e}")))?
        {
            let header = &route.header;
            let mut entry = RouteEntry {
                prefix_len: header.destination_prefix_length,
                table: header.table,
                scope: match header.scope {
                    RtScope::Universe => RouteScope::Universe,
                    RtScope::Site => RouteScope::Site,
                    RtScope::Link => RouteScope::Link,
                    RtScope::Host => RouteScope::Host,
                    RtScope::NoWhere => RouteScope::Nowhere,
                    RtScope::Other(v) => RouteScope::Unknown(v),
                    _ => RouteScope::Unknown(0),
                },
                route_type: match header.kind {
                    RtType::Unicast => RouteType::Unicast,
                    RtType::Local => RouteType::Local,
                    RtType::Broadcast => RouteType::Broadcast,
                    RtType::Anycast => RouteType::Anycast,
                    RtType::Multicast => RouteType::Multicast,
                    RtType::BlackHole => RouteType::Blackhole,
                    RtType::Unreachable => RouteType::Unreachable,
                    RtType::Prohibit => RouteType::Prohibit,
                    RtType::Throw => RouteType::Throw,
                    RtType::Nat => RouteType::Nat,
                    RtType::Unspec => RouteType::Unknown(0),
                    RtType::ExternalResolve => RouteType::Unknown(11),
                    RtType::Other(v) => RouteType::Unknown(v),
                    _ => RouteType::Unknown(0),
                },
                protocol: match header.protocol {
                    RtProto::Unspec => RouteProtocol::Unspec,
                    RtProto::Kernel => RouteProtocol::Kernel,
                    RtProto::Boot => RouteProtocol::Boot,
                    RtProto::Static => RouteProtocol::Static,
                    RtProto::Dhcp => RouteProtocol::Dhcp,
                    RtProto::Ra => RouteProtocol::Ra,
                    RtProto::Other(v) => RouteProtocol::Unknown(v),
                    _ => RouteProtocol::Unknown(0),
                },
                ..Default::default()
            };

            for attr in &route.attributes {
                match attr {
                    RouteAttribute::Destination(addr) => {
                        if let RouteAddress::Inet(v4) = addr {
                            entry.destination = format!("{}/{}", v4, entry.prefix_len);
                        }
                    }
                    RouteAttribute::Gateway(addr) => {
                        if let RouteAddress::Inet(v4) = addr {
                            entry.gateway = Some(v4.to_string());
                        }
                    }
                    RouteAttribute::Oif(idx) => {
                        entry.interface = get_interface_name_by_index(*idx);
                    }
                    RouteAttribute::Priority(p) => {
                        entry.metric = Some(*p);
                    }
                    _ => {}
                }
            }

            if entry.destination.is_empty() {
                entry.destination = format!("0.0.0.0/{}", entry.prefix_len);
            }

            table.ipv4.push(entry);
        }

        let ipv6_msg = rtnetlink::RouteMessageBuilder::<std::net::Ipv6Addr>::new().build();
        let mut ipv6_routes = handle.route().get(ipv6_msg).execute();
        while let Some(route) = ipv6_routes
            .try_next()
            .await
            .map_err(|e| EthError::Internal(format!("failed to query IPv6 routes: {e}")))?
        {
            let header = &route.header;
            let mut entry = RouteEntry {
                prefix_len: header.destination_prefix_length,
                table: header.table,
                scope: match header.scope {
                    RtScope::Universe => RouteScope::Universe,
                    RtScope::Site => RouteScope::Site,
                    RtScope::Link => RouteScope::Link,
                    RtScope::Host => RouteScope::Host,
                    RtScope::NoWhere => RouteScope::Nowhere,
                    RtScope::Other(v) => RouteScope::Unknown(v),
                    _ => RouteScope::Unknown(0),
                },
                route_type: match header.kind {
                    RtType::Unicast => RouteType::Unicast,
                    RtType::Local => RouteType::Local,
                    RtType::Broadcast => RouteType::Broadcast,
                    RtType::Anycast => RouteType::Anycast,
                    RtType::Multicast => RouteType::Multicast,
                    RtType::BlackHole => RouteType::Blackhole,
                    RtType::Unreachable => RouteType::Unreachable,
                    RtType::Prohibit => RouteType::Prohibit,
                    RtType::Throw => RouteType::Throw,
                    RtType::Nat => RouteType::Nat,
                    RtType::Unspec => RouteType::Unknown(0),
                    RtType::ExternalResolve => RouteType::Unknown(11),
                    RtType::Other(v) => RouteType::Unknown(v),
                    _ => RouteType::Unknown(0),
                },
                protocol: match header.protocol {
                    RtProto::Unspec => RouteProtocol::Unspec,
                    RtProto::Kernel => RouteProtocol::Kernel,
                    RtProto::Boot => RouteProtocol::Boot,
                    RtProto::Static => RouteProtocol::Static,
                    RtProto::Dhcp => RouteProtocol::Dhcp,
                    RtProto::Ra => RouteProtocol::Ra,
                    RtProto::Other(v) => RouteProtocol::Unknown(v),
                    _ => RouteProtocol::Unknown(0),
                },
                ..Default::default()
            };

            for attr in &route.attributes {
                match attr {
                    RouteAttribute::Destination(addr) => {
                        if let RouteAddress::Inet6(v6) = addr {
                            entry.destination = format!("{}/{}", v6, entry.prefix_len);
                        }
                    }
                    RouteAttribute::Gateway(addr) => {
                        if let RouteAddress::Inet6(v6) = addr {
                            entry.gateway = Some(v6.to_string());
                        }
                    }
                    RouteAttribute::Oif(idx) => {
                        entry.interface = get_interface_name_by_index(*idx);
                    }
                    RouteAttribute::Priority(p) => {
                        entry.metric = Some(*p);
                    }
                    _ => {}
                }
            }

            if entry.destination.is_empty() {
                entry.destination = format!("::/{}", entry.prefix_len);
            }

            table.ipv6.push(entry);
        }

        Ok::<_, EthError>(table)
    })
    .await;

    conn_handle.abort();

    match result {
        Ok(table) => table,
        Err(_) => Err(EthError::Internal(
            "rtnetlink route query timed out".to_string(),
        )),
    }
}

fn get_interface_name_by_index(index: u32) -> Option<String> {
    let entries = fs::read_dir(SYS_CLASS_NET).ok()?;
    for entry in entries.flatten() {
        let path = entry.path().join("ifindex");
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(idx) = content.trim().parse::<u32>() {
                if idx == index {
                    return Some(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

fn same_nat_rule(left: &NatRule, right: &NatRule) -> bool {
    left.family == right.family
        && left.chain == right.chain
        && left.nat_type == right.nat_type
        && left.source == right.source
        && left.destination == right.destination
        && left.protocol == right.protocol
        && left.dport == right.dport
        && left.sport == right.sport
        && left.to_source == right.to_source
        && left.to_destination == right.to_destination
        && left.interface_in == right.interface_in
        && left.interface_out == right.interface_out
}

/// Parse nftables JSON output to extract NAT rules.
///
/// The JSON format follows the libnftables-json schema where rules contain
/// expressions (expr) that may include NAT statements like snat, dnat, or masquerade.
fn parse_nftables_json(json_str: &str, table: &mut NatTable) -> Result<(), EthError> {
    use serde_json::Value;

    let root: Value = serde_json::from_str(json_str)
        .map_err(|e| EthError::Internal(format!("Invalid nftables JSON: {}", e)))?;

    let nftables = root
        .get("nftables")
        .and_then(|v| v.as_array())
        .ok_or_else(|| EthError::Internal("Missing 'nftables' array in JSON".to_string()))?;

    // First pass: collect chain information (for hook/type context)
    let mut chain_info: std::collections::HashMap<(String, String, String), NftChainInfo> =
        std::collections::HashMap::new();
    let mut table_chains: std::collections::HashMap<
        (String, String),
        std::collections::HashSet<String>,
    > = std::collections::HashMap::new();
    let mut nat_tables = std::collections::HashSet::new();

    for item in nftables {
        if let Some(chain) = item.get("chain") {
            let family = chain.get("family").and_then(|v| v.as_str()).unwrap_or("");
            let table_name = chain.get("table").and_then(|v| v.as_str()).unwrap_or("");
            let chain_name = chain.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let chain_type = chain.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let hook = chain.get("hook").and_then(|v| v.as_str()).unwrap_or("");

            if chain_type == "nat" {
                nat_tables.insert((family.to_string(), table_name.to_string()));
            }

            chain_info.insert(
                (
                    family.to_string(),
                    table_name.to_string(),
                    chain_name.to_string(),
                ),
                NftChainInfo {
                    chain_type: chain_type.to_string(),
                    hook: hook.to_string(),
                },
            );
            table_chains
                .entry((family.to_string(), table_name.to_string()))
                .or_default()
                .insert(chain_name.to_string());
        }
    }

    // Second pass: extract rules with NAT statements
    for item in nftables {
        if let Some(rule) = item.get("rule") {
            let family = rule.get("family").and_then(|v| v.as_str()).unwrap_or("");
            let table_name = rule.get("table").and_then(|v| v.as_str()).unwrap_or("");
            let chain_name = rule.get("chain").and_then(|v| v.as_str()).unwrap_or("");

            // `nft -j list ruleset` includes filter, route, and other tables.
            // A jump is relevant here only when it belongs to a table that has
            // a NAT base chain.
            if !nat_tables.contains(&(family.to_string(), table_name.to_string())) {
                continue;
            }

            // Get chain info to check if this is a NAT chain
            let info = chain_info.get(&(
                family.to_string(),
                table_name.to_string(),
                chain_name.to_string(),
            ));

            if let Some(exprs) = rule.get("expr").and_then(|v| v.as_array()) {
                let Some(known_chains) =
                    table_chains.get(&(family.to_string(), table_name.to_string()))
                else {
                    continue;
                };
                if let Some(nat_rule) =
                    parse_nft_rule_exprs(family, chain_name, exprs, info, known_chains)
                {
                    table.rules.push(nat_rule);
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
#[allow(dead_code)]
struct NftChainInfo {
    chain_type: String,
    hook: String,
}

/// Parse nftables rule expressions to extract NAT information.
fn parse_nft_rule_exprs(
    family: &str,
    chain: &str,
    exprs: &[serde_json::Value],
    _chain_info: Option<&NftChainInfo>,
    known_chains: &std::collections::HashSet<String>,
) -> Option<NatRule> {
    let mut nat_type: Option<NatType> = None;
    let mut to_addr: Option<String> = None;
    let mut to_port: Option<String> = None;
    let mut protocol: Option<String> = None;
    let mut source: Option<String> = None;
    let mut destination: Option<String> = None;
    let mut dport: Option<String> = None;
    let mut sport: Option<String> = None;
    let mut interface_in: Option<String> = None;
    let mut interface_out: Option<String> = None;
    let mut packets: u64 = 0;
    let mut bytes: u64 = 0;

    for expr in exprs {
        if expr.get("snat").is_some() {
            nat_type = Some(NatType::Snat);
            if let Some(snat) = expr.get("snat") {
                to_addr = extract_nft_addr(snat.get("addr"));
                to_port = extract_nft_port(snat.get("port"));
            }
        } else if expr.get("dnat").is_some() {
            nat_type = Some(NatType::Dnat);
            if let Some(dnat) = expr.get("dnat") {
                to_addr = extract_nft_addr(dnat.get("addr"));
                to_port = extract_nft_port(dnat.get("port"));
            }
        } else if expr.get("masquerade").is_some() {
            nat_type = Some(NatType::Masquerade);
            if let Some(masq) = expr.get("masquerade") {
                to_port = extract_nft_port(masq.get("port"));
            }
        } else if let Some(xt) = expr.get("xt") {
            // Handle iptables-nft xtables compatibility layer (e.g., Docker uses this)
            // Format: {"xt": {"type": "target", "name": "MASQUERADE"}}
            if xt.get("type").and_then(|t| t.as_str()) == Some("target") {
                match xt.get("name").and_then(|n| n.as_str()) {
                    Some("MASQUERADE") => nat_type = Some(NatType::Masquerade),
                    Some("SNAT") => nat_type = Some(NatType::Snat),
                    Some("DNAT") => nat_type = Some(NatType::Dnat),
                    Some(target) if known_chains.contains(target) => {
                        nat_type = Some(NatType::Jump(target.to_string()))
                    }
                    _ => {}
                }
            }
        } else if let Some(jump) = expr.get("jump").or_else(|| expr.get("goto")) {
            let target = jump
                .as_str()
                .or_else(|| jump.get("target").and_then(|v| v.as_str()));
            if let Some(target) = target {
                nat_type = Some(NatType::Jump(target.to_string()));
            }
        }

        if let Some(match_expr) = expr.get("match") {
            parse_nft_match(
                match_expr,
                &mut protocol,
                &mut source,
                &mut destination,
                &mut dport,
                &mut sport,
                &mut interface_in,
                &mut interface_out,
            );
        }

        if let Some(counter) = expr.get("counter") {
            packets = counter.get("packets").and_then(|v| v.as_u64()).unwrap_or(0);
            bytes = counter.get("bytes").and_then(|v| v.as_u64()).unwrap_or(0);
        }
    }

    // Only return if we found a NAT statement
    let nat_type = nat_type?;

    Some(NatRule {
        family: family.to_string(),
        chain: chain.to_string(),
        nat_type: nat_type.clone(),
        source,
        destination,
        protocol,
        dport,
        sport,
        to_source: if matches!(nat_type, NatType::Snat | NatType::Masquerade) {
            to_addr.clone()
        } else {
            None
        },
        to_destination: if matches!(nat_type, NatType::Dnat) {
            // Combine address and port for DNAT target
            match (&to_addr, &to_port) {
                (Some(addr), Some(port)) => Some(format!("{}:{}", addr, port)),
                (Some(addr), None) => Some(addr.clone()),
                (None, Some(port)) => Some(format!(":{}", port)),
                (None, None) => None,
            }
        } else {
            None
        },
        interface_in,
        interface_out,
        packets,
        bytes,
    })
}

/// Extract address from nftables expression.
fn extract_nft_addr(val: Option<&serde_json::Value>) -> Option<String> {
    val.and_then(|v| {
        // Can be a direct string or a complex expression
        if let Some(s) = v.as_str() {
            Some(s.to_string())
        } else if let Some(obj) = v.as_object() {
            // Handle prefix notation like {"prefix": {"addr": "10.0.0.0", "len": 24}}
            if let Some(prefix) = obj.get("prefix") {
                let addr = prefix.get("addr").and_then(|a| a.as_str()).unwrap_or("");
                let len = prefix.get("len").and_then(|l| l.as_u64()).unwrap_or(32);
                Some(format!("{}/{}", addr, len))
            } else {
                None
            }
        } else {
            None
        }
    })
}

/// Extract port from nftables expression.
fn extract_nft_port(val: Option<&serde_json::Value>) -> Option<String> {
    val.and_then(|v| {
        if let Some(n) = v.as_u64() {
            Some(n.to_string())
        } else if let Some(s) = v.as_str() {
            Some(s.to_string())
        } else if let Some(obj) = v.as_object() {
            // Handle range notation like {"range": [1024, 65535]}
            if let Some(range) = obj.get("range").and_then(|r| r.as_array()) {
                if range.len() == 2 {
                    let start = range[0].as_u64().unwrap_or(0);
                    let end = range[1].as_u64().unwrap_or(0);
                    return Some(format!("{}-{}", start, end));
                }
            }
            None
        } else {
            None
        }
    })
}

/// Parse nftables match expression to extract filter conditions.
fn parse_nft_match(
    match_expr: &serde_json::Value,
    protocol: &mut Option<String>,
    source: &mut Option<String>,
    destination: &mut Option<String>,
    dport: &mut Option<String>,
    sport: &mut Option<String>,
    interface_in: &mut Option<String>,
    interface_out: &mut Option<String>,
) {
    let left = match_expr.get("left");
    let right = match_expr.get("right");

    if let (Some(left), Some(right)) = (left, right) {
        // Check for meta expressions (interface names, protocol)
        if let Some(meta) = left.get("meta") {
            let key = meta.get("key").and_then(|k| k.as_str()).unwrap_or("");
            match key {
                "iifname" => *interface_in = right.as_str().map(|s| s.to_string()),
                "oifname" => *interface_out = right.as_str().map(|s| s.to_string()),
                "l4proto" => {
                    *protocol = right
                        .as_str()
                        .map(|s| s.to_string())
                        .or_else(|| right.as_u64().map(|n| proto_num_to_name(n)))
                }
                _ => {}
            }
        }

        // Check for payload expressions (addresses, ports)
        if let Some(payload) = left.get("payload") {
            let proto = payload
                .get("protocol")
                .and_then(|p| p.as_str())
                .unwrap_or("");
            let field = payload.get("field").and_then(|f| f.as_str()).unwrap_or("");

            match (proto, field) {
                ("ip", "saddr") | ("ip6", "saddr") => *source = extract_nft_addr(Some(right)),
                ("ip", "daddr") | ("ip6", "daddr") => *destination = extract_nft_addr(Some(right)),
                ("tcp", "dport") | ("udp", "dport") => {
                    *dport = extract_nft_port(Some(right));
                    *protocol = Some(proto.to_string());
                }
                ("tcp", "sport") | ("udp", "sport") => {
                    *sport = extract_nft_port(Some(right));
                    *protocol = Some(proto.to_string());
                }
                _ => {}
            }
        }
    }
}

/// Convert IP protocol number to name.
fn proto_num_to_name(num: u64) -> String {
    match num {
        6 => "tcp".to_string(),
        17 => "udp".to_string(),
        1 => "icmp".to_string(),
        58 => "icmpv6".to_string(),
        _ => num.to_string(),
    }
}

fn parse_iptables_nat(output: &str, family: &str, table: &mut NatTable) {
    let declared_chains: std::collections::HashSet<&str> = output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("-N "))
        .filter_map(|line| line.split_whitespace().next())
        .collect();

    for line in output.lines() {
        let line = line.trim();
        if !line.starts_with("-A ") {
            continue;
        }

        let mut nat_type: Option<NatType> = None;
        let mut chain = String::new();
        let mut protocol: Option<String> = None;
        let mut source: Option<String> = None;
        let mut destination: Option<String> = None;
        let mut dport: Option<String> = None;
        let mut sport: Option<String> = None;
        let mut interface_in: Option<String> = None;
        let mut interface_out: Option<String> = None;
        let mut to_source: Option<String> = None;
        let mut to_destination: Option<String> = None;

        let parts: Vec<&str> = line.split_whitespace().collect();
        let mut i = 0;
        while i < parts.len() {
            match parts[i] {
                "-A" => {
                    if i + 1 < parts.len() {
                        chain = parts[i + 1].to_string();
                        i += 1;
                    }
                }
                "-p" => {
                    if i + 1 < parts.len() {
                        protocol = Some(parts[i + 1].to_string());
                        i += 1;
                    }
                }
                "-s" => {
                    if i + 1 < parts.len() {
                        source = Some(parts[i + 1].to_string());
                        i += 1;
                    }
                }
                "-d" => {
                    if i + 1 < parts.len() {
                        destination = Some(parts[i + 1].to_string());
                        i += 1;
                    }
                }
                "--dport" => {
                    if i + 1 < parts.len() {
                        dport = Some(parts[i + 1].to_string());
                        i += 1;
                    }
                }
                "--sport" => {
                    if i + 1 < parts.len() {
                        sport = Some(parts[i + 1].to_string());
                        i += 1;
                    }
                }
                "-i" => {
                    if i + 1 < parts.len() {
                        interface_in = Some(parts[i + 1].to_string());
                        i += 1;
                    }
                }
                "-o" => {
                    if i + 1 < parts.len() {
                        interface_out = Some(parts[i + 1].to_string());
                        i += 1;
                    }
                }
                "-j" => {
                    if i + 1 < parts.len() {
                        let target = parts[i + 1];
                        match target {
                            "MASQUERADE" => nat_type = Some(NatType::Masquerade),
                            "SNAT" => nat_type = Some(NatType::Snat),
                            "DNAT" => nat_type = Some(NatType::Dnat),
                            _ if declared_chains.contains(target) => {
                                nat_type = Some(NatType::Jump(target.to_string()))
                            }
                            // Extension targets (REDIRECT, NETMAP, LOG, and
                            // future targets) are not chain jumps.
                            _ => {}
                        }
                        i += 1;
                    }
                }
                "--to-source" => {
                    if i + 1 < parts.len() {
                        to_source = Some(parts[i + 1].to_string());
                        i += 1;
                    }
                }
                "--to-destination" => {
                    if i + 1 < parts.len() {
                        to_destination = Some(parts[i + 1].to_string());
                        i += 1;
                    }
                }
                "--to" => {
                    if i + 1 < parts.len() {
                        let target = parts[i + 1].to_string();
                        if nat_type == Some(NatType::Snat) {
                            to_source = Some(target);
                        } else if nat_type == Some(NatType::Dnat) {
                            to_destination = Some(target);
                        }
                        i += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        if let Some(nt) = nat_type {
            table.rules.push(NatRule {
                family: family.to_string(),
                chain,
                nat_type: nt,
                source,
                destination,
                protocol,
                dport,
                sport,
                to_source,
                to_destination,
                interface_in,
                interface_out,
                packets: 0,
                bytes: 0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn softnet_uses_current_kernel_field_positions() {
        let stats = parse_softnet_stats(
            "00000001 00000002 00000003 00000004 00000005 00000006 00000007 00000008 00000009 0000000a 0000000b\n",
        );

        assert_eq!(stats.cpus.len(), 1);
        let cpu = &stats.cpus[0];
        assert_eq!(cpu.processed, 1);
        assert_eq!(cpu.dropped, 2);
        assert_eq!(cpu.time_squeeze, 3);
        assert_eq!(cpu.cpu_collision, 9);
        assert_eq!(cpu.received_rps, 10);
        assert_eq!(cpu.flow_limit_count, 11);
    }

    #[test]
    fn conntrack_counters_are_summed_across_cpus() {
        let counters = parse_conntrack_counters(
            "entries insert_failed drop early_drop\n00000001 00000002 00000003 00000004\n00000001 00000005 00000006 00000007\n",
        );
        assert_eq!(counters, (Some(7), Some(9), Some(11)));
    }

    #[test]
    fn classifies_physical_and_common_virtual_interface_types() {
        assert_eq!(
            classify_interface_type("eth0", None, true, Some(1)),
            InterfaceType::Physical
        );
        assert_eq!(
            classify_interface_type("cali123", None, false, Some(1)),
            InterfaceType::Veth
        );
        assert_eq!(
            classify_interface_type("br0", Some("bridge"), false, Some(1)),
            InterfaceType::Bridge
        );
        assert_eq!(
            classify_interface_type("bond0", Some("bond"), false, Some(1)),
            InterfaceType::Bond
        );
        assert_eq!(
            classify_interface_type("eth0.100", Some("vlan"), false, Some(1)),
            InterfaceType::Vlan
        );
        assert_eq!(
            classify_interface_type("vxlan.calico", Some("vxlan"), false, Some(1)),
            InterfaceType::Vxlan
        );
        assert_eq!(
            classify_interface_type("wg0", None, false, Some(65534)),
            InterfaceType::WireGuard
        );
        assert_eq!(
            classify_interface_type("gre0", None, false, Some(778)),
            InterfaceType::Tunnel
        );
        assert_eq!(
            classify_interface_type("unknown0", None, false, Some(1)),
            InterfaceType::Virtual
        );
    }

    #[test]
    fn tcp_listen_failures_are_parsed_from_netstat() {
        let netstat = "TcpExt: SyncookiesSent ListenOverflows ListenDrops TCPReqQFullDrop TCPAbortOnMemory\nTcpExt: 1 2 3 4 5\n";
        assert_eq!(
            parse_netstat_value(netstat, "TcpExt:", "ListenOverflows"),
            Some(2)
        );
        assert_eq!(
            parse_netstat_value(netstat, "TcpExt:", "ListenDrops"),
            Some(3)
        );
        assert_eq!(
            parse_netstat_value(netstat, "TcpExt:", "TCPReqQFullDrop"),
            Some(4)
        );
        assert_eq!(
            parse_netstat_value(netstat, "TcpExt:", "TCPAbortOnMemory"),
            Some(5)
        );
    }

    #[test]
    fn parses_neighbor_states_and_nft_rule_counts() {
        let mut neighbors = NeighborStats::default();
        parse_neighbor_json(
            r#"[
                {"dst":"10.0.0.1","state":["REACHABLE"]},
                {"dst":"10.0.0.2","state":["FAILED"]},
                {"dst":"10.0.0.3","state":["INCOMPLETE"]}
            ]"#,
            false,
            &mut neighbors,
        );
        assert_eq!(neighbors.ipv4_total, 3);
        assert_eq!(neighbors.reachable, 1);
        assert_eq!(neighbors.failed, 1);
        assert_eq!(neighbors.incomplete, 1);

        assert_eq!(
            count_nft_rules(r#"{"nftables":[{"table":{}},{"rule":{}},{"rule":{}}]}"#),
            2
        );
    }

    #[test]
    fn iptables_only_classifies_declared_chains_as_jumps() {
        let mut table = NatTable::default();
        parse_iptables_nat(
            "-N CUSTOM\n-A PREROUTING -j REDIRECT --to-ports 8080\n-A PREROUTING -j CUSTOM\n",
            "ip",
            &mut table,
        );

        assert_eq!(table.rules.len(), 1);
        assert_eq!(table.rules[0].nat_type, NatType::Jump("CUSTOM".into()));
        assert_eq!(table.rules[0].family, "ip");
    }

    #[test]
    fn nft_native_jump_is_reported() {
        let mut table = NatTable::default();
        parse_nftables_json(
            r#"{"nftables":[
                {"chain":{"family":"ip","table":"nat","name":"PREROUTING","type":"nat","hook":"prerouting"}},
                {"rule":{"family":"ip","table":"nat","chain":"PREROUTING","expr":[{"jump":{"target":"CUSTOM"}}]}}
            ]}"#,
            &mut table,
        )
        .unwrap();

        assert_eq!(table.rules.len(), 1);
        assert_eq!(table.rules[0].nat_type, NatType::Jump("CUSTOM".into()));
    }

    #[test]
    fn nft_xtables_custom_chain_and_masquerade_are_reported() {
        let mut table = NatTable::default();
        parse_nftables_json(
            r#"{"nftables":[
                {"chain":{"family":"ip","table":"nat","name":"POSTROUTING","type":"nat","hook":"postrouting"}},
                {"chain":{"family":"ip","table":"nat","name":"ts-postrouting"}},
                {"rule":{"family":"ip","table":"nat","chain":"POSTROUTING","expr":[{"counter":{"packets":10,"bytes":600}},{"xt":{"type":"target","name":"ts-postrouting"}}]}},
                {"rule":{"family":"ip","table":"nat","chain":"ts-postrouting","expr":[{"counter":{"packets":5,"bytes":300}},{"xt":{"type":"target","name":"MASQUERADE"}}]}}
            ]}"#,
            &mut table,
        )
        .unwrap();

        assert_eq!(table.rules.len(), 2);
        assert_eq!(
            table.rules[0].nat_type,
            NatType::Jump("ts-postrouting".into())
        );
        assert_eq!(table.rules[1].nat_type, NatType::Masquerade);
        assert_eq!(table.rules[1].chain, "ts-postrouting");
    }

    #[test]
    fn nat_rule_identity_ignores_backend_counter_differences() {
        let first = NatRule {
            family: "ip".into(),
            chain: "ts-postrouting".into(),
            nat_type: NatType::Masquerade,
            source: None,
            destination: None,
            protocol: None,
            dport: None,
            sport: None,
            to_source: None,
            to_destination: None,
            interface_in: None,
            interface_out: None,
            packets: 5,
            bytes: 300,
        };
        let mut second = first.clone();
        second.packets = 0;
        second.bytes = 0;

        assert!(same_nat_rule(&first, &second));
    }
}
