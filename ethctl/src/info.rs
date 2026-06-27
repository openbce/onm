use comfy_table::{presets::NOTHING, Cell, Color, Table};
use libonm::eth::{self, EthError};

#[derive(Clone, Copy)]
pub enum TuningProfile {
    ControlPlane,
    Worker,
}

impl TuningProfile {
    pub fn from_str(s: &str) -> Result<Self, EthError> {
        match s.to_lowercase().as_str() {
            "control-plane" | "controlplane" | "cp" | "master" => Ok(TuningProfile::ControlPlane),
            "worker" => Ok(TuningProfile::Worker),
            _ => Err(EthError::InvalidConfig(format!(
                "unknown profile '{s}' (expected 'worker' or 'control-plane')"
            ))),
        }
    }

    #[allow(dead_code)]
    pub fn header_suffix(&self) -> &'static str {
        match self {
            TuningProfile::ControlPlane => "Suggested (CP 10k)",
            TuningProfile::Worker => "Suggested (Worker 10k)",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            TuningProfile::ControlPlane => "control-plane",
            TuningProfile::Worker => "worker",
        }
    }
}

#[derive(Clone, Copy)]
pub enum OutputFormat {
    Cmd,
    Conf,
    Script,
}

/// Indicates whether the suggested value is a minimum or maximum bound
#[derive(Clone, Copy)]
pub enum BoundType {
    /// Current value should be >= suggested (e.g., buffer sizes, queue lengths)
    Min,
    /// Current value should be <= suggested (e.g., timeouts, retries)
    Max,
    /// Current two-value interval should cover the suggested interval.
    Range,
}

impl BoundType {
    pub fn prefix(&self) -> &'static str {
        match self {
            BoundType::Min => ">= ",
            BoundType::Max => "<= ",
            BoundType::Range => "covers ",
        }
    }

    pub fn is_satisfied(&self, current: Option<u64>, suggested: u64) -> bool {
        match current {
            None => false,
            Some(val) => match self {
                BoundType::Min => val >= suggested,
                BoundType::Max => val <= suggested,
                BoundType::Range => false,
            },
        }
    }

    pub fn is_str_satisfied(&self, current: Option<&str>, suggested: &str) -> bool {
        let parse = |value: &str| {
            value
                .split_whitespace()
                .map(str::parse::<u64>)
                .collect::<Result<Vec<_>, _>>()
                .ok()
        };
        let Some(current) = current.and_then(parse) else {
            return false;
        };
        let Some(suggested) = parse(suggested) else {
            return false;
        };
        if current.len() != suggested.len() {
            return false;
        }
        match self {
            BoundType::Min => current.iter().zip(suggested).all(|(a, b)| *a >= b),
            BoundType::Max => current.iter().zip(suggested).all(|(a, b)| *a <= b),
            BoundType::Range => {
                current.len() == 2 && current[0] <= suggested[0] && current[1] >= suggested[1]
            }
        }
    }
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Result<Self, EthError> {
        match s.to_lowercase().as_str() {
            "cmd" | "command" => Ok(OutputFormat::Cmd),
            "conf" | "sysctl.conf" | "file" => Ok(OutputFormat::Conf),
            "script" | "sh" | "bash" => Ok(OutputFormat::Script),
            _ => Err(EthError::InvalidConfig(format!(
                "unknown output format '{s}' (expected 'cmd', 'conf', or 'script')"
            ))),
        }
    }
}

pub struct SuggestedValues {
    // Connection tracking
    pub conntrack_max: u64,
    pub conntrack_buckets: u64,
    pub conntrack_tcp_timeout_established: u64,
    pub conntrack_tcp_timeout_time_wait: u64,
    pub conntrack_tcp_timeout_close_wait: u64,
    pub conntrack_tcp_timeout_fin_wait: u64,
    pub conntrack_tcp_max_retrans: u64,
    // Socket buffers
    pub rmem_max: u64,
    pub wmem_max: u64,
    pub rmem_default: u64,
    pub wmem_default: u64,
    pub tcp_rmem: &'static str,
    pub tcp_wmem: &'static str,
    pub netdev_max_backlog: u64,
    pub udp_rmem_min: u64,
    pub udp_wmem_min: u64,
    pub udp_mem: &'static str,
    // TCP settings
    pub somaxconn: u64,
    pub tcp_max_syn_backlog: u64,
    pub tcp_tw_reuse: u64,
    pub tcp_fin_timeout: u64,
    pub tcp_keepalive_time: u64,
    pub tcp_keepalive_probes: u64,
    pub tcp_keepalive_intvl: u64,
    pub ip_local_port_range: &'static str,
    // ARP/Neighbor
    pub arp_gc_thresh1: u64,
    pub arp_gc_thresh2: u64,
    pub arp_gc_thresh3: u64,
    pub arp_ignore: u64,
    pub arp_announce: u64,
    // RP filter
    pub rp_filter: u64,
    // Interface settings (ip link)
    pub txqueuelen: u64,
    pub mtu: u64,
    pub gso_max_size: u64,
    pub gso_max_segs: u64,
    pub gro_max_size: u64,
    pub tso_max_size: u64,
    pub tso_max_segs: u64,
    // Ethtool settings
    pub ring_rx: u64,
    pub ring_tx: u64,
    pub coalesce_rx_usecs: u64,
    pub coalesce_tx_usecs: u64,
    pub offload_tso: bool,
    pub offload_gso: bool,
    pub offload_gro: bool,
}

impl SuggestedValues {
    pub fn for_profile(profile: TuningProfile) -> Self {
        match profile {
            TuningProfile::ControlPlane => Self::control_plane(),
            TuningProfile::Worker => Self::worker(),
        }
    }

