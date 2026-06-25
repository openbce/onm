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
