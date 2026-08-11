use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;
use url::Url;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("invalid API URL: {0}")]
    InvalidUrl(String),
    #[error("failed to build HTTP client: {0}")]
    BuildClient(#[source] reqwest::Error),
    #[error("request to {url} timed out")]
    Timeout { url: String },
    #[error("request to {url} failed: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("OAuth token request failed (HTTP {status}): {message}")]
    OAuthHttp { status: StatusCode, message: String },
    #[error("Tailscale API returned HTTP {status}: {message}")]
    Http { status: StatusCode, message: String },
    #[error(
        "Tailscale API returned HTTP {status}: {message}; OAuth token scopes are [{token_scope}] — grant Devices Read (devices:core:read) on the OAuth client at https://login.tailscale.com/admin/settings/trust-credentials"
    )]
    PermissionDenied {
        status: StatusCode,
        message: String,
        token_scope: String,
    },
    #[error("OAuth token response missing access_token")]
    MissingAccessToken,
    #[error("device not found: {query}")]
    DeviceNotFound { query: String },
    #[error("ambiguous device query {query}: matched {count} devices")]
    AmbiguousDevice { query: String, count: usize },
    #[error("invalid JSON returned by Tailscale: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct TailscaleClient {
    client: Client,
    base_url: Url,
    api_key: String,
    /// Scopes granted on the current access token, when known (OAuth mint).
    /// `Some` also marks that auth came from OAuth client credentials.
    token_scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub node_id: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub client_version: String,
    #[serde(default)]
    pub update_available: bool,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub last_seen: String,
    #[serde(default)]
    pub key_expiry_disabled: bool,
    #[serde(default)]
    pub expires: String,
    #[serde(default)]
    pub authorized: bool,
    #[serde(default)]
    pub is_external: bool,
    #[serde(default)]
    pub machine_key: String,
    #[serde(default)]
    pub node_key: String,
    #[serde(default)]
    pub blocks_incoming_connections: bool,
    #[serde(default)]
    pub enabled_routes: Vec<String>,
    #[serde(default)]
    pub advertised_routes: Vec<String>,
    #[serde(default)]
    pub tailnet_lock_key: String,
    #[serde(default, rename = "tailnetLockError")]
    pub tailnet_lock_error: String,
}

#[derive(Debug, Deserialize)]
struct DevicesResponse {
    devices: Vec<Device>,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

impl TailscaleClient {
    pub fn new(api_url: &str, api_key: String) -> Result<Self, ApiError> {
        let mut base_url =
            Url::parse(api_url).map_err(|error| ApiError::InvalidUrl(error.to_string()))?;
        if !matches!(base_url.scheme(), "http" | "https") || base_url.host_str().is_none() {
            return Err(ApiError::InvalidUrl(api_url.to_string()));
        }
        if !base_url.path().ends_with('/') {
            let path = format!("{}/", base_url.path());
            base_url.set_path(&path);
        }

        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("tsctl/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(ApiError::BuildClient)?;

        Ok(Self {
            client,
            base_url,
            api_key,
            token_scope: None,
        })
    }

    pub async fn from_oauth(
        api_url: &str,
        client_id: &str,
        client_secret: &str,
        scope: &str,
    ) -> Result<Self, ApiError> {
        let bootstrap = Self::new(api_url, String::new())?;
        let (access_token, token_scope) = bootstrap
            .exchange_oauth_token(client_id, client_secret, scope)
            .await?;
        Ok(Self {
            api_key: access_token,
            token_scope,
            ..bootstrap
        })
    }

    pub async fn exchange_oauth_token(
        &self,
        client_id: &str,
        client_secret: &str,
        scope: &str,
    ) -> Result<(String, Option<String>), ApiError> {
        let url = self.url(&["oauth", "token"])?;
        let url_text = url.to_string();
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        form.append_pair("grant_type", "client_credentials")
            .append_pair("client_id", client_id)
            .append_pair("client_secret", client_secret);
        let scope = scope.trim();
        if !scope.is_empty() {
            form.append_pair("scope", scope);
        }
        let body = form.finish();
        let response = self
            .client
            .post(url)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(body)
            .send()
            .await
            .map_err(|source| {
                if source.is_timeout() {
                    ApiError::Timeout {
                        url: url_text.clone(),
                    }
                } else {
                    ApiError::Request {
                        url: url_text.clone(),
                        source,
                    }
                }
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|source| ApiError::Request {
            url: url_text,
            source,
        })?;

        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|value| {
                    value
                        .get("error_description")
                        .or_else(|| value.get("message"))
                        .and_then(|value| value.as_str())
                        .map(str::to_owned)
                })
                .unwrap_or_else(|| body.trim().to_owned());
            return Err(ApiError::OAuthHttp { status, message });
        }

        let token: OAuthTokenResponse = serde_json::from_str(&body)?;
        let access_token = token
            .access_token
            .filter(|value| !value.trim().is_empty())
            .ok_or(ApiError::MissingAccessToken)?;
        let token_scope = Some(
            token
                .scope
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "(none reported)".to_owned()),
        );
        Ok((access_token, token_scope))
    }

