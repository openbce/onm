use super::{BMCVersion, Redfish, RedfishError, BMC};

use crate::rest::{RestClient, RestConfig};
use async_trait::async_trait;

pub struct Bluefield {
    rest: RestClient,
}

#[async_trait]
impl Redfish for Bluefield {
    async fn change_password(&self, passwd: String) -> Result<(), RedfishError> {
        let mut data = std::collections::HashMap::new();
        data.insert("Password", passwd);

        self.rest
            .patch::<_, ()>("/redfish/v1/AccountService/Accounts/root", &data)
            .await
            .map_err(RedfishError::from)?;

        Ok(())
    }

    async fn bmc_version(&self) -> Result<BMCVersion, RedfishError> {
        let resp = self
            .rest
            .get("redfish/v1/UpdateService/FirmwareInventory/BMC_Firmware")
            .await
            .map_err(RedfishError::from)?;

        Ok(resp)
    }

    async fn discover(&self) -> Result<(), RedfishError> {
        let _ = self.bmc_version().await?;
        Ok(())
    }
}

impl Bluefield {
    pub fn new(bmc: &BMC) -> Result<Bluefield, RedfishError> {
        let config = RestConfig {
            address: bmc.address.clone(),
            password: bmc.password.clone(),
            username: bmc.username.clone(),
            tls_verify: bmc.tls_verify,
        };

        Ok(Bluefield {
            rest: RestClient::new(&config)?,
        })
    }
}