    // Control plane: handles API server, etcd, all node connections
    // Needs: massive conntrack, huge ARP tables, high socket buffers
    fn control_plane() -> Self {
        Self {
            conntrack_max: 4_194_304,     // 4M - API server sees all cluster traffic
            conntrack_buckets: 1_048_576, // max/4
            conntrack_tcp_timeout_established: 86400,
            conntrack_tcp_timeout_time_wait: 60,
            conntrack_tcp_timeout_close_wait: 60,
            conntrack_tcp_timeout_fin_wait: 60,
            conntrack_tcp_max_retrans: 3,

            rmem_max: 268_435_456, // 256MB - etcd/API server traffic
            wmem_max: 268_435_456,
            rmem_default: 33_554_432, // 32MB
            wmem_default: 33_554_432,
            tcp_rmem: "4096 2097152 268435456",
            tcp_wmem: "4096 2097152 268435456",
            netdev_max_backlog: 50000,
            udp_rmem_min: 16_384,
            udp_wmem_min: 16_384,
            udp_mem: "1048576 4194304 16777216",

            somaxconn: 65535,
            tcp_max_syn_backlog: 65535,
            tcp_tw_reuse: 1,
            tcp_fin_timeout: 15,
            tcp_keepalive_time: 600,
            tcp_keepalive_probes: 3,
            tcp_keepalive_intvl: 15,
            ip_local_port_range: "1024 65535",

            arp_gc_thresh1: 16384, // Must hold all 10k node entries
            arp_gc_thresh2: 65536,
            arp_gc_thresh3: 131072,
            arp_ignore: 1,
            arp_announce: 2,

            rp_filter: 0,

            txqueuelen: 10000, // High queue for API server traffic bursts
            mtu: 9000,         // Jumbo frames (requires network support)
            gso_max_size: 65536,
            gso_max_segs: 65535,
            gro_max_size: 65536,
            tso_max_size: 524280, // ~512KB for high throughput
            tso_max_segs: 65535,

            ring_rx: 4096,
            ring_tx: 4096,
            coalesce_rx_usecs: 50, // Balance latency vs CPU
            coalesce_tx_usecs: 50,
            offload_tso: true,
            offload_gso: true,
            offload_gro: true,
        }
    }

    // Worker: handles pod traffic, moderate connections
    // Needs: reasonable conntrack, standard ARP, good buffers
    fn worker() -> Self {
        Self {
            conntrack_max: 1_048_576, // 1M - per-node workload
            conntrack_buckets: 262_144,
            conntrack_tcp_timeout_established: 86400,
            conntrack_tcp_timeout_time_wait: 60,
            conntrack_tcp_timeout_close_wait: 60,
            conntrack_tcp_timeout_fin_wait: 60,
            conntrack_tcp_max_retrans: 3,

            rmem_max: 134_217_728, // 128MB
            wmem_max: 134_217_728,
            rmem_default: 16_777_216, // 16MB
            wmem_default: 16_777_216,
            tcp_rmem: "4096 1048576 134217728",
            tcp_wmem: "4096 1048576 134217728",
            netdev_max_backlog: 30000,
            udp_rmem_min: 16_384,
            udp_wmem_min: 16_384,
            udp_mem: "786432 2097152 8388608",

            somaxconn: 32768,
            tcp_max_syn_backlog: 32768,
            tcp_tw_reuse: 1,
            tcp_fin_timeout: 15,
            tcp_keepalive_time: 600,
            tcp_keepalive_probes: 3,
            tcp_keepalive_intvl: 15,
            ip_local_port_range: "1024 65535",

            arp_gc_thresh1: 4096, // Local subnet typically
            arp_gc_thresh2: 8192,
            arp_gc_thresh3: 16384,
            arp_ignore: 1,
            arp_announce: 2,

            rp_filter: 0,

            txqueuelen: 5000, // Moderate queue for pod traffic
            mtu: 9000,        // Jumbo frames (requires network support)
            gso_max_size: 65536,
            gso_max_segs: 65535,
            gro_max_size: 65536,
            tso_max_size: 262144, // 256KB for worker nodes
            tso_max_segs: 65535,

            ring_rx: 2048,
            ring_tx: 2048,
            coalesce_rx_usecs: 100, // Higher coalescing for throughput
            coalesce_tx_usecs: 100,
            offload_tso: true,
            offload_gso: true,
            offload_gro: true,
        }
    }
}

pub fn run(profile_str: &str, output: Option<&str>, backup: Option<&str>) -> Result<(), EthError> {
    let profile = TuningProfile::from_str(profile_str)?;

    if let Some(fmt) = backup {
        let format = OutputFormat::from_str(fmt)?;
        if matches!(format, OutputFormat::Script) {
            return Err(EthError::InvalidConfig(
                "backup format must be 'cmd' or 'conf'".to_string(),
            ));
        }
        generate_backup_output(format);
        return Ok(());
    }

    if let Some(fmt) = output {
        let format = OutputFormat::from_str(fmt)?;
        generate_sysctl_output(profile, format);
        return Ok(());
    }

    let interfaces = eth::list_interfaces()?;
    let s = SuggestedValues::for_profile(profile);
    let sysctl = eth::get_network_sysctl();

    let mut table = Table::new();
    table.load_preset(NOTHING);

    add_first_section(&mut table, "Overview");
    table.add_row(vec!["  Profile", profile.name(), "-"]);

    add_sysctl_rows(&mut table, &sysctl, &s);

    println!("{table}");
    println!();

    print_interfaces_table(&interfaces, &s);

    Ok(())
}

