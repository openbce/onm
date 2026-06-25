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
pub struct EthInterface {
    pub name: String,
    pub mac_address: String,
    pub mtu: u32,
    pub txqueuelen: u32,
    pub state: LinkState,
    pub speed: Option<u64>,
    pub driver: Option<String>,
    pub pci_slot: Option<String>,
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
            driver: None,
            pci_slot: None,
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
