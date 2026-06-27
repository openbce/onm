use std::time::Duration;

use bytes::Bytes;
use http::Method;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use url::Url;

use reqwest::{
    header::HeaderValue, header::ACCEPT, header::CONTENT_TYPE, Certificate, Client, Identity,
};

const REST_TIMEOUT: Duration = Duration::from_secs(10);

pub struct RestClient {
    client: Client,
    base_url: Url,
    auth: RestAuth,
}

enum RestAuth {
    Basic { user: String, password: String },
    Bearer(String),
    None,
}

pub struct RestConfig {
    pub address: String,
    pub username: String,
    pub password: String,
    pub tls_verify: bool,
}

#[derive(Error, Debug)]
pub enum RestError {
    #[error("internal error: {0}")]
    Internal(String),
    #[error("JSON error: {message}")]
    Json { message: String, detail: String },
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("connection timeout to {0}")]
    Timeout(String),
    #[error("connection failed to {0}: {1}")]
    Connection(String, String),
    #[error("'{0}' not found")]
    NotFound(String),
    #[error("authentication failed: {0}")]
    AuthFailure(String),
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}

impl From<reqwest::Error> for RestError {
    fn from(value: reqwest::Error) -> Self {
        tracing::debug!("{:?}", value);
        if value.is_timeout() {
            RestError::Timeout(value.url().map(|u| u.to_string()).unwrap_or_default())
        } else if value.is_connect() {
            RestError::Connection(
                value.url().map(|u| u.to_string()).unwrap_or_default(),
                value.to_string(),
            )
        } else {
            RestError::Http(value.to_string())
        }
    }
}

impl From<serde_json::Error> for RestError {
    fn from(value: serde_json::Error) -> Self {
        tracing::debug!("{:?}", value);
        RestError::Json {
            message: value.to_string(),
            detail: format!("line {}, column {}", value.line(), value.column()),
        }
    }
}

impl From<std::io::Error> for RestError {
    fn from(value: std::io::Error) -> Self {
        RestError::Internal(value.to_string())
    }
}

impl RestClient {
    pub fn new(config: &RestConfig) -> Result<Self, RestError> {
        Self::new_advanced(config, None, None)
    }

    pub(crate) fn new_advanced(
        config: &RestConfig,
        bearer_token: Option<String>,
        client_identity: Option<(&[u8], &[u8], &[u8])>,
    ) -> Result<Self, RestError> {
        let mut base_url = Url::parse(&config.address)
            .map_err(|e| RestError::InvalidConfig(format!("invalid url: {}", e)))?;
        if !matches!(base_url.scheme(), "http" | "https") {
            return Err(RestError::InvalidConfig(format!(
                "unsupported URL scheme '{}'",
                base_url.scheme()
            )));
        }
        if base_url.host_str().is_none() {
            return Err(RestError::InvalidConfig("missing host in url".to_string()));
        }
        base_url.set_query(None);
        base_url.set_fragment(None);
        if !base_url.path().ends_with('/') {
            let path = format!("{}/", base_url.path());
            base_url.set_path(&path);
        }

        let mut builder = Client::builder()
            .danger_accept_invalid_certs(!config.tls_verify)
            .timeout(REST_TIMEOUT);

        if let Some((ca_pem, cert_pem, key_pem)) = client_identity {
            let ca = Certificate::from_pem(ca_pem)
                .map_err(|e| RestError::InvalidConfig(format!("invalid CA certificate: {e}")))?;
            let mut identity_pem = Vec::with_capacity(cert_pem.len() + key_pem.len() + 1);
            identity_pem.extend_from_slice(cert_pem);
            identity_pem.push(b'\n');
            identity_pem.extend_from_slice(key_pem);
            let identity = Identity::from_pem(&identity_pem)
                .map_err(|e| RestError::InvalidConfig(format!("invalid client identity: {e}")))?;
            builder = builder.add_root_certificate(ca).identity(identity);
        }

        let client = builder
            .build()
            .map_err(|e| RestError::Internal(format!("failed to build HTTP client: {}", e)))?;

        let auth = if let Some(token) = bearer_token {
            RestAuth::Bearer(token)
        } else if !config.username.is_empty() || !config.password.is_empty() {
            RestAuth::Basic {
                user: config.username.clone(),
                password: config.password.clone(),
            }
        } else {
            RestAuth::None
        };

        Ok(RestClient {
            client,
            base_url,
            auth,
        })
    }

