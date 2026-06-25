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

impl BMC {
    pub fn to_libonm_bmc(&self, default_user: &str, default_pass: &str) -> libonm::xpu::BMC {
        libonm::xpu::BMC {
            username: self
                .username
                .clone()
                .unwrap_or_else(|| default_user.to_string()),
            address: self.address.clone(),
            password: self
                .password
                .clone()
                .unwrap_or_else(|| default_pass.to_string()),
        }
    }
}
