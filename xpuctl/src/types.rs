use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BMC {
    pub name: String,
    pub vendor: String,
    pub address: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Context {
    pub username: String,
    pub password: String,

    pub bmc: Vec<BMC>,
}

impl From<(&BMC, &str, &str)> for libonm::xpu::BMC {
    fn from((bmc, default_user, default_pass): (&BMC, &str, &str)) -> Self {
        libonm::xpu::BMC {
            username: bmc
                .username
                .clone()
                .unwrap_or_else(|| default_user.to_string()),
            address: bmc.address.clone(),
            password: bmc
                .password
                .clone()
                .unwrap_or_else(|| default_pass.to_string()),
        }
    }
}
