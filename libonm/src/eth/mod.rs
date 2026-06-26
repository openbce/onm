mod types;

use futures::TryStreamExt;
use std::fs;
use std::path::Path;
use std::time::Duration;

pub use types::{
    ArpSettings, ConntrackSettings, ConntrackStats, EthError, EthInterface, EthtoolCoalesce,
    EthtoolOffload, EthtoolRing, EthtoolSettings, InterfaceStats, InterfaceType, LinkSettings,
    LinkState, NatRule, NatTable, NatType, NetworkStats, NetworkSysctl, RouteEntry, RouteProtocol,
    RouteScope, RouteTable, RouteType, RpFilterSettings, SocketBufferSettings, SocketStats,
    SoftnetCpuStats, SoftnetStats, TcpSettings, UdpSettings,
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
                823      // ARPHRD_IP6GRE - ip6gre
            );
        }
    }

    false
}

fn get_interface_type(path: &Path) -> InterfaceType {
    // Physical interfaces have a device symlink pointing to a PCI device
    // Virtual interfaces (veth, bridge, etc.) don't have a real PCI device
    let device_path = path.join("device");
    if !device_path.exists() {
        return InterfaceType::Virtual;
    }

    // Check if device path contains "virtual" (e.g., /sys/devices/virtual/...)
    if let Ok(resolved) = device_path.canonicalize() {
        if resolved.to_string_lossy().contains("/virtual/") {
            return InterfaceType::Virtual;
        }
    }

    InterfaceType::Physical
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

    let interface_type = get_interface_type(path);

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
            netdev_max_backlog: read_sysctl_u64("net.core.netdev_max_backlog"),
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

pub async fn get_ethtool_settings(name: &str) -> Result<EthtoolSettings, EthError> {
    use ethtool::{EthtoolAttr, EthtoolCoalesceAttr, EthtoolFeatureAttr, EthtoolRingAttr};

    let (conn, mut handle, _) = ethtool::new_connection()
        .map_err(|e| EthError::Internal(format!("Failed to create ethtool connection: {}", e)))?;
    let conn_handle = tokio::spawn(conn);

    let result = tokio::time::timeout(NETLINK_TIMEOUT, async {
        let mut settings = EthtoolSettings::default();

        let mut rings = handle.ring().get(Some(name)).execute().await;
        while let Ok(Some(msg)) = rings.try_next().await {
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
        while let Ok(Some(msg)) = coalesces.try_next().await {
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
        while let Ok(Some(msg)) = features.try_next().await {
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

        settings
    })
    .await;

    conn_handle.abort();

    match result {
        Ok(settings) => Ok(settings),
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
        if let Some(link) = links.try_next().await.ok().flatten() {
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
        }

        settings
    })
    .await;

    conn_handle.abort();

    match result {
        Ok(settings) => Ok(settings),
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

    let ifindex = get_interface_index(name);

    let result = tokio::time::timeout(NETLINK_TIMEOUT, async {
        let mut addresses = Vec::new();

        let mut addr_stream = handle.address().get().execute();
        while let Ok(Some(msg)) = addr_stream.try_next().await {
            if Some(msg.header.index) != ifindex {
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

        addresses
    })
    .await;

    conn_handle.abort();

    match result {
        Ok(addrs) => Ok(addrs),
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
    use netlink_packet_route::route::{RouteAddress, RouteAttribute, RouteProtocol as RtProto, RouteScope as RtScope, RouteType as RtType};
    use std::net::Ipv4Addr;

    let (conn, handle, _) = rtnetlink::new_connection()
        .map_err(|e| EthError::Internal(format!("Failed to create rtnetlink connection: {}", e)))?;
    let conn_handle = tokio::spawn(conn);

    let result = tokio::time::timeout(NETLINK_TIMEOUT, async {
        let mut table = RouteTable::default();

        let ipv4_msg = rtnetlink::RouteMessageBuilder::<Ipv4Addr>::new().build();
        let mut ipv4_routes = handle.route().get(ipv4_msg).execute();
        while let Ok(Some(route)) = ipv4_routes.try_next().await {
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
        while let Ok(Some(route)) = ipv6_routes.try_next().await {
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
                entry.destination = format!("::{}", entry.prefix_len);
            }

            table.ipv6.push(entry);
        }

        table
    })
    .await;

    conn_handle.abort();

    match result {
        Ok(table) => Ok(table),
        Err(_) => Err(EthError::Internal("rtnetlink route query timed out".to_string())),
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

/// Get NAT rules from nftables using JSON API.
/// 
/// This function queries nftables via `nft -j list ruleset` and parses the JSON output
/// to extract NAT rules (SNAT, DNAT, MASQUERADE). This approach is more robust than
/// parsing iptables text output as the JSON format is stable and well-defined.
pub fn get_nat_rules() -> Result<NatTable, EthError> {
    use std::process::Command;

    let mut table = NatTable::default();

    // Try nftables first (preferred)
    let output = Command::new("nft")
        .args(["-j", "list", "ruleset"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Err(e) = parse_nftables_json(&stdout, &mut table) {
                tracing::debug!("Failed to parse nftables JSON: {}", e);
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.contains("Permission denied") && !stderr.contains("not found") {
                tracing::debug!("nft command failed: {}", stderr);
            }
        }
        Err(e) => {
            tracing::debug!("Failed to run nft: {}", e);
        }
    }

    Ok(table)
}

/// Parse nftables JSON output to extract NAT rules.
/// 
/// The JSON format follows the libnftables-json schema where rules contain
/// expressions (expr) that may include NAT statements like snat, dnat, or masquerade.
fn parse_nftables_json(json_str: &str, table: &mut NatTable) -> Result<(), EthError> {
    use serde_json::Value;

    let root: Value = serde_json::from_str(json_str)
        .map_err(|e| EthError::Internal(format!("Invalid nftables JSON: {}", e)))?;

    let nftables = root.get("nftables")
        .and_then(|v| v.as_array())
        .ok_or_else(|| EthError::Internal("Missing 'nftables' array in JSON".to_string()))?;

    // First pass: collect chain information (for hook/type context)
    let mut chain_info: std::collections::HashMap<(String, String, String), NftChainInfo> = 
        std::collections::HashMap::new();

    for item in nftables {
        if let Some(chain) = item.get("chain") {
            let family = chain.get("family").and_then(|v| v.as_str()).unwrap_or("");
            let table_name = chain.get("table").and_then(|v| v.as_str()).unwrap_or("");
            let chain_name = chain.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let chain_type = chain.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let hook = chain.get("hook").and_then(|v| v.as_str()).unwrap_or("");

            chain_info.insert(
                (family.to_string(), table_name.to_string(), chain_name.to_string()),
                NftChainInfo {
                    chain_type: chain_type.to_string(),
                    hook: hook.to_string(),
                }
            );
        }
    }

    // Second pass: extract rules with NAT statements
    for item in nftables {
        if let Some(rule) = item.get("rule") {
            let family = rule.get("family").and_then(|v| v.as_str()).unwrap_or("");
            let table_name = rule.get("table").and_then(|v| v.as_str()).unwrap_or("");
            let chain_name = rule.get("chain").and_then(|v| v.as_str()).unwrap_or("");

            // Get chain info to check if this is a NAT chain
            let info = chain_info.get(&(family.to_string(), table_name.to_string(), chain_name.to_string()));
            
            if let Some(exprs) = rule.get("expr").and_then(|v| v.as_array()) {
                if let Some(nat_rule) = parse_nft_rule_exprs(chain_name, exprs, info) {
                    table.rules.push(nat_rule);
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct NftChainInfo {
    chain_type: String,
    hook: String,
}

/// Parse nftables rule expressions to extract NAT information.
fn parse_nft_rule_exprs(chain: &str, exprs: &[serde_json::Value], _chain_info: Option<&NftChainInfo>) -> Option<NatRule> {
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
                    _ => {}
                }
            }
        }

        if let Some(match_expr) = expr.get("match") {
            parse_nft_match(match_expr, &mut protocol, &mut source, &mut destination, 
                           &mut dport, &mut sport, &mut interface_in, &mut interface_out);
        }

        if let Some(counter) = expr.get("counter") {
            packets = counter.get("packets").and_then(|v| v.as_u64()).unwrap_or(0);
            bytes = counter.get("bytes").and_then(|v| v.as_u64()).unwrap_or(0);
        }
    }

    // Only return if we found a NAT statement
    let nat_type = nat_type?;

    Some(NatRule {
        chain: chain.to_string(),
        nat_type: nat_type.clone(),
        source,
        destination,
        protocol,
        dport,
        sport,
        to_source: if matches!(nat_type, NatType::Snat | NatType::Masquerade) { to_addr.clone() } else { None },
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
                "l4proto" => *protocol = right.as_str().map(|s| s.to_string())
                    .or_else(|| right.as_u64().map(|n| proto_num_to_name(n))),
                _ => {}
            }
        }

        // Check for payload expressions (addresses, ports)
        if let Some(payload) = left.get("payload") {
            let proto = payload.get("protocol").and_then(|p| p.as_str()).unwrap_or("");
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