fn print_interfaces_table(interfaces: &[eth::EthInterface], s: &SuggestedValues) {
    let mut table = Table::new();
    table.load_preset(NOTHING);

    add_first_section(&mut table, "Interfaces");
    table.add_row(vec![
        Cell::new("  Name"),
        Cell::new("MAC Address"),
        Cell::new("MTU"),
        Cell::new("TXQ"),
        Cell::new("State"),
        Cell::new("Speed"),
        Cell::new("Duplex"),
        Cell::new("NUMA"),
        Cell::new("Driver"),
        Cell::new("Type"),
    ]);

    for iface in interfaces {
        let mtu_str = format_with_suggested(iface.mtu as u64, s.mtu);
        let txq_str = format_with_suggested(iface.txqueuelen as u64, s.txqueuelen);
        let speed_str = iface
            .speed
            .map(|sp| format!("{}", sp))
            .unwrap_or("-".to_string());
        let duplex_str = iface.duplex.clone().unwrap_or("-".to_string());
        let numa_str = iface
            .numa_node
            .map(|n| n.to_string())
            .unwrap_or("-".to_string());
        let driver_str = iface.driver.clone().unwrap_or("-".to_string());

        table.add_row(vec![
            format!("  {}", iface.name),
            iface.mac_address.clone(),
            mtu_str,
            txq_str,
            iface.state.to_string(),
            speed_str,
            duplex_str,
            numa_str,
            driver_str,
            iface.interface_type.to_string(),
        ]);
    }

    println!("{table}");
}

fn format_with_suggested(current: u64, suggested: u64) -> String {
    if current == suggested {
        current.to_string()
    } else {
        format!("{} ({})", current, suggested)
    }
}

#[allow(dead_code)]
pub fn print_sysctl_tables(profile: TuningProfile) {
    use libonm::eth;

    let sysctl = eth::get_network_sysctl();
    let s = SuggestedValues::for_profile(profile);

    let mut table = Table::new();
    table.load_preset(NOTHING);

    add_first_section(&mut table, "Overview");
    table.add_row(vec!["  Profile", profile.name(), "-"]);

    add_sysctl_rows(&mut table, &sysctl, &s);

    println!("{table}");
}

fn add_sysctl_rows(table: &mut Table, sysctl: &eth::NetworkSysctl, s: &SuggestedValues) {
    use BoundType::{Max, Min};

    add_section(table, "Connection Tracking");
    add_row(
        table,
        "  nf_conntrack_max",
        sysctl.conntrack.max,
        s.conntrack_max,
        Min,
    );
    add_row(
        table,
        "  nf_conntrack_buckets",
        sysctl.conntrack.buckets,
        s.conntrack_buckets,
        Min,
    );
    add_row(
        table,
        "  nf_conntrack_tcp_timeout_established",
        sysctl.conntrack.tcp_timeout_established,
        s.conntrack_tcp_timeout_established,
        Min,
    );
    add_row(
        table,
        "  nf_conntrack_tcp_timeout_time_wait",
        sysctl.conntrack.tcp_timeout_time_wait,
        s.conntrack_tcp_timeout_time_wait,
        Max,
    );
    add_row(
        table,
        "  nf_conntrack_tcp_timeout_close_wait",
        sysctl.conntrack.tcp_timeout_close_wait,
        s.conntrack_tcp_timeout_close_wait,
        Max,
    );
    add_row(
        table,
        "  nf_conntrack_tcp_timeout_fin_wait",
        sysctl.conntrack.tcp_timeout_fin_wait,
        s.conntrack_tcp_timeout_fin_wait,
        Max,
    );
    add_row(
        table,
        "  nf_conntrack_tcp_max_retrans",
        sysctl.conntrack.tcp_max_retrans,
        s.conntrack_tcp_max_retrans,
        Max,
    );

    add_section(table, "Socket Buffers");
    add_row_bytes(
        table,
        "  net.core.rmem_max",
        sysctl.socket_buffer.rmem_max,
        s.rmem_max,
        Min,
    );
    add_row_bytes(
        table,
        "  net.core.wmem_max",
        sysctl.socket_buffer.wmem_max,
        s.wmem_max,
        Min,
    );
    add_row_bytes(
        table,
        "  net.core.rmem_default",
        sysctl.socket_buffer.rmem_default,
        s.rmem_default,
        Min,
    );
    add_row_bytes(
        table,
        "  net.core.wmem_default",
        sysctl.socket_buffer.wmem_default,
        s.wmem_default,
        Min,
    );
    add_row_tcp_mem(
        table,
        "  net.ipv4.tcp_rmem",
        sysctl.socket_buffer.tcp_rmem.clone(),
        s.tcp_rmem,
        Min,
    );
    add_row_tcp_mem(
        table,
        "  net.ipv4.tcp_wmem",
        sysctl.socket_buffer.tcp_wmem.clone(),
        s.tcp_wmem,
        Min,
    );
    add_row(
        table,
        "  net.core.netdev_max_backlog",
        sysctl.socket_buffer.netdev_max_backlog,
        s.netdev_max_backlog,
        Min,
    );
    add_row_bytes(
        table,
        "  net.ipv4.udp_rmem_min",
        sysctl.udp.rmem_min,
        s.udp_rmem_min,
        Min,
    );
    add_row_bytes(
        table,
        "  net.ipv4.udp_wmem_min",
        sysctl.udp.wmem_min,
        s.udp_wmem_min,
        Min,
    );
    add_row_tcp_mem(
        table,
        "  net.ipv4.udp_mem",
        sysctl.udp.udp_mem.clone(),
        s.udp_mem,
        Min,
    );

    add_section(table, "TCP Settings");
    add_row(
        table,
        "  net.core.somaxconn",
        sysctl.tcp.somaxconn,
        s.somaxconn,
        Min,
    );
    add_row(
        table,
        "  net.ipv4.tcp_max_syn_backlog",
        sysctl.tcp.max_syn_backlog,
        s.tcp_max_syn_backlog,
        Min,
    );
    add_row(
        table,
        "  net.ipv4.tcp_tw_reuse",
        sysctl.tcp.tw_reuse,
        s.tcp_tw_reuse,
        Min,
    );
    add_row(
        table,
        "  net.ipv4.tcp_fin_timeout",
        sysctl.tcp.fin_timeout,
        s.tcp_fin_timeout,
        Max,
    );
    add_row(
        table,
        "  net.ipv4.tcp_keepalive_time",
        sysctl.tcp.keepalive_time,
        s.tcp_keepalive_time,
        Max,
    );
    add_row(
        table,
        "  net.ipv4.tcp_keepalive_probes",
        sysctl.tcp.keepalive_probes,
        s.tcp_keepalive_probes,
        Max,
    );
    add_row(
        table,
        "  net.ipv4.tcp_keepalive_intvl",
        sysctl.tcp.keepalive_intvl,
        s.tcp_keepalive_intvl,
        Max,
    );
    add_row_str(
        table,
        "  net.ipv4.ip_local_port_range",
        sysctl.tcp.ip_local_port_range.clone(),
        s.ip_local_port_range,
        BoundType::Range,
    );

    add_section(table, "ARP / Neighbor Table");
    add_row(
        table,
        "  net.ipv4.neigh.default.gc_thresh1",
        sysctl.arp.gc_thresh1,
        s.arp_gc_thresh1,
        Min,
    );
    add_row(
        table,
        "  net.ipv4.neigh.default.gc_thresh2",
        sysctl.arp.gc_thresh2,
        s.arp_gc_thresh2,
        Min,
    );
    add_row(
        table,
        "  net.ipv4.neigh.default.gc_thresh3",
        sysctl.arp.gc_thresh3,
        s.arp_gc_thresh3,
        Min,
    );
    add_row(
        table,
        "  net.ipv4.conf.all.arp_ignore",
        sysctl.arp.arp_ignore,
        s.arp_ignore,
        Min,
    );
    add_row(
        table,
        "  net.ipv4.conf.all.arp_announce",
        sysctl.arp.arp_announce,
        s.arp_announce,
        Min,
    );

    add_section(table, "Reverse Path Filtering");
    add_row(
        table,
        "  net.ipv4.conf.all.rp_filter",
        sysctl.rp_filter.all,
        s.rp_filter,
        Max,
    );
    add_row(
        table,
        "  net.ipv4.conf.default.rp_filter",
        sysctl.rp_filter.default,
        s.rp_filter,
        Max,
    );
}

