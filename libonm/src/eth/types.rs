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
    pub udp_rmem_min: Option<u64>,
    pub udp_wmem_min: Option<u64>,
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
    pub arp: ArpSettings,
    pub rp_filter: RpFilterSettings,
}
