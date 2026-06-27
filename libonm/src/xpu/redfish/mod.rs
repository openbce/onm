use crate::rest::RestError;
use async_trait::async_trait;
use thiserror::Error;

use super::{BMCVersion, BMC};

use bluefield::Bluefield;

mod bluefield;

#[async_trait]
#[allow(dead_code)]
pub trait Redfish {
    async fn discover(&self) -> Result<(), RedfishError>;
    async fn change_password(&self, passwd: String) -> Result<(), RedfishError>;
    async fn bmc_version(&self) -> Result<BMCVersion, RedfishError>;
}

#[derive(Error, Debug)]
pub enum RedfishError {
    #[error("{0}")]
    RestError(String),
    #[error("{0}")]
    IOError(String),
    #[error("{0}")]
    Json(String),
}

impl From<RestError> for RedfishError {
    fn from(value: RestError) -> Self {
        RedfishError::RestError(value.to_string())
    }
}

impl From<std::io::Error> for RedfishError {
    fn from(value: std::io::Error) -> Self {
        RedfishError::IOError(value.to_string())
    }
}

pub fn build(bmc: &BMC) -> Result<Box<dyn Redfish>, RedfishError> {
    Ok(Box::new(Bluefield::new(bmc)?))
}
