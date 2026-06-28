use comfy_table::{presets::NOTHING, Cell, Color, Table};
use libonm::eth::{self, EthError};

use crate::format;

#[derive(Clone, Copy)]
pub enum TuningProfile {
    ControlPlane,
    Worker,
    Gateway,
}

impl TuningProfile {
    pub fn from_str(s: &str) -> Result<Self, EthError> {
        match s.to_lowercase().as_str() {
            "control-plane" | "controlplane" | "cp" | "master" => Ok(TuningProfile::ControlPlane),
            "worker" => Ok(TuningProfile::Worker),
            "gateway" | "router" => Ok(TuningProfile::Gateway),
            _ => Err(EthError::InvalidConfig(format!(
                "unknown profile '{s}' (expected 'worker', 'control-plane', or 'gateway')"
            ))),
        }
    }

    #[allow(dead_code)]
    pub fn header_suffix(&self) -> &'static str {
        match self {
            TuningProfile::ControlPlane => "Suggested (CP 10k)",
            TuningProfile::Worker => "Suggested (Worker 10k)",
            TuningProfile::Gateway => "Suggested (Gateway)",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            TuningProfile::ControlPlane => "control-plane",
            TuningProfile::Worker => "worker",
            TuningProfile::Gateway => "gateway",
        }
    }

    fn is_gateway(&self) -> bool {
        matches!(self, TuningProfile::Gateway)
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
    /// Current scalar value must exactly match the suggestion.
    Exact,
}

impl BoundType {
    pub fn prefix(&self) -> &'static str {
        match self {
            BoundType::Min => ">= ",
            BoundType::Max => "<= ",
            BoundType::Exact => "= ",
        }
    }

    pub fn is_satisfied(&self, current: Option<u64>, suggested: u64) -> bool {
        match current {
            None => false,
            Some(val) => match self {
                BoundType::Min => val >= suggested,
                BoundType::Max => val <= suggested,
                BoundType::Exact => val == suggested,
            },
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
    pub conntrack_tcp_timeout_established: u64,
    pub conntrack_tcp_timeout_close_wait: u64,
    pub conntrack_udp_timeout: u64,
    pub conntrack_udp_timeout_stream: u64,
    // Socket buffers
    pub rmem_max: u64,
    pub wmem_max: u64,
    pub tcp_rmem: &'static str,
    pub tcp_wmem: &'static str,
    pub netdev_max_backlog: u64,
    pub udp_rmem_min: u64,
    // TCP settings
    pub somaxconn: u64,
    pub tcp_max_syn_backlog: u64,
    pub tcp_fin_timeout: u64,
    pub tcp_keepalive_time: u64,
    pub tcp_keepalive_probes: u64,
    pub tcp_keepalive_intvl: u64,
    // Investigation-only network topology candidate
    pub rp_filter: u64,
    // Interface settings (ip link)
    pub txqueuelen: u64,
    pub mtu: u64,
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
            TuningProfile::Gateway => Self::gateway(),
        }
    }

    // Control plane: prioritizes API server and etcd throughput. Conntrack capacity
    // is CPU-derived because node role and cluster size do not determine flow count.
    fn control_plane() -> Self {
        let conntrack_max = suggested_conntrack_max();
        Self {
            conntrack_max,
            conntrack_tcp_timeout_established: 86400,
            conntrack_tcp_timeout_close_wait: 3600,
            conntrack_udp_timeout: 30,
            conntrack_udp_timeout_stream: 300,

            rmem_max: 268_435_456, // 256MB - etcd/API server traffic
            wmem_max: 268_435_456,
            tcp_rmem: "4096 2097152 268435456",
            tcp_wmem: "4096 2097152 268435456",
            netdev_max_backlog: 50000,
            udp_rmem_min: 16_384,

            somaxconn: 65535,
            tcp_max_syn_backlog: 65535,
            tcp_fin_timeout: 15,
            tcp_keepalive_time: 600,
            tcp_keepalive_probes: 3,
            tcp_keepalive_intvl: 15,
            rp_filter: 0,

            txqueuelen: 10000, // High queue for API server traffic bursts
            mtu: 9000,         // Jumbo frames (requires network support)

            ring_rx: 4096,
            ring_tx: 4096,
            coalesce_rx_usecs: 50, // Balance latency vs CPU
            coalesce_tx_usecs: 50,
            offload_tso: true,
            offload_gso: true,
            offload_gro: true,
        }
    }

    // Worker: handles pod traffic. Conntrack capacity is CPU-derived to match the
    // kube-proxy default rather than assuming every worker has the same hardware.
    fn worker() -> Self {
        let conntrack_max = suggested_conntrack_max();
        Self {
            conntrack_max,
            conntrack_tcp_timeout_established: 86400,
            conntrack_tcp_timeout_close_wait: 3600,
            conntrack_udp_timeout: 30,
            conntrack_udp_timeout_stream: 300,

            rmem_max: 134_217_728, // 128MB
            wmem_max: 134_217_728,
            tcp_rmem: "4096 1048576 134217728",
            tcp_wmem: "4096 1048576 134217728",
            netdev_max_backlog: 30000,
            udp_rmem_min: 16_384,

            somaxconn: 32768,
            tcp_max_syn_backlog: 32768,
            tcp_fin_timeout: 15,
            tcp_keepalive_time: 600,
            tcp_keepalive_probes: 3,
            tcp_keepalive_intvl: 15,
            rp_filter: 0,

            txqueuelen: 5000, // Moderate queue for pod traffic
            mtu: 9000,        // Jumbo frames (requires network support)

            ring_rx: 2048,
            ring_tx: 2048,
            coalesce_rx_usecs: 100, // Higher coalescing for throughput
            coalesce_tx_usecs: 100,
            offload_tso: true,
            offload_gso: true,
            offload_gro: true,
        }
    }

    // Gateway: forwards traffic between interfaces. Endpoint-only TCP and
    // socket settings remain observational in this profile.
    fn gateway() -> Self {
        let conntrack_max = suggested_conntrack_max();
        Self {
            conntrack_max,
            conntrack_tcp_timeout_established: 86400,
            conntrack_tcp_timeout_close_wait: 3600,
            conntrack_udp_timeout: 30,
            conntrack_udp_timeout_stream: 300,

            rmem_max: 134_217_728,
            wmem_max: 134_217_728,
            tcp_rmem: "4096 1048576 134217728",
            tcp_wmem: "4096 1048576 134217728",
            netdev_max_backlog: 30000,
            udp_rmem_min: 16_384,

            somaxconn: 32768,
            tcp_max_syn_backlog: 32768,
            tcp_fin_timeout: 15,
            tcp_keepalive_time: 600,
            tcp_keepalive_probes: 3,
            tcp_keepalive_intvl: 15,
            rp_filter: 2,

            txqueuelen: 2000,
            mtu: 1500,
            ring_rx: 2048,
            ring_tx: 2048,
            coalesce_rx_usecs: 50,
            coalesce_tx_usecs: 50,
            offload_tso: true,
            offload_gso: true,
            offload_gro: true,
        }
    }
}

