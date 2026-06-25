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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BMC {
    pub address: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BMCVersion {
    pub description: String,
    pub id: String,
    pub version: String,
}

pub struct XPU {
    redfish: Box<dyn Redfish>,

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
            vendor: "-".to_string(),
            serial_number: "-".to_string(),
            firmware_version: "-".to_string(),
            bmc_version: bmc_ver.version,
            status: XPUStatus::Ready,
        };

        Ok(xpu)
    }
}