pub async fn print_link_tables(name: &str, profile: TuningProfile) -> Result<(), EthError> {
    use libonm::eth;

    let s = SuggestedValues::for_profile(profile);

    let link = eth::get_link_settings(name).await?;
    let ethtool = eth::get_ethtool_settings(name).await?;

    let mut table = Table::new();
    table.load_preset(NOTHING);

    add_first_section(&mut table, "Overview");
    table.add_row(vec!["  Profile", profile.name(), "-"]);

    add_section(&mut table, "IP Link Settings");
    {
        let l = &link;
        add_row_u32_bytes(&mut table, "  mtu", l.mtu, s.mtu as u32);
        add_row_u32_bytes(&mut table, "  min_mtu", l.min_mtu, 0);
        add_row_u32_bytes(&mut table, "  max_mtu", l.max_mtu, 0);
        add_row_u32(
            &mut table,
            "  txqueuelen",
            l.txqueuelen,
            s.txqueuelen as u32,
        );
        add_row_u32(&mut table, "  num_tx_queues", l.num_tx_queues, 0);
        add_row_u32(&mut table, "  num_rx_queues", l.num_rx_queues, 0);
        add_row_u32_bytes(
            &mut table,
            "  gso_max_size",
            l.gso_max_size,
            s.gso_max_size as u32,
        );
        add_row_u32(
            &mut table,
            "  gso_max_segs",
            l.gso_max_segs,
            s.gso_max_segs as u32,
        );
        add_row_u32_bytes(
            &mut table,
            "  gro_max_size",
            l.gro_max_size,
            s.gro_max_size as u32,
        );
        add_row_u32_bytes(
            &mut table,
            "  tso_max_size",
            l.tso_max_size,
            s.tso_max_size as u32,
        );
        add_row_u32(
            &mut table,
            "  tso_max_segs",
            l.tso_max_segs,
            s.tso_max_segs as u32,
        );
        add_row_str(&mut table, "  qdisc", l.qdisc.clone(), "-", BoundType::Min);
        add_row_u32(&mut table, "  group", l.group, 0);
    }

    add_section(&mut table, "Ethtool Settings");
    {
        let e = &ethtool;
        add_row_u32(&mut table, "  ring_rx", e.ring.rx, s.ring_rx as u32);
        add_row_u32(&mut table, "  ring_rx_max", e.ring.rx_max, 0);
        add_row_u32(&mut table, "  ring_tx", e.ring.tx, s.ring_tx as u32);
        add_row_u32(&mut table, "  ring_tx_max", e.ring.tx_max, 0);
        add_row_u32(
            &mut table,
            "  coalesce_rx_usecs",
            e.coalesce.rx_usecs,
            s.coalesce_rx_usecs as u32,
        );
        add_row_u32(
            &mut table,
            "  coalesce_tx_usecs",
            e.coalesce.tx_usecs,
            s.coalesce_tx_usecs as u32,
        );
        add_row_bool(&mut table, "  offload_tso", e.offload.tso, s.offload_tso);
        add_row_bool(&mut table, "  offload_gso", e.offload.gso, s.offload_gso);
        add_row_bool(&mut table, "  offload_gro", e.offload.gro, s.offload_gro);
    }

    println!("{table}");
    Ok(())
}

