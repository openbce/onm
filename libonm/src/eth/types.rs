use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EthError {
    #[error("{0}")]
    Internal(String),
    #[error("'{0}' not found")]
    NotFound(String),
    #[error("invalid configuration '{0}'")]
    InvalidConfig(String),
    #[error("IO error: {0}")]
    IoError(String),
}

impl From<std::io::Error> for EthError {
    fn from(e: std::io::Error) -> Self {
        EthError::IoError(e.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LinkState {
    Up,
    Down,
    Unknown,
}

impl ToString for LinkState {
    fn to_string(&self) -> String {
        match self {
            LinkState::Up => "Up".to_string(),
            LinkState::Down => "Down".to_string(),
            LinkState::Unknown => "Unknown".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InterfaceType {
    Physical,
    Virtual,
}

impl ToString for InterfaceType {
    fn to_string(&self) -> String {
        match self {
            InterfaceType::Physical => "physical".to_string(),
            InterfaceType::Virtual => "virtual".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EthInterface {
    pub name: String,
    pub mac_address: String,
    pub mtu: u32,
    pub txqueuelen: u32,
    pub state: LinkState,
    pub speed: Option<u64>,
    pub duplex: Option<String>,
    pub carrier: Option<bool>,
    pub numa_node: Option<i32>,
    pub driver: Option<String>,
    pub pci_slot: Option<String>,
    pub interface_type: InterfaceType,
    pub addresses: Vec<String>,
    pub master: Option<String>,
    pub kind: Option<String>,
}

impl Default for EthInterface {
    fn default() -> Self {
        EthInterface {
            name: String::new(),
            mac_address: String::new(),
            mtu: 1500,
            txqueuelen: 1000,
            state: LinkState::Unknown,
            speed: None,
            duplex: None,
            carrier: None,
            numa_node: None,
            driver: None,
            pci_slot: None,
            interface_type: InterfaceType::Virtual,
            addresses: Vec::new(),
            master: None,
            kind: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConntrackSettings {
    pub max: Option<u64>,
    pub buckets: Option<u64>,
    pub tcp_timeout_established: Option<u64>,
    pub tcp_timeout_time_wait: Option<u64>,
    pub tcp_timeout_close_wait: Option<u64>,
    pub tcp_timeout_fin_wait: Option<u64>,
    pub tcp_max_retrans: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SocketBufferSettings {
    pub rmem_max: Option<u64>,
    pub wmem_max: Option<u64>,
    pub rmem_default: Option<u64>,
    pub wmem_default: Option<u64>,
    pub tcp_rmem: Option<String>,
    pub tcp_wmem: Option<String>,
    pub netdev_max_backlog: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TcpSettings {
    pub somaxconn: Option<u64>,
    pub max_syn_backlog: Option<u64>,
    pub tw_reuse: Option<u64>,
    pub fin_timeout: Option<u64>,
    pub keepalive_time: Option<u64>,
    pub keepalive_probes: Option<u64>,
    pub keepalive_intvl: Option<u64>,
    pub ip_local_port_range: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UdpSettings {
    pub rmem_min: Option<u64>,
    pub wmem_min: Option<u64>,
    pub udp_mem: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ArpSettings {
    pub gc_thresh1: Option<u64>,
    pub gc_thresh2: Option<u64>,
    pub gc_thresh3: Option<u64>,
    pub arp_ignore: Option<u64>,
    pub arp_announce: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RpFilterSettings {
    pub all: Option<u64>,
    pub default: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NetworkSysctl {
    pub conntrack: ConntrackSettings,
    pub socket_buffer: SocketBufferSettings,
    pub tcp: TcpSettings,
    pub udp: UdpSettings,
    pub arp: ArpSettings,
    pub rp_filter: RpFilterSettings,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConntrackStats {
    pub current: Option<u64>,
    pub max: Option<u64>,
    pub usage_percent: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SoftnetCpuStats {
    pub cpu: u32,
    pub processed: u64,
    pub dropped: u64,
    pub time_squeeze: u64,
    pub cpu_collision: u64,
    pub received_rps: u64,
    pub flow_limit_count: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SoftnetStats {
    pub cpus: Vec<SoftnetCpuStats>,
    pub total_processed: u64,
    pub total_dropped: u64,
    pub total_time_squeeze: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SocketStats {
    pub tcp_inuse: Option<u64>,
    pub tcp_orphan: Option<u64>,
    pub tcp_tw: Option<u64>,
    pub tcp_alloc: Option<u64>,
    pub tcp_mem: Option<u64>,
    pub udp_inuse: Option<u64>,
    pub udp_mem: Option<u64>,
    pub raw_inuse: Option<u64>,
    pub frag_inuse: Option<u64>,
    pub frag_memory: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InterfaceStats {
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errors: u64,
    pub rx_dropped: u64,
    pub rx_fifo: u64,
    pub rx_frame: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errors: u64,
    pub tx_dropped: u64,
    pub tx_fifo: u64,
    pub tx_carrier: u64,
    pub tx_collisions: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NetworkStats {
    pub conntrack: ConntrackStats,
    pub softnet: SoftnetStats,
    pub sockets: SocketStats,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EthtoolRing {
    pub rx: Option<u32>,
    pub rx_max: Option<u32>,
    pub tx: Option<u32>,
    pub tx_max: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EthtoolCoalesce {
    pub rx_usecs: Option<u32>,
    pub tx_usecs: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EthtoolOffload {
    pub tso: Option<bool>,
    pub gso: Option<bool>,
    pub gro: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EthtoolSettings {
    pub ring: EthtoolRing,
    pub coalesce: EthtoolCoalesce,
    pub offload: EthtoolOffload,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LinkSettings {
    pub mtu: Option<u32>,
    pub min_mtu: Option<u32>,
    pub max_mtu: Option<u32>,
    pub txqueuelen: Option<u32>,
    pub num_tx_queues: Option<u32>,
    pub num_rx_queues: Option<u32>,
    pub gso_max_size: Option<u32>,
    pub gso_max_segs: Option<u32>,
    pub gro_max_size: Option<u32>,
    pub tso_max_size: Option<u32>,
    pub tso_max_segs: Option<u32>,
    pub qdisc: Option<String>,
    pub group: Option<u32>,
}

// Route types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RouteScope {
    Universe,
    Site,
    Link,
    Host,
    Nowhere,
    Unknown(u8),
}

impl Default for RouteScope {
    fn default() -> Self {
        RouteScope::Universe
    }
}

impl ToString for RouteScope {
    fn to_string(&self) -> String {
        match self {
            RouteScope::Universe => "universe".to_string(),
            RouteScope::Site => "site".to_string(),
            RouteScope::Link => "link".to_string(),
            RouteScope::Host => "host".to_string(),
            RouteScope::Nowhere => "nowhere".to_string(),
            RouteScope::Unknown(v) => format!("unknown({})", v),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RouteType {
    Unicast,
    Local,
    Broadcast,
    Anycast,
    Multicast,
    Blackhole,
    Unreachable,
    Prohibit,
    Throw,
    Nat,
    Unknown(u8),
}

impl Default for RouteType {
    fn default() -> Self {
        RouteType::Unicast
    }
}

impl ToString for RouteType {
    fn to_string(&self) -> String {
        match self {
            RouteType::Unicast => "unicast".to_string(),
            RouteType::Local => "local".to_string(),
            RouteType::Broadcast => "broadcast".to_string(),
            RouteType::Anycast => "anycast".to_string(),
            RouteType::Multicast => "multicast".to_string(),
            RouteType::Blackhole => "blackhole".to_string(),
            RouteType::Unreachable => "unreachable".to_string(),
            RouteType::Prohibit => "prohibit".to_string(),
            RouteType::Throw => "throw".to_string(),
            RouteType::Nat => "nat".to_string(),
            RouteType::Unknown(v) => format!("unknown({})", v),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RouteProtocol {
    Unspec,
    Kernel,
    Boot,
    Static,
    Dhcp,
    Ra,
    Unknown(u8),
}

impl Default for RouteProtocol {
    fn default() -> Self {
        RouteProtocol::Unspec
    }
}

impl ToString for RouteProtocol {
    fn to_string(&self) -> String {
        match self {
            RouteProtocol::Unspec => "unspec".to_string(),
            RouteProtocol::Kernel => "kernel".to_string(),
            RouteProtocol::Boot => "boot".to_string(),
            RouteProtocol::Static => "static".to_string(),
            RouteProtocol::Dhcp => "dhcp".to_string(),
            RouteProtocol::Ra => "ra".to_string(),
            RouteProtocol::Unknown(v) => format!("unknown({})", v),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RouteEntry {
    pub destination: String,
    pub gateway: Option<String>,
    pub interface: Option<String>,
    pub metric: Option<u32>,
    pub scope: RouteScope,
    pub route_type: RouteType,
    pub protocol: RouteProtocol,
    pub table: u8,
    pub prefix_len: u8,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RouteTable {
    pub ipv4: Vec<RouteEntry>,
    pub ipv6: Vec<RouteEntry>,
}

// NAT types (iptables/nftables)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NatType {
    Snat,
    Dnat,
    Masquerade,
}

impl ToString for NatType {
    fn to_string(&self) -> String {
        match self {
            NatType::Snat => "SNAT".to_string(),
            NatType::Dnat => "DNAT".to_string(),
            NatType::Masquerade => "MASQUERADE".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NatRule {
    pub chain: String,
    pub nat_type: NatType,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub protocol: Option<String>,
    pub dport: Option<String>,
    pub sport: Option<String>,
    pub to_source: Option<String>,
    pub to_destination: Option<String>,
    pub interface_in: Option<String>,
    pub interface_out: Option<String>,
    pub packets: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NatTable {
    pub rules: Vec<NatRule>,
}
