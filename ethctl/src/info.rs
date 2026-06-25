use comfy_table::{presets::NOTHING, Cell, Color, Table};
use libonm::eth::{self, EthError};

#[derive(Clone, Copy)]
pub enum TuningProfile {
    ControlPlane,
    Worker,
}

impl TuningProfile {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "control-plane" | "controlplane" | "cp" | "master" => TuningProfile::ControlPlane,
            _ => TuningProfile::Worker,
        }
    }

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

impl OutputFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "conf" | "sysctl.conf" | "file" => OutputFormat::Conf,
            "script" | "sh" | "bash" => OutputFormat::Script,
            _ => OutputFormat::Cmd,
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

pub fn run(name: &str, profile_str: &str) -> Result<(), EthError> {
    let iface = eth::get_interface(name)?;
    let profile = TuningProfile::from_str(profile_str);
    let s = SuggestedValues::for_profile(profile);
    let sysctl = eth::get_network_sysctl();

    let mut table = Table::new();
    table.load_preset(NOTHING);

    add_first_section(&mut table, "Overview");
    table.add_row(vec!["  Profile", profile.name(), "-"]);
    table.add_row(vec!["  Name", &iface.name, "-"]);
    table.add_row(vec!["  MAC Address", &iface.mac_address, "-"]);
    table.add_row(vec![
        "  MTU",
        &iface.mtu.to_string(),
        &format!("{} (jumbo)", s.mtu),
    ]);
    table.add_row(vec![
        "  TX Queue Length",
        &iface.txqueuelen.to_string(),
        &s.txqueuelen.to_string(),
    ]);
    table.add_row(vec!["  State", &iface.state.to_string(), "-"]);
    table.add_row(vec![
        "  Carrier",
        &iface
            .carrier
            .map(|c| if c { "yes" } else { "no" }.to_string())
            .unwrap_or("-".to_string()),
        "-",
    ]);
    table.add_row(vec![
        "  Speed",
        &iface
            .speed
            .map(|sp| format!("{} Mbps", sp))
            .unwrap_or("-".to_string()),
        "-",
    ]);
    table.add_row(vec![
        "  Duplex",
        &iface.duplex.clone().unwrap_or("-".to_string()),
        "-",
    ]);
    table.add_row(vec![
        "  NUMA Node",
        &iface
            .numa_node
            .map(|n| n.to_string())
            .unwrap_or("-".to_string()),
        "-",
    ]);
    table.add_row(vec![
        "  Driver",
        &iface.driver.clone().unwrap_or("-".to_string()),
        "-",
    ]);
    table.add_row(vec![
        "  PCI Slot",
        &iface.pci_slot.clone().unwrap_or("-".to_string()),
        "-",
    ]);

    add_sysctl_rows(&mut table, &sysctl, &s);

    println!("{table}");

    Ok(())
}

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
    add_section(table, "Connection Tracking");
    add_row(
        table,
        "  nf_conntrack_max",
        sysctl.conntrack.max,
        s.conntrack_max,
    );
    add_row(
        table,
        "  nf_conntrack_buckets",
        sysctl.conntrack.buckets,
        s.conntrack_buckets,
    );
    add_row(
        table,
        "  nf_conntrack_tcp_timeout_established",
        sysctl.conntrack.tcp_timeout_established,
        s.conntrack_tcp_timeout_established,
    );
    add_row(
        table,
        "  nf_conntrack_tcp_timeout_time_wait",
        sysctl.conntrack.tcp_timeout_time_wait,
        s.conntrack_tcp_timeout_time_wait,
    );
    add_row(
        table,
        "  nf_conntrack_tcp_timeout_close_wait",
        sysctl.conntrack.tcp_timeout_close_wait,
        s.conntrack_tcp_timeout_close_wait,
    );
    add_row(
        table,
        "  nf_conntrack_tcp_timeout_fin_wait",
        sysctl.conntrack.tcp_timeout_fin_wait,
        s.conntrack_tcp_timeout_fin_wait,
    );
    add_row(
        table,
        "  nf_conntrack_tcp_max_retrans",
        sysctl.conntrack.tcp_max_retrans,
        s.conntrack_tcp_max_retrans,
    );

    add_section(table, "Socket Buffers");
    add_row_bytes(
        table,
        "  net.core.rmem_max",
        sysctl.socket_buffer.rmem_max,
        s.rmem_max,
    );
    add_row_bytes(
        table,
        "  net.core.wmem_max",
        sysctl.socket_buffer.wmem_max,
        s.wmem_max,
    );
    add_row_bytes(
        table,
        "  net.core.rmem_default",
        sysctl.socket_buffer.rmem_default,
        s.rmem_default,
    );
    add_row_bytes(
        table,
        "  net.core.wmem_default",
        sysctl.socket_buffer.wmem_default,
        s.wmem_default,
    );
    add_row_tcp_mem(
        table,
        "  net.ipv4.tcp_rmem",
        sysctl.socket_buffer.tcp_rmem.clone(),
        s.tcp_rmem,
    );
    add_row_tcp_mem(
        table,
        "  net.ipv4.tcp_wmem",
        sysctl.socket_buffer.tcp_wmem.clone(),
        s.tcp_wmem,
    );
    add_row(
        table,
        "  net.core.netdev_max_backlog",
        sysctl.socket_buffer.netdev_max_backlog,
        s.netdev_max_backlog,
    );
    add_row_bytes(
        table,
        "  net.ipv4.udp_rmem_min",
        sysctl.udp.rmem_min,
        s.udp_rmem_min,
    );
    add_row_bytes(
        table,
        "  net.ipv4.udp_wmem_min",
        sysctl.udp.wmem_min,
        s.udp_wmem_min,
    );
    add_row_tcp_mem(
        table,
        "  net.ipv4.udp_mem",
        sysctl.udp.udp_mem.clone(),
        s.udp_mem,
    );

    add_section(table, "TCP Settings");
    add_row(
        table,
        "  net.core.somaxconn",
        sysctl.tcp.somaxconn,
        s.somaxconn,
    );
    add_row(
        table,
        "  net.ipv4.tcp_max_syn_backlog",
        sysctl.tcp.max_syn_backlog,
        s.tcp_max_syn_backlog,
    );
    add_row(
        table,
        "  net.ipv4.tcp_tw_reuse",
        sysctl.tcp.tw_reuse,
        s.tcp_tw_reuse,
    );
    add_row(
        table,
        "  net.ipv4.tcp_fin_timeout",
        sysctl.tcp.fin_timeout,
        s.tcp_fin_timeout,
    );
    add_row(
        table,
        "  net.ipv4.tcp_keepalive_time",
        sysctl.tcp.keepalive_time,
        s.tcp_keepalive_time,
    );
    add_row(
        table,
        "  net.ipv4.tcp_keepalive_probes",
        sysctl.tcp.keepalive_probes,
        s.tcp_keepalive_probes,
    );
    add_row(
        table,
        "  net.ipv4.tcp_keepalive_intvl",
        sysctl.tcp.keepalive_intvl,
        s.tcp_keepalive_intvl,
    );
    add_row_str(
        table,
        "  net.ipv4.ip_local_port_range",
        sysctl.tcp.ip_local_port_range.clone(),
        s.ip_local_port_range,
    );

    add_section(table, "ARP / Neighbor Table");
    add_row(
        table,
        "  net.ipv4.neigh.default.gc_thresh1",
        sysctl.arp.gc_thresh1,
        s.arp_gc_thresh1,
    );
    add_row(
        table,
        "  net.ipv4.neigh.default.gc_thresh2",
        sysctl.arp.gc_thresh2,
        s.arp_gc_thresh2,
    );
    add_row(
        table,
        "  net.ipv4.neigh.default.gc_thresh3",
        sysctl.arp.gc_thresh3,
        s.arp_gc_thresh3,
    );
    add_row(
        table,
        "  net.ipv4.conf.all.arp_ignore",
        sysctl.arp.arp_ignore,
        s.arp_ignore,
    );
    add_row(
        table,
        "  net.ipv4.conf.all.arp_announce",
        sysctl.arp.arp_announce,
        s.arp_announce,
    );

    add_section(table, "Reverse Path Filtering");
    add_row(
        table,
        "  net.ipv4.conf.all.rp_filter",
        sysctl.rp_filter.all,
        s.rp_filter,
    );
    add_row(
        table,
        "  net.ipv4.conf.default.rp_filter",
        sysctl.rp_filter.default,
        s.rp_filter,
    );
}

