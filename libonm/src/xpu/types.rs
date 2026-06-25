use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::redfish::{self, Redfish, RedfishError};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum XPUStatus {
    Ready,
    Error,
    Unknown,
}

#[derive(Error, Debug)]
pub enum XPUError {
    #[error("{0}")]
    Internal(String),
    #[error("'{0}' not found")]
    NotFound(String),
    #[error("invalid configuration '{0}'")]
    InvalidConfig(String),
}

impl From<RedfishError> for XPUError {
    fn from(value: RedfishError) -> Self {
        XPUError::Internal(value.to_string())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BMC {
    pub address: String,
    pub username: String,
    pub password: String,
    /// Whether to verify TLS certificates. Defaults to true for security.
    /// Set to false only for development/testing with self-signed certs.
    #[serde(default = "default_tls_verify")]
    pub tls_verify: bool,
}

fn default_tls_verify() -> bool {
    true
}

impl std::fmt::Debug for BMC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BMC")
            .field("address", &self.address)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("tls_verify", &self.tls_verify)
            .finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BMCVersion {
    pub description: String,
    pub id: String,
    pub version: String,
}

pub struct XPU {
    #[allow(dead_code)]
    redfish: Box<dyn Redfish>,

    pub bmc: BMC,
    pub vendor: String,
    pub serial_number: String,
    pub firmware_version: String,
    pub bmc_version: String,

    pub status: XPUStatus,
}

impl ToString for XPUStatus {
    fn to_string(&self) -> String {
        match self {
            XPUStatus::Error => "Error".to_string(),
            XPUStatus::Ready => "Ready".to_string(),
            XPUStatus::Unknown => "Unknown".to_string(),
        }
    }
}

impl XPU {
    pub async fn new(bmc: &BMC) -> Result<Self, XPUError> {
        let redfish = redfish::build(bmc)?;

        // Run discover flow to handle default password scenarios
        redfish.discover().await?;

        let bmc_ver = redfish.bmc_version().await?;

        let xpu = XPU {
            redfish,
            bmc: bmc.clone(),
            vendor: "-".to_string(),
            serial_number: "-".to_string(),
            firmware_version: "-".to_string(),
            bmc_version: bmc_ver.version,
            status: XPUStatus::Ready,
        };

        Ok(xpu)
    }
}
