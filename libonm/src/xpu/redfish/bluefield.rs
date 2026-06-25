use super::{BMCVersion, Redfish, RedfishError, BMC};

use crate::rest::{RestClient, RestConfig};
use async_trait::async_trait;

pub struct Bluefield {
    rest: RestClient,
    bmc: BMC,
}

const DEFAULT_PASSWORD: &str = "0penBmc";
const DEFAULT_USER: &str = "root";

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
        if self.bmc_version().await.is_ok() {
            return Ok(());
        }

        let default_bmc = Bluefield::default_bmc(&self.bmc.address);
        let default_redfish = Box::new(Bluefield::new(&default_bmc)?);
        default_redfish
            .change_password(self.bmc.password.clone())
            .await?;

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
        };

        Ok(Bluefield {
            rest: RestClient::new(&config)?,
            bmc: bmc.clone(),
        })
    }

    fn default_bmc(addr: &str) -> BMC {
        BMC {
            address: addr.to_string(),
            password: DEFAULT_PASSWORD.to_string(),
            username: DEFAULT_USER.to_string(),
        }
    }
}