fn conntrack_max_for_cores(cores: u64) -> u64 {
    cores.saturating_mul(32_768).max(131_072)
}

fn suggested_conntrack_max() -> u64 {
    let cores = std::thread::available_parallelism()
        .map(|value| value.get() as u64)
        .unwrap_or(1);
    conntrack_max_for_cores(cores)
}

fn investigation_settings(
    profile: TuningProfile,
    s: &SuggestedValues,
    sysctl: &eth::NetworkSysctl,
    interfaces: &[eth::EthInterface],
) -> Vec<(String, String)> {
    let mut settings = vec![
        ("net.core.rmem_max".to_string(), s.rmem_max.to_string()),
        ("net.core.wmem_max".to_string(), s.wmem_max.to_string()),
        ("net.ipv4.tcp_rmem".to_string(), s.tcp_rmem.to_string()),
        ("net.ipv4.tcp_wmem".to_string(), s.tcp_wmem.to_string()),
        (
            "net.core.netdev_max_backlog".to_string(),
            s.netdev_max_backlog.to_string(),
        ),
        (
            "net.ipv4.udp_rmem_min".to_string(),
            s.udp_rmem_min.to_string(),
        ),
        ("net.core.somaxconn".to_string(), s.somaxconn.to_string()),
        (
            "net.ipv4.tcp_max_syn_backlog".to_string(),
            s.tcp_max_syn_backlog.to_string(),
        ),
        (
            "net.ipv4.tcp_fin_timeout".to_string(),
            s.tcp_fin_timeout.to_string(),
        ),
        (
            "net.ipv4.tcp_keepalive_time".to_string(),
            s.tcp_keepalive_time.to_string(),
        ),
        (
            "net.ipv4.tcp_keepalive_probes".to_string(),
            s.tcp_keepalive_probes.to_string(),
        ),
        (
            "net.ipv4.tcp_keepalive_intvl".to_string(),
            s.tcp_keepalive_intvl.to_string(),
        ),
    ];

    settings.extend(
        sysctl
            .rp_filter
            .interfaces
            .iter()
            .filter(|(name, _)| is_physical_interface(name, interfaces))
            .map(|(name, _)| {
                (
                    format!("net.ipv4.conf.{name}.rp_filter"),
                    s.rp_filter.to_string(),
                )
            }),
    );

    if !profile.is_gateway() {
        settings.extend([
            ("net.ipv4.ip_forward".to_string(), "1".to_string()),
            ("net.ipv6.conf.all.forwarding".to_string(), "1".to_string()),
        ]);
    }

    settings
}

fn print_investigation_settings(
    format: OutputFormat,
    profile: TuningProfile,
    s: &SuggestedValues,
    sysctl: &eth::NetworkSysctl,
    interfaces: &[eth::EthInterface],
) {
    println!();
    println!("# Starting candidates requiring measured pressure and workload validation:");
    println!("# Check ethctl stats deltas, RAM/BDP, application timeouts, and CNI topology first.");
    for (key, value) in investigation_settings(profile, s, sysctl, interfaces) {
        println!("{}", investigation_output_line(format, &key, &value));
    }
}