fn add_section(table: &mut Table, name: &str) {
    table.add_row(vec![Cell::new(""), Cell::new(""), Cell::new("")]);
    table.add_row(vec![
        Cell::new(format!("{}:", name)).fg(Color::Cyan),
        Cell::new(""),
        Cell::new(""),
    ]);
}

fn add_first_section(table: &mut Table, name: &str) {
    table.add_row(vec![
        Cell::new(format!("{}:", name)).fg(Color::Cyan),
        Cell::new(""),
        Cell::new(""),
    ]);
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{}G", bytes / 1_073_741_824)
    } else if bytes >= 1_048_576 {
        format!("{}M", bytes / 1_048_576)
    } else if bytes >= 1024 {
        format!("{}K", bytes / 1024)
    } else {
        bytes.to_string()
    }
}

fn format_size_u32(bytes: u32) -> String {
    format_size(bytes as u64)
}

fn format_tcp_mem(value: &str) -> String {
    value
        .split_whitespace()
        .map(|s| s.parse::<u64>().map(format_size).unwrap_or(s.to_string()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn add_row_u32(table: &mut Table, name: &str, value: Option<u32>, suggested: u32) {
    let suggested_str = if suggested == 0 {
        "-".to_string()
    } else {
        suggested.to_string()
    };
    table.add_row(vec![
        name.to_string(),
        value.map(|v| v.to_string()).unwrap_or("-".to_string()),
        suggested_str,
    ]);
}

fn add_row_u32_bytes(table: &mut Table, name: &str, value: Option<u32>, suggested: u32) {
    let suggested_str = if suggested == 0 {
        "-".to_string()
    } else {
        format_size_u32(suggested)
    };
    table.add_row(vec![
        name.to_string(),
        value.map(|v| format_size_u32(v)).unwrap_or("-".to_string()),
        suggested_str,
    ]);
}

fn add_row_bool(table: &mut Table, name: &str, value: Option<bool>, suggested: bool) {
    table.add_row(vec![
        name.to_string(),
        value
            .map(|v| if v { "on" } else { "off" }.to_string())
            .unwrap_or("-".to_string()),
        if suggested { "on" } else { "off" }.to_string(),
    ]);
}

fn add_row(table: &mut Table, name: &str, value: Option<u64>, suggested: u64, bound: BoundType) {
    let suggested_str = if bound.is_satisfied(value, suggested) {
        "OK".to_string()
    } else {
        format!("{}{}", bound.prefix(), suggested)
    };
    table.add_row(vec![
        name.to_string(),
        value.map(|v| v.to_string()).unwrap_or("-".to_string()),
        suggested_str,
    ]);
}

fn add_row_bytes(
    table: &mut Table,
    name: &str,
    value: Option<u64>,
    suggested: u64,
    bound: BoundType,
) {
    let suggested_str = if bound.is_satisfied(value, suggested) {
        "OK".to_string()
    } else {
        format!("{}{}", bound.prefix(), format_size(suggested))
    };
    table.add_row(vec![
        name.to_string(),
        value.map(|v| format_size(v)).unwrap_or("-".to_string()),
        suggested_str,
    ]);
}

fn add_row_str(
    table: &mut Table,
    name: &str,
    value: Option<String>,
    suggested: &str,
    bound: BoundType,
) {
    let suggested_str = if bound.is_str_satisfied(value.as_deref(), suggested) {
        "OK".to_string()
    } else {
        format!("{}{}", bound.prefix(), suggested)
    };
    table.add_row(vec![
        name.to_string(),
        value.unwrap_or("-".to_string()),
        suggested_str,
    ]);
}

fn add_row_tcp_mem(
    table: &mut Table,
    name: &str,
    value: Option<String>,
    suggested: &str,
    bound: BoundType,
) {
    let suggested_str = if bound.is_str_satisfied(value.as_deref(), suggested) {
        "OK".to_string()
    } else {
        format!("{}{}", bound.prefix(), format_tcp_mem(suggested))
    };
    table.add_row(vec![
        name.to_string(),
        value.map(|v| format_tcp_mem(&v)).unwrap_or("-".to_string()),
        suggested_str,
    ]);
}

pub fn generate_backup_output(format: OutputFormat) {
    use libonm::eth;

    let sysctl = eth::get_network_sysctl();

    let settings: Vec<(&str, Option<u64>)> = vec![
        ("net.netfilter.nf_conntrack_max", sysctl.conntrack.max),
        (
            "net.netfilter.nf_conntrack_buckets",
            sysctl.conntrack.buckets,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_established",
            sysctl.conntrack.tcp_timeout_established,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_time_wait",
            sysctl.conntrack.tcp_timeout_time_wait,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_close_wait",
            sysctl.conntrack.tcp_timeout_close_wait,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_fin_wait",
            sysctl.conntrack.tcp_timeout_fin_wait,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_max_retrans",
            sysctl.conntrack.tcp_max_retrans,
        ),
        ("net.core.rmem_max", sysctl.socket_buffer.rmem_max),
        ("net.core.wmem_max", sysctl.socket_buffer.wmem_max),
        ("net.core.rmem_default", sysctl.socket_buffer.rmem_default),
        ("net.core.wmem_default", sysctl.socket_buffer.wmem_default),
        (
            "net.core.netdev_max_backlog",
            sysctl.socket_buffer.netdev_max_backlog,
        ),
        ("net.core.somaxconn", sysctl.tcp.somaxconn),
        ("net.ipv4.tcp_max_syn_backlog", sysctl.tcp.max_syn_backlog),
        ("net.ipv4.tcp_tw_reuse", sysctl.tcp.tw_reuse),
        ("net.ipv4.tcp_fin_timeout", sysctl.tcp.fin_timeout),
        ("net.ipv4.tcp_keepalive_time", sysctl.tcp.keepalive_time),
        ("net.ipv4.tcp_keepalive_probes", sysctl.tcp.keepalive_probes),
        ("net.ipv4.tcp_keepalive_intvl", sysctl.tcp.keepalive_intvl),
        ("net.ipv4.udp_rmem_min", sysctl.udp.rmem_min),
        ("net.ipv4.udp_wmem_min", sysctl.udp.wmem_min),
        ("net.ipv4.neigh.default.gc_thresh1", sysctl.arp.gc_thresh1),
        ("net.ipv4.neigh.default.gc_thresh2", sysctl.arp.gc_thresh2),
        ("net.ipv4.neigh.default.gc_thresh3", sysctl.arp.gc_thresh3),
        ("net.ipv4.conf.all.arp_ignore", sysctl.arp.arp_ignore),
        ("net.ipv4.conf.all.arp_announce", sysctl.arp.arp_announce),
        ("net.ipv4.conf.all.rp_filter", sysctl.rp_filter.all),
        ("net.ipv4.conf.default.rp_filter", sysctl.rp_filter.default),
    ];

    let string_settings: Vec<(&str, Option<String>)> = vec![
        ("net.ipv4.tcp_rmem", sysctl.socket_buffer.tcp_rmem.clone()),
        ("net.ipv4.tcp_wmem", sysctl.socket_buffer.tcp_wmem.clone()),
        (
            "net.ipv4.ip_local_port_range",
            sysctl.tcp.ip_local_port_range.clone(),
        ),
        ("net.ipv4.udp_mem", sysctl.udp.udp_mem.clone()),
    ];

    match format {
        OutputFormat::Cmd => {
            println!("# Backup of current sysctl values");
            for (key, value) in &settings {
                if let Some(v) = value {
                    println!("sysctl -w {}={}", key, v);
                }
            }
            for (key, value) in &string_settings {
                if let Some(v) = value {
                    println!("sysctl -w {}=\"{}\"", key, v);
                }
            }
        }
        OutputFormat::Conf | OutputFormat::Script => {
            println!("# Backup of current sysctl values");
            println!("# Save to restore later with: sysctl --system");
            println!();
            for (key, value) in &settings {
                if let Some(v) = value {
                    println!("{} = {}", key, v);
                }
            }
            for (key, value) in &string_settings {
                if let Some(v) = value {
                    println!("{} = {}", key, v);
                }
            }
        }
    }
}

pub fn generate_sysctl_output(profile: TuningProfile, format: OutputFormat) {
    use libonm::eth;
    use BoundType::{Max, Min};

    let s = SuggestedValues::for_profile(profile);
    let sysctl = eth::get_network_sysctl();

    let settings: Vec<(&str, u64, Option<u64>, BoundType)> = vec![
        (
            "net.netfilter.nf_conntrack_max",
            s.conntrack_max,
            sysctl.conntrack.max,
            Min,
        ),
        (
            "net.netfilter.nf_conntrack_buckets",
            s.conntrack_buckets,
            sysctl.conntrack.buckets,
            Min,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_established",
            s.conntrack_tcp_timeout_established,
            sysctl.conntrack.tcp_timeout_established,
            Min,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_time_wait",
            s.conntrack_tcp_timeout_time_wait,
            sysctl.conntrack.tcp_timeout_time_wait,
            Max,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_close_wait",
            s.conntrack_tcp_timeout_close_wait,
            sysctl.conntrack.tcp_timeout_close_wait,
            Max,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_fin_wait",
            s.conntrack_tcp_timeout_fin_wait,
            sysctl.conntrack.tcp_timeout_fin_wait,
            Max,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_max_retrans",
            s.conntrack_tcp_max_retrans,
            sysctl.conntrack.tcp_max_retrans,
            Max,
        ),
        (
            "net.core.rmem_max",
            s.rmem_max,
            sysctl.socket_buffer.rmem_max,
            Min,
        ),
        (
            "net.core.wmem_max",
            s.wmem_max,
            sysctl.socket_buffer.wmem_max,
            Min,
        ),
        (
            "net.core.rmem_default",
            s.rmem_default,
            sysctl.socket_buffer.rmem_default,
            Min,
        ),
        (
            "net.core.wmem_default",
            s.wmem_default,
            sysctl.socket_buffer.wmem_default,
            Min,
        ),
        (
            "net.core.netdev_max_backlog",
            s.netdev_max_backlog,
            sysctl.socket_buffer.netdev_max_backlog,
            Min,
        ),
        ("net.core.somaxconn", s.somaxconn, sysctl.tcp.somaxconn, Min),
        (
            "net.ipv4.tcp_max_syn_backlog",
            s.tcp_max_syn_backlog,
            sysctl.tcp.max_syn_backlog,
            Min,
        ),
        (
            "net.ipv4.tcp_tw_reuse",
            s.tcp_tw_reuse,
            sysctl.tcp.tw_reuse,
            Min,
        ),
        (
            "net.ipv4.tcp_fin_timeout",
            s.tcp_fin_timeout,
            sysctl.tcp.fin_timeout,
            Max,
        ),
        (
            "net.ipv4.tcp_keepalive_time",
            s.tcp_keepalive_time,
            sysctl.tcp.keepalive_time,
            Max,
        ),
        (
            "net.ipv4.tcp_keepalive_probes",
            s.tcp_keepalive_probes,
            sysctl.tcp.keepalive_probes,
            Max,
        ),
        (
            "net.ipv4.tcp_keepalive_intvl",
            s.tcp_keepalive_intvl,
            sysctl.tcp.keepalive_intvl,
            Max,
        ),
        (
            "net.ipv4.udp_rmem_min",
            s.udp_rmem_min,
            sysctl.udp.rmem_min,
            Min,
        ),
        (
            "net.ipv4.udp_wmem_min",
            s.udp_wmem_min,
            sysctl.udp.wmem_min,
            Min,
        ),
        (
            "net.ipv4.neigh.default.gc_thresh1",
            s.arp_gc_thresh1,
            sysctl.arp.gc_thresh1,
            Min,
        ),
        (
            "net.ipv4.neigh.default.gc_thresh2",
            s.arp_gc_thresh2,
            sysctl.arp.gc_thresh2,
            Min,
        ),
        (
            "net.ipv4.neigh.default.gc_thresh3",
            s.arp_gc_thresh3,
            sysctl.arp.gc_thresh3,
            Min,
        ),
        (
            "net.ipv4.conf.all.arp_ignore",
            s.arp_ignore,
            sysctl.arp.arp_ignore,
            Min,
        ),
        (
            "net.ipv4.conf.all.arp_announce",
            s.arp_announce,
            sysctl.arp.arp_announce,
            Min,
        ),
        (
            "net.ipv4.conf.all.rp_filter",
            s.rp_filter,
            sysctl.rp_filter.all,
            Max,
        ),
        (
            "net.ipv4.conf.default.rp_filter",
            s.rp_filter,
            sysctl.rp_filter.default,
            Max,
        ),
    ];

    let string_settings: Vec<(&str, &str, Option<String>, BoundType)> = vec![
        (
            "net.ipv4.tcp_rmem",
            s.tcp_rmem,
            sysctl.socket_buffer.tcp_rmem.clone(),
            Min,
        ),
        (
            "net.ipv4.tcp_wmem",
            s.tcp_wmem,
            sysctl.socket_buffer.tcp_wmem.clone(),
            Min,
        ),
        (
            "net.ipv4.ip_local_port_range",
            s.ip_local_port_range,
            sysctl.tcp.ip_local_port_range.clone(),
            BoundType::Range,
        ),
        (
            "net.ipv4.udp_mem",
            s.udp_mem,
            sysctl.udp.udp_mem.clone(),
            Min,
        ),
    ];

    let needs_change: Vec<(&str, String)> = settings
        .iter()
        .filter(|(_, suggested, current, bound)| !bound.is_satisfied(*current, *suggested))
        .map(|(key, suggested, _, _)| (*key, suggested.to_string()))
        .chain(
            string_settings
                .iter()
                .filter(|(_, suggested, current, bound)| {
                    !bound.is_str_satisfied(current.as_deref(), suggested)
                })
                .map(|(key, suggested, _, _)| (*key, suggested.to_string())),
        )
        .collect();

    let tso = if s.offload_tso { "on" } else { "off" };
    let gso = if s.offload_gso { "on" } else { "off" };
    let gro = if s.offload_gro { "on" } else { "off" };

    match format {
        OutputFormat::Cmd => {
            if needs_change.is_empty() {
                println!("# All sysctl settings already meet requirements");
            } else {
                for (key, value) in &needs_change {
                    println!("sysctl -w '{}={}'", key, value);
                }
            }
            println!();
            println!("# Interface tuning (ip link)");
            println!("for iface in $(ls /sys/class/net | grep -E '^(eth|ens|eno|enp)'); do");
            println!("    ip link set dev \"$iface\" txqueuelen {}", s.txqueuelen);
            println!("    # MTU {} requires network-wide support", s.mtu);
            println!("    # ip link set dev \"$iface\" mtu {}", s.mtu);
            println!("done");
            println!();
            println!("# Ethtool tuning (ring buffers, coalesce, offloads)");
            println!("for iface in $(ls /sys/class/net | grep -E '^(eth|ens|eno|enp)'); do");
            println!(
                "    ethtool -G \"$iface\" rx {} tx {} 2>/dev/null || true",
                s.ring_rx, s.ring_tx
            );
            println!(
                "    ethtool -C \"$iface\" rx-usecs {} tx-usecs {} 2>/dev/null || true",
                s.coalesce_rx_usecs, s.coalesce_tx_usecs
            );
            println!(
                "    ethtool -K \"$iface\" tso {} gso {} gro {} 2>/dev/null || true",
                tso, gso, gro
            );
            println!("done");
        }
        OutputFormat::Conf => {
            println!(
                "# Sysctl tuning for 10k-node cluster ({} profile)",
                profile.name()
            );
            println!("# Save to /etc/sysctl.d/99-k8s-tuning.conf and run: sysctl --system");
            println!();
            if needs_change.is_empty() {
                println!("# All sysctl settings already meet requirements");
            } else {
                for (key, value) in &needs_change {
                    println!("{} = {}", key, value);
                }
            }
            println!();
            println!("# NOTE: Interface settings (not sysctl) - apply via script or systemd unit:");
            println!("#   ip link set dev <iface> txqueuelen {}", s.txqueuelen);
            println!(
                "#   ip link set dev <iface> mtu {} (requires network support)",
                s.mtu
            );
            println!("#   ethtool -G <iface> rx {} tx {}", s.ring_rx, s.ring_tx);
            println!(
                "#   ethtool -C <iface> rx-usecs {} tx-usecs {}",
                s.coalesce_rx_usecs, s.coalesce_tx_usecs
            );
            println!("#   ethtool -K <iface> tso {} gso {} gro {}", tso, gso, gro);
        }
        OutputFormat::Script => {
            println!("#!/bin/bash");
            println!(
                "# Network tuning for 10k-node cluster ({} profile)",
                profile.name()
            );
            println!("# Run with: sudo bash <script>");
            println!();
            println!("set -e");
            println!();
            println!("# Sysctl settings");
            if needs_change.is_empty() {
                println!("# All sysctl settings already meet requirements");
            } else {
                for (key, value) in &needs_change {
                    println!("sysctl -w '{}={}'", key, value);
                }
            }
            println!();
            println!("# Interface tuning (ip link)");
            println!("for iface in $(ls /sys/class/net | grep -E '^(eth|ens|eno|enp)'); do");
            println!("    ip link set dev \"$iface\" txqueuelen {}", s.txqueuelen);
            println!(
                "    # MTU {} requires network-wide jumbo frame support",
                s.mtu
            );
            println!("    # ip link set dev \"$iface\" mtu {}", s.mtu);
            println!("done");
            println!();
            println!("# Ethtool tuning (ring buffers, coalesce, offloads)");
            println!("for iface in $(ls /sys/class/net | grep -E '^(eth|ens|eno|enp)'); do");
            println!(
                "    ethtool -G \"$iface\" rx {} tx {} 2>/dev/null || true",
                s.ring_rx, s.ring_tx
            );
            println!(
                "    ethtool -C \"$iface\" rx-usecs {} tx-usecs {} 2>/dev/null || true",
                s.coalesce_rx_usecs, s.coalesce_tx_usecs
            );
            println!(
                "    ethtool -K \"$iface\" tso {} gso {} gro {} 2>/dev/null || true",
                tso, gso, gro
            );
            println!("done");
            println!();
            println!("echo 'Network tuning applied successfully'");
        }
    }
}

pub async fn generate_link_output(
    name: &str,
    profile: TuningProfile,
    format: OutputFormat,
) -> Result<(), EthError> {
    use libonm::eth;

    let s = SuggestedValues::for_profile(profile);
    let link = eth::get_link_settings(name).await?;

    let tso = if s.offload_tso { "on" } else { "off" };
    let gso = if s.offload_gso { "on" } else { "off" };
    let gro = if s.offload_gro { "on" } else { "off" };

    match format {
        OutputFormat::Cmd => {
            println!("ip link set dev {} txqueuelen {}", name, s.txqueuelen);
            println!("# MTU {} requires network-wide jumbo frame support", s.mtu);
            println!("# ip link set dev {} mtu {}", name, s.mtu);
            println!();
            println!("ethtool -G {} rx {} tx {}", name, s.ring_rx, s.ring_tx);
            println!(
                "ethtool -C {} rx-usecs {} tx-usecs {}",
                name, s.coalesce_rx_usecs, s.coalesce_tx_usecs
            );
            println!("ethtool -K {} tso {} gso {} gro {}", name, tso, gso, gro);

            {
                let l = &link;
                println!();
                println!("# Current values for {}:", name);
                if let Some(v) = l.mtu {
                    println!("#   mtu: {}", v);
                }
                if let Some(v) = l.txqueuelen {
                    println!("#   txqueuelen: {}", v);
                }
                if let Some(v) = l.gso_max_size {
                    println!("#   gso_max_size: {}", v);
                }
                if let Some(v) = l.tso_max_size {
                    println!("#   tso_max_size: {}", v);
                }
            }
        }
        OutputFormat::Conf => {
            println!(
                "# IP link and ethtool tuning for {} ({} profile)",
                name,
                profile.name()
            );
            println!("# Apply via script or systemd unit");
            println!();
            println!("# ip link settings:");
            println!("ip link set dev {} txqueuelen {}", name, s.txqueuelen);
            println!(
                "# ip link set dev {} mtu {} (requires network-wide jumbo frame support)",
                name, s.mtu
            );
            println!();
            println!("# ethtool settings:");
            println!("ethtool -G {} rx {} tx {}", name, s.ring_rx, s.ring_tx);
            println!(
                "ethtool -C {} rx-usecs {} tx-usecs {}",
                name, s.coalesce_rx_usecs, s.coalesce_tx_usecs
            );
            println!("ethtool -K {} tso {} gso {} gro {}", name, tso, gso, gro);
        }
        OutputFormat::Script => {
            println!("#!/bin/bash");
            println!(
                "# IP link and ethtool tuning for {} ({} profile)",
                name,
                profile.name()
            );
            println!("# Run with: sudo bash <script>");
            println!();
            println!("set -e");
            println!("IFACE={}", name);
            println!();
            println!("ip link set dev \"$IFACE\" txqueuelen {}", s.txqueuelen);
            println!("# MTU {} requires network-wide jumbo frame support", s.mtu);
            println!("# ip link set dev \"$IFACE\" mtu {}", s.mtu);
            println!();
            println!(
                "ethtool -G \"$IFACE\" rx {} tx {} 2>/dev/null || true",
                s.ring_rx, s.ring_tx
            );
            println!(
                "ethtool -C \"$IFACE\" rx-usecs {} tx-usecs {} 2>/dev/null || true",
                s.coalesce_rx_usecs, s.coalesce_tx_usecs
            );
            println!(
                "ethtool -K \"$IFACE\" tso {} gso {} gro {} 2>/dev/null || true",
                tso, gso, gro
            );
            println!();
            println!("echo 'Link tuning for {} applied successfully'", name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_bounds_compare_each_numeric_field() {
        assert!(BoundType::Min
            .is_str_satisfied(Some("4096 2097152 268435456"), "4096 1048576 134217728"));
        assert!(!BoundType::Min
            .is_str_satisfied(Some("4096 524288 268435456"), "4096 1048576 134217728"));
        assert!(!BoundType::Min.is_str_satisfied(Some("4096 1048576"), "4096 1048576 1"));
        assert!(BoundType::Range.is_str_satisfied(Some("512 65535"), "1024 65535"));
        assert!(!BoundType::Range.is_str_satisfied(Some("32768 60999"), "1024 65535"));
    }

    #[test]
    fn invalid_profile_and_format_are_rejected() {
        assert!(TuningProfile::from_str("typo").is_err());
        assert!(OutputFormat::from_str("yaml").is_err());
    }
}
