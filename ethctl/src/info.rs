use comfy_table::{presets::UTF8_FULL, Table};
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
    pub udp_rmem_min: u64,
    pub udp_wmem_min: u64,
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
            udp_rmem_min: 16_384,
            udp_wmem_min: 16_384,

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
            udp_rmem_min: 16_384,
            udp_wmem_min: 16_384,

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
        }
    }
}

pub fn run(name: &str, profile_str: &str) -> Result<(), EthError> {
    let iface = eth::get_interface(name)?;
    let profile = TuningProfile::from_str(profile_str);

    let mut iface_table = Table::new();
    iface_table.load_preset(UTF8_FULL);
    iface_table.set_header(vec!["Interface Property", "Value"]);
    iface_table.add_row(vec!["Name", &iface.name]);
    iface_table.add_row(vec!["MAC Address", &iface.mac_address]);
    iface_table.add_row(vec!["MTU", &iface.mtu.to_string()]);
    iface_table.add_row(vec!["State", &iface.state.to_string()]);
    iface_table.add_row(vec![
        "Speed",
        &iface
            .speed
            .map(|s| format!("{} Mbps", s))
            .unwrap_or("-".to_string()),
    ]);
    iface_table.add_row(vec![
        "Driver",
        &iface.driver.clone().unwrap_or("-".to_string()),
    ]);
    iface_table.add_row(vec![
        "PCI Slot",
        &iface.pci_slot.clone().unwrap_or("-".to_string()),
    ]);

    println!("{iface_table}");
    println!();

    print_sysctl_tables(profile);

    Ok(())
}