fn investigation_output_line(format: OutputFormat, key: &str, value: &str) -> String {
    match format {
        OutputFormat::Conf => format!("# {key} = {value}"),
        OutputFormat::Cmd | OutputFormat::Script => format!("# sysctl -w '{key}={value}'"),
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
        let interfaces = eth::list_interfaces()?;
        generate_sysctl_output(profile, format, &interfaces);
        return Ok(());
    }

    let interfaces = eth::list_interfaces()?;
    let s = SuggestedValues::for_profile(profile);
    let sysctl = eth::get_network_sysctl();

    let mut table = Table::new();
    table.load_preset(NOTHING);

    add_first_section(&mut table, "Overview");
    table.add_row(vec!["  Profile", profile.name(), "-"]);

    add_sysctl_rows(&mut table, &sysctl, &s, profile, &interfaces);

    println!("{table}");
    println!();

    print_interfaces_table(&interfaces);

    Ok(())
}

fn print_interfaces_table(interfaces: &[eth::EthInterface]) {
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
        let mtu_str = iface.mtu.to_string();
        let txq_str = iface.txqueuelen.to_string();
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

#[allow(dead_code)]
pub fn print_sysctl_tables(profile: TuningProfile) {
    use libonm::eth;

    let sysctl = eth::get_network_sysctl();
    let interfaces = eth::list_interfaces().unwrap_or_default();
    let s = SuggestedValues::for_profile(profile);

    let mut table = Table::new();
    table.load_preset(NOTHING);

    add_first_section(&mut table, "Overview");
    table.add_row(vec!["  Profile", profile.name(), "-"]);

    add_sysctl_rows(&mut table, &sysctl, &s, profile, &interfaces);

    println!("{table}");
}

fn add_sysctl_rows(
    table: &mut Table,
    sysctl: &eth::NetworkSysctl,
    s: &SuggestedValues,
    profile: TuningProfile,
    interfaces: &[eth::EthInterface],
) {
    use BoundType::{Max, Min};

    add_section(table, "Packet Forwarding");
    if profile.is_gateway() {
        if sysctl.forwarding.ipv4.is_some() {
            add_row(
                table,
                "  net.ipv4.ip_forward",
                sysctl.forwarding.ipv4,
                1,
                BoundType::Exact,
            );
        } else {
            add_info_row(table, "  net.ipv4.ip_forward", None, "1");
        }
        if sysctl.forwarding.ipv6.is_some() {
            add_row(
                table,
                "  net.ipv6.conf.all.forwarding",
                sysctl.forwarding.ipv6,
                1,
                BoundType::Exact,
            );
        } else {
            add_info_row(table, "  net.ipv6.conf.all.forwarding", None, "1");
        }
    } else {
        add_info_row(table, "  net.ipv4.ip_forward", sysctl.forwarding.ipv4, "1");
        add_info_row(
            table,
            "  net.ipv6.conf.all.forwarding",
            sysctl.forwarding.ipv6,
            "1",
        );
    }

    add_section(table, "Connection Tracking");
    add_row_count(
        table,
        "  nf_conntrack_max",
        sysctl.conntrack.max,
        s.conntrack_max,
        Min,
    );
    add_info_row_count(
        table,
        "  nf_conntrack_buckets",
        sysctl.conntrack.buckets,
        (s.conntrack_max / 4).clamp(1_024, 262_144),
    );
    add_row(
        table,
        "  nf_conntrack_tcp_timeout_established",
        sysctl.conntrack.tcp_timeout_established,
        s.conntrack_tcp_timeout_established,
        Max,
    );
    add_info_row(
        table,
        "  nf_conntrack_tcp_timeout_time_wait",
        sysctl.conntrack.tcp_timeout_time_wait,
        "kernel/CNI-specific",
    );
    add_row(
        table,
        "  nf_conntrack_udp_timeout",
        sysctl.conntrack.udp_timeout,
        s.conntrack_udp_timeout,
        Max,
    );
    add_row(
        table,
        "  nf_conntrack_udp_timeout_stream",
        sysctl.conntrack.udp_timeout_stream,
        s.conntrack_udp_timeout_stream,
        Max,
    );
    add_row(
        table,
        "  nf_conntrack_tcp_timeout_close_wait",
        sysctl.conntrack.tcp_timeout_close_wait,
        s.conntrack_tcp_timeout_close_wait,
        Max,
    );
    add_info_row(
        table,
        "  nf_conntrack_tcp_timeout_fin_wait",
        sysctl.conntrack.tcp_timeout_fin_wait,
        "kernel/CNI-specific",
    );
    add_info_row(
        table,
        "  nf_conntrack_tcp_max_retrans",
        sysctl.conntrack.tcp_max_retrans,
        "kernel default",
    );

    add_section(table, "Socket Buffers");
    add_info_row_bytes(
        table,
        "  net.core.rmem_max",
        sysctl.socket_buffer.rmem_max,
        &format_size(s.rmem_max),
    );
    add_info_row_bytes(
        table,
        "  net.core.wmem_max",
        sysctl.socket_buffer.wmem_max,
        &format_size(s.wmem_max),
    );
    add_info_row_bytes(
        table,
        "  net.core.rmem_default",
        sysctl.socket_buffer.rmem_default,
        "kernel default",
    );
    add_info_row_bytes(
        table,
        "  net.core.wmem_default",
        sysctl.socket_buffer.wmem_default,
        "kernel default",
    );
    add_info_row_str(
        table,
        "  net.ipv4.tcp_rmem",
        sysctl
            .socket_buffer
            .tcp_rmem
            .as_deref()
            .map(|value| format_count_list(value, Some(CountFormat::Bytes))),
        &format_count_list(s.tcp_rmem, Some(CountFormat::Bytes)),
    );
    add_info_row_str(
        table,
        "  net.ipv4.tcp_wmem",
        sysctl
            .socket_buffer
            .tcp_wmem
            .as_deref()
            .map(|value| format_count_list(value, Some(CountFormat::Bytes))),
        &format_count_list(s.tcp_wmem, Some(CountFormat::Bytes)),
    );
    add_info_row_count(
        table,
        "  net.core.netdev_max_backlog",
        sysctl.socket_buffer.netdev_max_backlog,
        s.netdev_max_backlog,
    );
    add_info_row(
        table,
        "  net.core.netdev_budget",
        sysctl.socket_buffer.netdev_budget,
        "300",
    );
    add_info_row(
        table,
        "  net.core.netdev_budget_usecs",
        sysctl.socket_buffer.netdev_budget_usecs,
        "2000",
    );
    add_info_row_bytes(
        table,
        "  net.ipv4.udp_rmem_min",
        sysctl.udp.rmem_min,
        &format_size(s.udp_rmem_min),
    );
    add_info_row_bytes(
        table,
        "  net.ipv4.udp_wmem_min (unused)",
        sysctl.udp.wmem_min,
        "unused",
    );
    add_info_row_str(
        table,
        "  net.ipv4.udp_mem (pages)",
        sysctl
            .udp
            .udp_mem
            .as_deref()
            .map(|value| format_count_list(value, Some(CountFormat::Decimal))),
        "auto",
    );

    add_section(table, "TCP Settings");
    table.add_row(vec![
        "  net.ipv4.tcp_tw_reuse".to_string(),
        sysctl
            .tcp
            .tw_reuse
            .map_or("-".to_string(), |value| value.to_string()),
        "kernel default".to_string(),
    ]);
    add_info_row_count(
        table,
        "  net.core.somaxconn",
        sysctl.tcp.somaxconn,
        s.somaxconn,
    );
    add_info_row_count(
        table,
        "  net.ipv4.tcp_max_syn_backlog",
        sysctl.tcp.max_syn_backlog,
        s.tcp_max_syn_backlog,
    );
    add_info_row(
        table,
        "  net.ipv4.tcp_fin_timeout",
        sysctl.tcp.fin_timeout,
        "application-specific",
    );
    add_info_row(
        table,
        "  net.ipv4.tcp_keepalive_time",
        sysctl.tcp.keepalive_time,
        "application-specific",
    );
    add_info_row(
        table,
        "  net.ipv4.tcp_keepalive_probes",
        sysctl.tcp.keepalive_probes,
        "application-specific",
    );
    add_info_row(
        table,
        "  net.ipv4.tcp_keepalive_intvl",
        sysctl.tcp.keepalive_intvl,
        "application-specific",
    );
    add_info_row_str(
        table,
        "  net.ipv4.ip_local_port_range",
        sysctl.tcp.ip_local_port_range.clone(),
        "kernel default / capacity-check",
    );
    add_info_row_str(
        table,
        "  net.ipv4.ip_local_reserved_ports",
        sysctl.tcp.ip_local_reserved_ports.clone(),
        "NodePort/app-specific",
    );
    let ephemeral_capacity = usable_ephemeral_ports(
        sysctl.tcp.ip_local_port_range.as_deref(),
        sysctl.tcp.ip_local_reserved_ports.as_deref(),
    );
    table.add_row(vec![
        "  usable ephemeral ports".to_string(),
        ephemeral_capacity
            .map(format_count)
            .unwrap_or("-".to_string()),
        "monitor TIME_WAIT/exhaustion".to_string(),
    ]);

    add_section(table, "ARP / Neighbor Table");
    add_info_row(
        table,
        "  net.ipv4.neigh.default.gc_thresh1",
        sysctl.arp.gc_thresh1,
        "derive from neighbor occupancy",
    );
    add_info_row(
        table,
        "  net.ipv4.neigh.default.gc_thresh2",
        sysctl.arp.gc_thresh2,
        "derive from neighbor occupancy",
    );
    add_info_row(
        table,
        "  net.ipv4.neigh.default.gc_thresh3",
        sysctl.arp.gc_thresh3,
        "derive from neighbor occupancy",
    );
    add_info_row(
        table,
        "  net.ipv4.conf.all.arp_ignore",
        sysctl.arp.arp_ignore,
        "topology-specific",
    );
    add_info_row(
        table,
        "  net.ipv4.conf.all.arp_announce",
        sysctl.arp.arp_announce,
        "topology-specific",
    );
    add_info_row(
        table,
        "  net.ipv6.neigh.default.gc_thresh1",
        sysctl.arp.ipv6_gc_thresh1,
        "derive from neighbor occupancy",
    );
    add_info_row(
        table,
        "  net.ipv6.neigh.default.gc_thresh2",
        sysctl.arp.ipv6_gc_thresh2,
        "derive from neighbor occupancy",
    );
    add_info_row(
        table,
        "  net.ipv6.neigh.default.gc_thresh3",
        sysctl.arp.ipv6_gc_thresh3,
        "derive from neighbor occupancy",
    );

    add_section(table, "Reverse Path Filtering");
    add_info_row(
        table,
        "  net.ipv4.conf.all.rp_filter",
        sysctl.rp_filter.all,
        "CNI/topology-specific",
    );
    add_info_row(
        table,
        "  net.ipv4.conf.default.rp_filter",
        sysctl.rp_filter.default,
        "CNI/topology-specific",
    );
    for (name, value) in &sysctl.rp_filter.interfaces {
        let setting = format!("  net.ipv4.conf.{name}.rp_filter");
        if is_physical_interface(name, interfaces) {
            add_info_row(table, &setting, Some(*value), &s.rp_filter.to_string());
        } else {
            table.add_row(vec![setting, value.to_string(), "not managed".to_string()]);
        }
    }
}

fn is_physical_interface(name: &str, interfaces: &[eth::EthInterface]) -> bool {
    interfaces
        .iter()
        .any(|interface| interface.name == name && interface.interface_type.is_physical())
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
        add_row_u32_bytes(&mut table, "  mtu", l.mtu, 0);
        add_row_u32_bytes(&mut table, "  min_mtu", l.min_mtu, 0);
        add_row_u32_bytes(&mut table, "  max_mtu", l.max_mtu, 0);
        add_row_u32(&mut table, "  txqueuelen", l.txqueuelen, 0);
        add_row_u32(&mut table, "  num_tx_queues", l.num_tx_queues, 0);
        add_row_u32(&mut table, "  num_rx_queues", l.num_rx_queues, 0);
        add_row_u32_bytes(&mut table, "  gso_max_size", l.gso_max_size, 0);
        add_row_u32(&mut table, "  gso_max_segs", l.gso_max_segs, 0);
        add_row_u32_bytes(&mut table, "  gro_max_size", l.gro_max_size, 0);
        add_row_u32_bytes(&mut table, "  tso_max_size", l.tso_max_size, 0);
        add_row_u32(&mut table, "  tso_max_segs", l.tso_max_segs, 0);
        table.add_row(vec![
            "  qdisc".to_string(),
            l.qdisc.clone().unwrap_or("-".to_string()),
            "-".to_string(),
        ]);
        add_row_u32(&mut table, "  group", l.group, 0);
    }

    add_section(&mut table, "Ethtool Settings");
    {
        let e = &ethtool;
        let ring_rx = clamp_to_device_max(s.ring_rx as u32, e.ring.rx_max);
        let ring_tx = clamp_to_device_max(s.ring_tx as u32, e.ring.tx_max);
        add_row_u32(&mut table, "  ring_rx", e.ring.rx, ring_rx);
        add_row_u32(&mut table, "  ring_rx_max", e.ring.rx_max, 0);
        add_row_u32(&mut table, "  ring_tx", e.ring.tx, ring_tx);
        add_row_u32(&mut table, "  ring_tx_max", e.ring.tx_max, 0);
        add_row_u32(&mut table, "  coalesce_rx_usecs", e.coalesce.rx_usecs, 0);
        add_row_u32(&mut table, "  coalesce_tx_usecs", e.coalesce.tx_usecs, 0);
        add_info_row_bool(&mut table, "  offload_tso", e.offload.tso, s.offload_tso);
        add_info_row_bool(&mut table, "  offload_gso", e.offload.gso, s.offload_gso);
        add_info_row_bool(&mut table, "  offload_gro", e.offload.gro, s.offload_gro);
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
    format::binary(bytes)
}

fn format_count(value: u64) -> String {
    format::count(value)
}

fn format_size_u32(bytes: u32) -> String {
    format_size(bytes as u64)
}

#[derive(Clone, Copy)]
enum CountFormat {
    Decimal,
    Bytes,
}

fn format_count_list(value: &str, format: Option<CountFormat>) -> String {
    value
        .split_whitespace()
        .map(|item| match format {
            Some(CountFormat::Decimal) => item
                .parse::<u64>()
                .map(format_count)
                .unwrap_or(item.to_string()),
            Some(CountFormat::Bytes) => item
                .parse::<u64>()
                .map(format_size)
                .unwrap_or(item.to_string()),
            None => item.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn usable_ephemeral_ports(range: Option<&str>, reserved: Option<&str>) -> Option<u64> {
    let values = range?
        .split_whitespace()
        .map(str::parse::<u16>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if values.len() != 2 || values[1] < values[0] {
        return None;
    }
    let (start, end) = (values[0], values[1]);
    let mut intervals = reserved
        .unwrap_or_default()
        .split(',')
        .filter(|value| !value.is_empty())
        .filter_map(|value| {
            let mut bounds = value.splitn(2, '-');
            let first = bounds.next()?.parse::<u16>().ok()?;
            let last = bounds
                .next()
                .and_then(|bound| bound.parse::<u16>().ok())
                .unwrap_or(first);
            (last >= start && first <= end).then_some((first.max(start), last.min(end)))
        })
        .collect::<Vec<_>>();
    intervals.sort_unstable();

    let mut reserved_count = 0u64;
    let mut merged: Option<(u16, u16)> = None;
    for (first, last) in intervals {
        match merged {
            Some((merged_first, merged_last)) if first as u32 <= merged_last as u32 + 1 => {
                merged = Some((merged_first, merged_last.max(last)));
            }
            Some((merged_first, merged_last)) => {
                reserved_count += (merged_last - merged_first) as u64 + 1;
                merged = Some((first, last));
            }
            None => merged = Some((first, last)),
        }
    }
    if let Some((first, last)) = merged {
        reserved_count += (last - first) as u64 + 1;
    }

    Some((end - start) as u64 + 1 - reserved_count)
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

fn investigation_value(preferred: &str) -> String {
    format!("{preferred} (?)")
}

fn add_info_row_bool(table: &mut Table, name: &str, value: Option<bool>, preferred: bool) {
    table.add_row(vec![
        name.to_string(),
        value
            .map(|v| if v { "on" } else { "off" }.to_string())
            .unwrap_or("-".to_string()),
        investigation_value(if preferred { "on" } else { "off" }),
    ]);
}

fn clamp_to_device_max(target: u32, maximum: Option<u32>) -> u32 {
    maximum.map_or(target, |value| target.min(value))
}

fn safe_ring_target(target: u32, maximum: Option<u32>, current: Option<u32>) -> Option<u32> {
    maximum.map(|value| target.min(value)).or(current)
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

fn add_info_row(table: &mut Table, name: &str, value: Option<u64>, preferred: &str) {
    table.add_row(vec![
        name.to_string(),
        value.map(|v| v.to_string()).unwrap_or("-".to_string()),
        investigation_value(preferred),
    ]);
}

fn add_row_count(
    table: &mut Table,
    name: &str,
    value: Option<u64>,
    suggested: u64,
    bound: BoundType,
) {
    let suggested_str = if bound.is_satisfied(value, suggested) {
        "OK".to_string()
    } else {
        format!("{}{}", bound.prefix(), format_count(suggested))
    };
    table.add_row(vec![
        name.to_string(),
        value.map(format_count).unwrap_or("-".to_string()),
        suggested_str,
    ]);
}

fn add_info_row_count(table: &mut Table, name: &str, value: Option<u64>, preferred: u64) {
    table.add_row(vec![
        name.to_string(),
        value.map(format_count).unwrap_or("-".to_string()),
        investigation_value(&format_count(preferred)),
    ]);
}

fn add_info_row_bytes(table: &mut Table, name: &str, value: Option<u64>, preferred: &str) {
    table.add_row(vec![
        name.to_string(),
        value.map(format_size).unwrap_or("-".to_string()),
        investigation_value(preferred),
    ]);
}

fn add_info_row_str(table: &mut Table, name: &str, value: Option<String>, preferred: &str) {
    table.add_row(vec![
        name.to_string(),
        value
            .as_deref()
            .map(|value| format_count_list(value, None))
            .unwrap_or("-".to_string()),
        investigation_value(&format_count_list(preferred, None)),
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
        (
            "net.netfilter.nf_conntrack_udp_timeout",
            sysctl.conntrack.udp_timeout,
        ),
        (
            "net.netfilter.nf_conntrack_udp_timeout_stream",
            sysctl.conntrack.udp_timeout_stream,
        ),
        ("net.core.rmem_max", sysctl.socket_buffer.rmem_max),
        ("net.core.wmem_max", sysctl.socket_buffer.wmem_max),
        ("net.core.rmem_default", sysctl.socket_buffer.rmem_default),
        ("net.core.wmem_default", sysctl.socket_buffer.wmem_default),
        (
            "net.core.netdev_max_backlog",
            sysctl.socket_buffer.netdev_max_backlog,
        ),
        ("net.core.netdev_budget", sysctl.socket_buffer.netdev_budget),
        (
            "net.core.netdev_budget_usecs",
            sysctl.socket_buffer.netdev_budget_usecs,
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
        ("net.ipv4.ip_forward", sysctl.forwarding.ipv4),
        ("net.ipv6.conf.all.forwarding", sysctl.forwarding.ipv6),
    ];

    let string_settings: Vec<(&str, Option<String>)> = vec![
        ("net.ipv4.tcp_rmem", sysctl.socket_buffer.tcp_rmem.clone()),
        ("net.ipv4.tcp_wmem", sysctl.socket_buffer.tcp_wmem.clone()),
        (
            "net.ipv4.ip_local_port_range",
            sysctl.tcp.ip_local_port_range.clone(),
        ),
        (
            "net.ipv4.ip_local_reserved_ports",
            sysctl.tcp.ip_local_reserved_ports.clone(),
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

pub fn generate_sysctl_output(
    profile: TuningProfile,
    format: OutputFormat,
    interfaces: &[eth::EthInterface],
) {
    use libonm::eth;
    use BoundType::{Max, Min};

    let s = SuggestedValues::for_profile(profile);
    let sysctl = eth::get_network_sysctl();

    let mut settings: Vec<(&str, u64, Option<u64>, BoundType)> = vec![
        (
            "net.netfilter.nf_conntrack_max",
            s.conntrack_max,
            sysctl.conntrack.max,
            Min,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_established",
            s.conntrack_tcp_timeout_established,
            sysctl.conntrack.tcp_timeout_established,
            Max,
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_close_wait",
            s.conntrack_tcp_timeout_close_wait,
            sysctl.conntrack.tcp_timeout_close_wait,
            Max,
        ),
        (
            "net.netfilter.nf_conntrack_udp_timeout",
            s.conntrack_udp_timeout,
            sysctl.conntrack.udp_timeout,
            Max,
        ),
        (
            "net.netfilter.nf_conntrack_udp_timeout_stream",
            s.conntrack_udp_timeout_stream,
            sysctl.conntrack.udp_timeout_stream,
            Max,
        ),
    ];

    if profile.is_gateway() {
        settings.push((
            "net.ipv4.ip_forward",
            1,
            sysctl.forwarding.ipv4,
            BoundType::Exact,
        ));
        if let Some(current) = sysctl.forwarding.ipv6 {
            settings.push((
                "net.ipv6.conf.all.forwarding",
                1,
                Some(current),
                BoundType::Exact,
            ));
        }
    }

    let needs_change: Vec<(&str, String)> = settings
        .iter()
        .filter(|(_, suggested, current, bound)| !bound.is_satisfied(*current, *suggested))
        .map(|(key, suggested, _, _)| (*key, suggested.to_string()))
        .collect();

    match format {
        OutputFormat::Cmd => {
            if needs_change.is_empty() {
                println!("# All sysctl settings already meet requirements");
            } else {
                for (key, value) in &needs_change {
                    println!("sysctl -w '{}={}'", key, value);
                }
            }
            print_investigation_settings(format, profile, &s, &sysctl, interfaces);
            println!("# Device tuning is topology-specific; inspect one NIC with: ethctl link --name <iface>");
            if profile.is_gateway() {
                println!(
                    "# Verify that the firewall forwarding policy permits only intended traffic."
                );
            }
        }
        OutputFormat::Conf => {
            println!("# Network sysctl tuning ({} profile)", profile.name());
            println!("# Save to /etc/sysctl.d/99-k8s-tuning.conf and run: sysctl --system");
            println!();
            if needs_change.is_empty() {
                println!("# All sysctl settings already meet requirements");
            } else {
                for (key, value) in &needs_change {
                    println!("{} = {}", key, value);
                }
            }
            print_investigation_settings(format, profile, &s, &sysctl, interfaces);
            println!();
            println!("# Device tuning is intentionally omitted; MTU, queues, coalescing, and offloads depend on the NIC and CNI path.");
            if profile.is_gateway() {
                println!("# Verify firewall forwarding policy separately; this profile does not modify firewall rules.");
            }
        }
        OutputFormat::Script => {
            println!("#!/bin/bash");
            println!("# Network tuning ({} profile)", profile.name());
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
            print_investigation_settings(format, profile, &s, &sysctl, interfaces);
            println!();
            println!("# Device tuning is intentionally omitted; use ethctl link after validating the NIC and end-to-end path.");
            if profile.is_gateway() {
                println!("# Firewall forwarding policy is not changed by this script.");
            }
            println!("echo 'Network sysctl tuning applied successfully'");
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
    let ethtool = eth::get_ethtool_settings(name).await?;
    let ring_rx = safe_ring_target(s.ring_rx as u32, ethtool.ring.rx_max, ethtool.ring.rx);
    let ring_tx = safe_ring_target(s.ring_tx as u32, ethtool.ring.tx_max, ethtool.ring.tx);

    let tso = if s.offload_tso { "on" } else { "off" };
    let gso = if s.offload_gso { "on" } else { "off" };
    let gro = if s.offload_gro { "on" } else { "off" };

    match format {
        OutputFormat::Cmd => {
            if let (Some(rx), Some(tx)) = (ring_rx, ring_tx) {
                println!("# Candidate only: ethtool -G {} rx {} tx {}", name, rx, tx);
            }
            println!(
                "# MTU, TX queue length, coalescing, and offloads require workload/CNI validation."
            );
            println!(
                "# Candidate only: ip link set dev {} txqueuelen {}",
                name, s.txqueuelen
            );
            println!("# Candidate only: ip link set dev {} mtu {}", name, s.mtu);
            println!(
                "# Candidate only: ethtool -C {} rx-usecs {} tx-usecs {}",
                name, s.coalesce_rx_usecs, s.coalesce_tx_usecs
            );
            println!(
                "# Candidate only: ethtool -K {} tso {} gso {} gro {}",
                name, tso, gso, gro
            );

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
            println!("# Device-dependent candidates (review before enabling):");
            println!("# ip link set dev {} txqueuelen {}", name, s.txqueuelen);
            println!("# ip link set dev {} mtu {}", name, s.mtu);
            println!();
            if let (Some(rx), Some(tx)) = (ring_rx, ring_tx) {
                println!("# ethtool -G {} rx {} tx {}", name, rx, tx);
            }
            println!(
                "# ethtool -C {} rx-usecs {} tx-usecs {}",
                name, s.coalesce_rx_usecs, s.coalesce_tx_usecs
            );
            println!("# ethtool -K {} tso {} gso {} gro {}", name, tso, gso, gro);
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
            if let (Some(rx), Some(tx)) = (ring_rx, ring_tx) {
                println!(
                    "# ethtool -G \"$IFACE\" rx {} tx {} 2>/dev/null || true",
                    rx, tx
                );
            }
            println!("# The remaining settings are candidates; uncomment only after path/workload validation.");
            println!("# ip link set dev \"$IFACE\" txqueuelen {}", s.txqueuelen);
            println!("# ip link set dev \"$IFACE\" mtu {}", s.mtu);
            println!(
                "# ethtool -C \"$IFACE\" rx-usecs {} tx-usecs {} 2>/dev/null || true",
                s.coalesce_rx_usecs, s.coalesce_tx_usecs
            );
            println!(
                "# ethtool -K \"$IFACE\" tso {} gso {} gro {} 2>/dev/null || true",
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
    fn conntrack_capacity_matches_kube_proxy_defaults() {
        assert_eq!(conntrack_max_for_cores(1), 131_072);
        assert_eq!(conntrack_max_for_cores(4), 131_072);
        assert_eq!(conntrack_max_for_cores(32), 1_048_576);
    }

    #[test]
    fn ring_targets_are_clamped_to_device_capabilities() {
        assert_eq!(safe_ring_target(4096, Some(2048), Some(512)), Some(2048));
        assert_eq!(safe_ring_target(4096, None, Some(512)), Some(512));
        assert_eq!(safe_ring_target(4096, None, None), None);
    }

    #[test]
    fn gateway_uses_conservative_device_candidates() {
        let values = SuggestedValues::for_profile(TuningProfile::Gateway);
        assert_eq!(values.txqueuelen, 2_000);
        assert_eq!(values.mtu, 1_500);
        assert_eq!(values.conntrack_max, suggested_conntrack_max());
    }

    #[test]
    fn investigation_candidates_are_marked_with_question_suffix() {
        assert_eq!(investigation_value("2"), "2 (?)");
        assert_eq!(investigation_value("kernel default"), "kernel default (?)");
    }

    #[test]
    fn generated_comments_include_profile_investigation_candidates() {
        let mut sysctl = eth::NetworkSysctl::default();
        sysctl.rp_filter.interfaces = vec![("eth0".into(), 1), ("cali123".into(), 0)];

        let mut physical = eth::EthInterface::default();
        physical.name = "eth0".into();
        physical.interface_type = eth::InterfaceType::Physical;
        let mut virtual_interface = eth::EthInterface::default();
        virtual_interface.name = "cali123".into();
        let interfaces = vec![physical, virtual_interface];

        let gateway = SuggestedValues::for_profile(TuningProfile::Gateway);
        let gateway_settings =
            investigation_settings(TuningProfile::Gateway, &gateway, &sysctl, &interfaces);
        assert!(!gateway_settings
            .iter()
            .any(|(key, _)| key == "net.ipv4.conf.all.rp_filter"));
        assert!(!gateway_settings
            .iter()
            .any(|(key, _)| key == "net.ipv4.conf.default.rp_filter"));
        assert!(gateway_settings
            .iter()
            .any(|(key, _)| key == "net.ipv4.conf.eth0.rp_filter"));
        assert!(!gateway_settings
            .iter()
            .any(|(key, _)| key == "net.ipv4.conf.cali123.rp_filter"));
        assert!(gateway_settings
            .iter()
            .any(|(key, _)| key == "net.core.rmem_max"));
        assert!(!gateway_settings
            .iter()
            .any(|(key, _)| key == "net.ipv4.tcp_tw_reuse"));
        assert!(!gateway_settings
            .iter()
            .any(|(key, _)| key.contains("gc_thresh")));
        assert!(!gateway_settings
            .iter()
            .any(|(key, _)| key == "net.ipv4.ip_forward"));

        let worker = SuggestedValues::for_profile(TuningProfile::Worker);
        let worker_settings =
            investigation_settings(TuningProfile::Worker, &worker, &sysctl, &interfaces);
        assert!(worker_settings
            .iter()
            .any(|(key, value)| key == "net.ipv4.ip_forward" && value == "1"));

        assert_eq!(
            investigation_output_line(OutputFormat::Cmd, "net.ipv4.ip_forward", "1"),
            "# sysctl -w 'net.ipv4.ip_forward=1'"
        );
        assert_eq!(
            investigation_output_line(OutputFormat::Conf, "net.ipv4.ip_forward", "1"),
            "# net.ipv4.ip_forward = 1"
        );
    }

    #[test]
    fn invalid_profile_and_format_are_rejected() {
        assert!(TuningProfile::from_str("typo").is_err());
        assert!(TuningProfile::from_str("gateway").unwrap().is_gateway());
        assert!(TuningProfile::from_str("router").unwrap().is_gateway());
        assert!(OutputFormat::from_str("yaml").is_err());
    }

    #[test]
    fn count_lists_use_compact_values_and_single_space_separators() {
        assert_eq!(
            format_count_list("1520958\t2027945  3041916", Some(CountFormat::Decimal)),
            "1M 2M 3M"
        );
        assert_eq!(
            format_count_list("4096\t131072  16777216", Some(CountFormat::Bytes)),
            "4Ki 128Ki 16Mi"
        );
        assert_eq!(format_count_list("32768\t  60999", None), "32768 60999");
    }

    #[test]
    fn ephemeral_capacity_excludes_merged_reserved_ranges() {
        assert_eq!(
            usable_ephemeral_ports(
                Some("32768 60999"),
                Some("30000-32767,40000-40009,40005-40020")
            ),
            Some(28_211)
        );
        assert_eq!(
            usable_ephemeral_ports(Some("32768 60999"), Some("")),
            Some(28_232)
        );
    }
}