    pub async fn get<'a, T: DeserializeOwned>(&self, path: &str) -> Result<T, RestError> {
        let resp = self.execute_request(Method::GET, path, None).await?;
        let data = deserialize_response(&resp)?;
        Ok(data)
    }

    pub async fn list<'a, T: DeserializeOwned>(&self, path: &str) -> Result<Vec<T>, RestError> {
        let resp = self.execute_request(Method::GET, path, None).await?;
        let data = deserialize_response(&resp)?;
        Ok(data)
    }

    pub async fn put<'a, S: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        o: &S,
    ) -> Result<T, RestError> {
        let input = serde_json::to_string(o)?;
        let resp = self.execute_request(Method::PUT, path, Some(input)).await?;
        let data = deserialize_response(&resp)?;
        Ok(data)
    }

    pub async fn post<'a, S: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        o: &S,
    ) -> Result<T, RestError> {
        let input = serde_json::to_string(o)?;
        let resp = self
            .execute_request(Method::POST, path, Some(input))
            .await?;
        let data = deserialize_response(&resp)?;
        Ok(data)
    }

    pub async fn delete<'a, T: DeserializeOwned>(&self, path: &str) -> Result<T, RestError> {
        let resp = self.execute_request(Method::DELETE, path, None).await?;
        let data = deserialize_response(&resp)?;
        Ok(data)
    }

    #[allow(dead_code)]
    pub async fn patch<'a, S: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        o: &S,
    ) -> Result<T, RestError> {
        let input = serde_json::to_string(o)?;
        let resp = self
            .execute_request(Method::PATCH, path, Some(input))
            .await?;
        let data = deserialize_response(&resp)?;
        Ok(data)
    }

    async fn execute_request(
        &self,
        method: Method,
        path: &str,
        data: Option<String>,
    ) -> Result<String, RestError> {
        let url = self
            .base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| RestError::InvalidConfig(format!("invalid request path: {e}")))?;

        let body = Bytes::from(data.clone().unwrap_or_default());
        tracing::debug!("Method: {method}, URL: {url}");

        let mut req = self
            .client
            .request(method, url.clone())
            .header(ACCEPT, HeaderValue::from_static("application/json"))
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .body(body);
        req = match &self.auth {
            RestAuth::Basic { user, password } => req.basic_auth(user, Some(password)),
            RestAuth::Bearer(token) => req.bearer_auth(token),
            RestAuth::None => req,
        };
        let req = req.build()?;
        let resp = self.client.execute(req).await?;

        let status = resp.status();
        let body = resp.text().await?;

        match status {
            StatusCode::OK
            | StatusCode::CREATED
            | StatusCode::ACCEPTED
            | StatusCode::NO_CONTENT => Ok(body),
            StatusCode::NOT_FOUND => Err(RestError::NotFound(url.to_string())),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(RestError::AuthFailure(body)),
            _ => Err(RestError::Http(format!("HTTP {}: {}", status, body))),
        }
    }
}

fn deserialize_response<T: DeserializeOwned>(body: &str) -> Result<T, RestError> {
    if body.trim().is_empty() {
        Ok(serde_json::from_str("null")?)
    } else {
        Ok(serde_json::from_str(body)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_response_deserializes_as_unit() {
        assert_eq!(deserialize_response::<()>("").unwrap(), ());
    }

    #[test]
    fn preserves_http_scheme_and_port() {
        let client = RestClient::new(&RestConfig {
            address: "http://127.0.0.1:8080".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            tls_verify: true,
        })
        .unwrap();

        assert_eq!(client.base_url.as_str(), "http://127.0.0.1:8080/");
    }
}