pub async fn print_link_tables(name: &str, profile: TuningProfile) {
    use libonm::eth;

    let s = SuggestedValues::for_profile(profile);

    let link = eth::get_link_settings(name).await.ok();
    let ethtool = eth::get_ethtool_settings(name).await.ok();

    let mut table = Table::new();
    table.load_preset(NOTHING);

    add_first_section(&mut table, "Overview");
    table.add_row(vec!["  Profile", profile.name(), "-"]);

    add_section(&mut table, "IP Link Settings");
    if let Some(ref l) = link {
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
        add_row_str(&mut table, "  qdisc", l.qdisc.clone(), "-");
        add_row_u32(&mut table, "  group", l.group, 0);
    } else {
        table.add_row(vec!["  (rtnetlink unavailable)", "-", "-"]);
    }

    add_section(&mut table, "Ethtool Settings");
    if let Some(ref e) = ethtool {
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
    } else {
        table.add_row(vec!["  (ethtool unavailable)", "-", "-"]);
    }

    println!("{table}");
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

fn add_row(table: &mut Table, name: &str, value: Option<u64>, suggested: u64) {
    table.add_row(vec![
        name.to_string(),
        value.map(|v| v.to_string()).unwrap_or("-".to_string()),
        suggested.to_string(),
    ]);
}

fn add_row_bytes(table: &mut Table, name: &str, value: Option<u64>, suggested: u64) {
    table.add_row(vec![
        name.to_string(),
        value.map(|v| format_size(v)).unwrap_or("-".to_string()),
        format_size(suggested),
    ]);
}

fn add_row_str(table: &mut Table, name: &str, value: Option<String>, suggested: &str) {
    table.add_row(vec![
        name.to_string(),
        value.unwrap_or("-".to_string()),
        suggested.to_string(),
    ]);
}

fn add_row_tcp_mem(table: &mut Table, name: &str, value: Option<String>, suggested: &str) {
    table.add_row(vec![
        name.to_string(),
        value.map(|v| format_tcp_mem(&v)).unwrap_or("-".to_string()),
        format_tcp_mem(suggested),
    ]);
}

pub fn generate_sysctl_output(profile: TuningProfile, format: OutputFormat) {
    let s = SuggestedValues::for_profile(profile);

    let settings: Vec<(&str, String)> = vec![
        (
            "net.netfilter.nf_conntrack_max",
            s.conntrack_max.to_string(),
        ),
        (
            "net.netfilter.nf_conntrack_buckets",
            s.conntrack_buckets.to_string(),
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_established",
            s.conntrack_tcp_timeout_established.to_string(),
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_time_wait",
            s.conntrack_tcp_timeout_time_wait.to_string(),
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_close_wait",
            s.conntrack_tcp_timeout_close_wait.to_string(),
        ),
        (
            "net.netfilter.nf_conntrack_tcp_timeout_fin_wait",
            s.conntrack_tcp_timeout_fin_wait.to_string(),
        ),
        (
            "net.netfilter.nf_conntrack_tcp_max_retrans",
            s.conntrack_tcp_max_retrans.to_string(),
        ),
        ("net.core.rmem_max", s.rmem_max.to_string()),
        ("net.core.wmem_max", s.wmem_max.to_string()),
        ("net.core.rmem_default", s.rmem_default.to_string()),
        ("net.core.wmem_default", s.wmem_default.to_string()),
        ("net.ipv4.tcp_rmem", s.tcp_rmem.to_string()),
        ("net.ipv4.tcp_wmem", s.tcp_wmem.to_string()),
        (
            "net.core.netdev_max_backlog",
            s.netdev_max_backlog.to_string(),
        ),
        ("net.core.somaxconn", s.somaxconn.to_string()),
        (
            "net.ipv4.tcp_max_syn_backlog",
            s.tcp_max_syn_backlog.to_string(),
        ),
        ("net.ipv4.tcp_tw_reuse", s.tcp_tw_reuse.to_string()),
        ("net.ipv4.tcp_fin_timeout", s.tcp_fin_timeout.to_string()),
        (
            "net.ipv4.tcp_keepalive_time",
            s.tcp_keepalive_time.to_string(),
        ),
        (
            "net.ipv4.tcp_keepalive_probes",
            s.tcp_keepalive_probes.to_string(),
        ),
        (
            "net.ipv4.tcp_keepalive_intvl",
            s.tcp_keepalive_intvl.to_string(),
        ),
        (
            "net.ipv4.ip_local_port_range",
            s.ip_local_port_range.to_string(),
        ),
        ("net.ipv4.udp_rmem_min", s.udp_rmem_min.to_string()),
        ("net.ipv4.udp_wmem_min", s.udp_wmem_min.to_string()),
        ("net.ipv4.udp_mem", s.udp_mem.to_string()),
        (
            "net.ipv4.neigh.default.gc_thresh1",
            s.arp_gc_thresh1.to_string(),
        ),
        (
            "net.ipv4.neigh.default.gc_thresh2",
            s.arp_gc_thresh2.to_string(),
        ),
        (
            "net.ipv4.neigh.default.gc_thresh3",
            s.arp_gc_thresh3.to_string(),
        ),
        ("net.ipv4.conf.all.arp_ignore", s.arp_ignore.to_string()),
        ("net.ipv4.conf.all.arp_announce", s.arp_announce.to_string()),
        ("net.ipv4.conf.all.rp_filter", s.rp_filter.to_string()),
        ("net.ipv4.conf.default.rp_filter", s.rp_filter.to_string()),
    ];

    let tso = if s.offload_tso { "on" } else { "off" };
    let gso = if s.offload_gso { "on" } else { "off" };
    let gro = if s.offload_gro { "on" } else { "off" };

    match format {
        OutputFormat::Cmd => {
            for (key, value) in &settings {
                println!("sysctl -w {}={}", key, value);
            }
            println!();
            println!("# Interface tuning (ip link)");
            println!("for iface in $(ls /sys/class/net | grep -E '^(eth|ens|eno|enp)'); do");
            println!("    ip link set dev \"$iface\" txqueuelen {}", s.txqueuelen);
            println!("    ip link set dev \"$iface\" mtu {}", s.mtu);
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
            for (key, value) in &settings {
                println!("{} = {}", key, value);
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
            for (key, value) in &settings {
                println!("sysctl -w {}={}", key, value);
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

pub async fn generate_link_output(name: &str, profile: TuningProfile, format: OutputFormat) {
    use libonm::eth;

    let s = SuggestedValues::for_profile(profile);
    let link = eth::get_link_settings(name).await.ok();

    let tso = if s.offload_tso { "on" } else { "off" };
    let gso = if s.offload_gso { "on" } else { "off" };
    let gro = if s.offload_gro { "on" } else { "off" };

    match format {
        OutputFormat::Cmd => {
            println!("ip link set dev {} txqueuelen {}", name, s.txqueuelen);
            println!("ip link set dev {} mtu {}", name, s.mtu);
            println!();
            println!("ethtool -G {} rx {} tx {}", name, s.ring_rx, s.ring_tx);
            println!(
                "ethtool -C {} rx-usecs {} tx-usecs {}",
                name, s.coalesce_rx_usecs, s.coalesce_tx_usecs
            );
            println!("ethtool -K {} tso {} gso {} gro {}", name, tso, gso, gro);

            if let Some(ref l) = link {
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
}