pub fn print_sysctl_tables(profile: TuningProfile) {
    use libonm::eth;

    let sysctl = eth::get_network_sysctl();
    let s = SuggestedValues::for_profile(profile);
    let header = profile.header_suffix();

    let mut conntrack = Table::new();
    conntrack.load_preset(UTF8_FULL);
    conntrack.set_header(vec!["Connection Tracking", "Value", header]);
    add_row(
        &mut conntrack,
        "nf_conntrack_max",
        sysctl.conntrack.max,
        s.conntrack_max,
    );
    add_row(
        &mut conntrack,
        "nf_conntrack_buckets",
        sysctl.conntrack.buckets,
        s.conntrack_buckets,
    );
    add_row(
        &mut conntrack,
        "nf_conntrack_tcp_timeout_established",
        sysctl.conntrack.tcp_timeout_established,
        s.conntrack_tcp_timeout_established,
    );
    add_row(
        &mut conntrack,
        "nf_conntrack_tcp_timeout_time_wait",
        sysctl.conntrack.tcp_timeout_time_wait,
        s.conntrack_tcp_timeout_time_wait,
    );
    add_row(
        &mut conntrack,
        "nf_conntrack_tcp_timeout_close_wait",
        sysctl.conntrack.tcp_timeout_close_wait,
        s.conntrack_tcp_timeout_close_wait,
    );
    add_row(
        &mut conntrack,
        "nf_conntrack_tcp_timeout_fin_wait",
        sysctl.conntrack.tcp_timeout_fin_wait,
        s.conntrack_tcp_timeout_fin_wait,
    );
    add_row(
        &mut conntrack,
        "nf_conntrack_tcp_max_retrans",
        sysctl.conntrack.tcp_max_retrans,
        s.conntrack_tcp_max_retrans,
    );
    println!("{conntrack}");
    println!();

    let mut socket = Table::new();
    socket.load_preset(UTF8_FULL);
    socket.set_header(vec!["Socket Buffers", "Value", header]);
    add_row(
        &mut socket,
        "net.core.rmem_max",
        sysctl.socket_buffer.rmem_max,
        s.rmem_max,
    );
    add_row(
        &mut socket,
        "net.core.wmem_max",
        sysctl.socket_buffer.wmem_max,
        s.wmem_max,
    );
    add_row(
        &mut socket,
        "net.core.rmem_default",
        sysctl.socket_buffer.rmem_default,
        s.rmem_default,
    );
    add_row(
        &mut socket,
        "net.core.wmem_default",
        sysctl.socket_buffer.wmem_default,
        s.wmem_default,
    );
    add_row_str(
        &mut socket,
        "net.ipv4.tcp_rmem",
        sysctl.socket_buffer.tcp_rmem,
        s.tcp_rmem,
    );
    add_row_str(
        &mut socket,
        "net.ipv4.tcp_wmem",
        sysctl.socket_buffer.tcp_wmem,
        s.tcp_wmem,
    );
    add_row(
        &mut socket,
        "net.ipv4.udp_rmem_min",
        sysctl.socket_buffer.udp_rmem_min,
        s.udp_rmem_min,
    );
    add_row(
        &mut socket,
        "net.ipv4.udp_wmem_min",
        sysctl.socket_buffer.udp_wmem_min,
        s.udp_wmem_min,
    );
    println!("{socket}");
    println!();

    let mut tcp = Table::new();
    tcp.load_preset(UTF8_FULL);
    tcp.set_header(vec!["TCP Settings", "Value", header]);
    add_row(
        &mut tcp,
        "net.core.somaxconn",
        sysctl.tcp.somaxconn,
        s.somaxconn,
    );
    add_row(
        &mut tcp,
        "net.ipv4.tcp_max_syn_backlog",
        sysctl.tcp.max_syn_backlog,
        s.tcp_max_syn_backlog,
    );
    add_row(
        &mut tcp,
        "net.ipv4.tcp_tw_reuse",
        sysctl.tcp.tw_reuse,
        s.tcp_tw_reuse,
    );
    add_row(
        &mut tcp,
        "net.ipv4.tcp_fin_timeout",
        sysctl.tcp.fin_timeout,
        s.tcp_fin_timeout,
    );
    add_row(
        &mut tcp,
        "net.ipv4.tcp_keepalive_time",
        sysctl.tcp.keepalive_time,
        s.tcp_keepalive_time,
    );
    add_row(
        &mut tcp,
        "net.ipv4.tcp_keepalive_probes",
        sysctl.tcp.keepalive_probes,
        s.tcp_keepalive_probes,
    );
    add_row(
        &mut tcp,
        "net.ipv4.tcp_keepalive_intvl",
        sysctl.tcp.keepalive_intvl,
        s.tcp_keepalive_intvl,
    );
    add_row_str(
        &mut tcp,
        "net.ipv4.ip_local_port_range",
        sysctl.tcp.ip_local_port_range,
        s.ip_local_port_range,
    );
    println!("{tcp}");
    println!();

    let mut arp = Table::new();
    arp.load_preset(UTF8_FULL);
    arp.set_header(vec!["ARP / Neighbor Table", "Value", header]);
    add_row(
        &mut arp,
        "net.ipv4.neigh.default.gc_thresh1",
        sysctl.arp.gc_thresh1,
        s.arp_gc_thresh1,
    );
    add_row(
        &mut arp,
        "net.ipv4.neigh.default.gc_thresh2",
        sysctl.arp.gc_thresh2,
        s.arp_gc_thresh2,
    );
    add_row(
        &mut arp,
        "net.ipv4.neigh.default.gc_thresh3",
        sysctl.arp.gc_thresh3,
        s.arp_gc_thresh3,
    );
    add_row(
        &mut arp,
        "net.ipv4.conf.all.arp_ignore",
        sysctl.arp.arp_ignore,
        s.arp_ignore,
    );
    add_row(
        &mut arp,
        "net.ipv4.conf.all.arp_announce",
        sysctl.arp.arp_announce,
        s.arp_announce,
    );
    println!("{arp}");
    println!();

    let mut rp = Table::new();
    rp.load_preset(UTF8_FULL);
    rp.set_header(vec!["Reverse Path Filtering", "Value", header]);
    add_row(
        &mut rp,
        "net.ipv4.conf.all.rp_filter",
        sysctl.rp_filter.all,
        s.rp_filter,
    );
    add_row(
        &mut rp,
        "net.ipv4.conf.default.rp_filter",
        sysctl.rp_filter.default,
        s.rp_filter,
    );
    println!("{rp}");
}

fn add_row(table: &mut Table, name: &str, value: Option<u64>, suggested: u64) {
    table.add_row(vec![
        name.to_string(),
        value.map(|v| v.to_string()).unwrap_or("-".to_string()),
        suggested.to_string(),
    ]);
}

fn add_row_str(table: &mut Table, name: &str, value: Option<String>, suggested: &str) {
    table.add_row(vec![
        name.to_string(),
        value.unwrap_or("-".to_string()),
        suggested.to_string(),
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
        ("net.ipv4.udp_rmem_min", s.udp_rmem_min.to_string()),
        ("net.ipv4.udp_wmem_min", s.udp_wmem_min.to_string()),
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

    match format {
        OutputFormat::Cmd => {
            for (key, value) in &settings {
                println!("sysctl -w {}={}", key, value);
            }
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
        }
        OutputFormat::Script => {
            println!("#!/bin/bash");
            println!(
                "# Sysctl tuning for 10k-node cluster ({} profile)",
                profile.name()
            );
            println!("# Run with: sudo bash <script>");
            println!();
            println!("set -e");
            println!();
            for (key, value) in &settings {
                println!("sysctl -w {}={}", key, value);
            }
            println!();
            println!("echo 'Sysctl settings applied successfully'");
        }
    }
}