    pub async fn list_devices(
        &self,
        tailnet: &str,
        all_fields: bool,
    ) -> Result<(Vec<Device>, Value), ApiError> {
        let mut url = self.url(&["tailnet", tailnet, "devices"])?;
        url.query_pairs_mut()
            .append_pair("fields", if all_fields { "all" } else { "default" });
        let value: Value = self.get(url).await?;
        let response: DevicesResponse = serde_json::from_value(value.clone())?;
        Ok((response.devices, value))
    }

    pub async fn get_device(&self, device_id: &str) -> Result<(Device, Value), ApiError> {
        let url = self.url(&["device", device_id])?;
        let value: Value = self.get(url).await?;
        let device = serde_json::from_value(value.clone())?;
        Ok((device, value))
    }

    /// Resolve a device by device ID, node ID, MagicDNS name, or hostname.
    pub async fn resolve_device(&self, query: &str) -> Result<(Device, Value), ApiError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(ApiError::DeviceNotFound {
                query: query.to_owned(),
            });
        }

        match self.get_device(query).await {
            Ok(result) => Ok(result),
            Err(ApiError::Http { status, .. }) if status == StatusCode::NOT_FOUND => {
                self.find_device_in_tailnet("-", query).await
            }
            Err(error) => Err(error),
        }
    }

    async fn find_device_in_tailnet(
        &self,
        tailnet: &str,
        query: &str,
    ) -> Result<(Device, Value), ApiError> {
        let (devices, _) = self.list_devices(tailnet, true).await?;
        let matches: Vec<&Device> = devices
            .iter()
            .filter(|device| device_matches(device, query))
            .collect();

        match matches.as_slice() {
            [device] => {
                let id = if !device.id.is_empty() {
                    device.id.as_str()
                } else {
                    device.node_id.as_str()
                };
                if id.is_empty() {
                    return Err(ApiError::DeviceNotFound {
                        query: query.to_owned(),
                    });
                }
                self.get_device(id).await
            }
            [] => Err(ApiError::DeviceNotFound {
                query: query.to_owned(),
            }),
            many => Err(ApiError::AmbiguousDevice {
                query: query.to_owned(),
                count: many.len(),
            }),
        }
    }

    fn url(&self, segments: &[&str]) -> Result<Url, ApiError> {
        let mut url = self.base_url.clone();
        let mut path = url
            .path_segments_mut()
            .map_err(|_| ApiError::InvalidUrl(self.base_url.to_string()))?;
        path.pop_if_empty().extend(segments);
        drop(path);
        Ok(url)
    }

    async fn get<T: DeserializeOwned>(&self, url: Url) -> Result<T, ApiError> {
        let url_text = url.to_string();
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|source| {
                if source.is_timeout() {
                    ApiError::Timeout {
                        url: url_text.clone(),
                    }
                } else {
                    ApiError::Request {
                        url: url_text.clone(),
                        source,
                    }
                }
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|source| ApiError::Request {
            url: url_text,
            source,
        })?;

        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|value| value.get("message")?.as_str().map(str::to_owned))
                .unwrap_or_else(|| body.trim().to_owned());
            if status == StatusCode::FORBIDDEN {
                if let Some(token_scope) = self.token_scope.as_deref() {
                    return Err(ApiError::PermissionDenied {
                        status,
                        message,
                        token_scope: token_scope.to_owned(),
                    });
                }
            }
            return Err(ApiError::Http { status, message });
        }

        Ok(serde_json::from_str(&body)?)
    }
}

fn device_matches(device: &Device, query: &str) -> bool {
    [
        device.id.as_str(),
        device.node_id.as_str(),
        device.name.as_str(),
        device.hostname.as_str(),
    ]
    .into_iter()
    .any(|value| !value.is_empty() && value.eq_ignore_ascii_case(query))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_escapes_api_paths() {
        let client =
            TailscaleClient::new("https://example.test/api/v2", "secret".to_string()).unwrap();
        let url = client
            .url(&["tailnet", "user@example.com", "devices"])
            .unwrap();

        assert_eq!(
            url.as_str(),
            "https://example.test/api/v2/tailnet/user@example.com/devices"
        );
    }

    #[test]
    fn builds_oauth_token_url() {
        let client =
            TailscaleClient::new("https://example.test/api/v2", "secret".to_string()).unwrap();
        let url = client.url(&["oauth", "token"]).unwrap();

        assert_eq!(url.as_str(), "https://example.test/api/v2/oauth/token");
    }

    #[test]
    fn device_schema_allows_omitted_and_unknown_fields() {
        let device: Device = serde_json::from_value(serde_json::json!({
            "id": "123",
            "nodeId": "n123",
            "hostname": "server",
            "futureField": "accepted"
        }))
        .unwrap();

        assert_eq!(device.id, "123");
        assert_eq!(device.node_id, "n123");
        assert_eq!(device.hostname, "server");
        assert!(device.addresses.is_empty());
    }

    #[test]
    fn device_matches_name_and_ids() {
        let device: Device = serde_json::from_value(serde_json::json!({
            "id": "123",
            "nodeId": "n123CNTRL",
            "name": "box.tailnet.ts.net",
            "hostname": "box"
        }))
        .unwrap();

        assert!(device_matches(&device, "box.tailnet.ts.net"));
        assert!(device_matches(&device, "BOX"));
        assert!(device_matches(&device, "n123CNTRL"));
        assert!(device_matches(&device, "123"));
        assert!(!device_matches(&device, "other"));
    }
}
