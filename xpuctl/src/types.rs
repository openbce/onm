use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BMC {
    pub name: String,
    pub vendor: String,
    pub address: String,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "default_tls_verify")]
    pub tls_verify: Option<bool>,
}

fn default_tls_verify() -> Option<bool> {
    None
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Context {
    pub username: String,
    pub password: String,
    #[serde(default = "default_context_tls_verify")]
    pub tls_verify: bool,

    pub bmc: Vec<BMC>,
}

fn default_context_tls_verify() -> bool {
    true
}

impl BMC {
    pub fn to_libonm_bmc(&self, default_user: &str, default_pass: &str, default_tls_verify: bool) -> libonm::xpu::BMC {
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
            tls_verify: self.tls_verify.unwrap_or(default_tls_verify),
        }
    }
}
