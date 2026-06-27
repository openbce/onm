use std::time::Duration;

use bytes::Bytes;
use http::Method;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use url::Url;

use reqwest::{header::HeaderValue, header::ACCEPT, header::CONTENT_TYPE, Client};

const REST_TIMEOUT: Duration = Duration::from_secs(10);

pub struct RestClient {
    client: Client,
    address: String,
    user: String,
    password: String,
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
        let url = Url::parse(&config.address)
            .map_err(|e| RestError::InvalidConfig(format!("invalid url: {}", e)))?;
        let host = url
            .host_str()
            .ok_or(RestError::InvalidConfig("missing host in url".to_string()))?;
        let port = url.port().unwrap_or(443);
        let address = format!("{}:{}", host, port);

        let client = Client::builder()
            .danger_accept_invalid_certs(!config.tls_verify)
            .timeout(REST_TIMEOUT)
            .build()
            .map_err(|e| RestError::Internal(format!("failed to build HTTP client: {}", e)))?;

        Ok(RestClient {
            client,
            address,
            user: config.username.clone(),
            password: config.password.clone(),
        })
    }

    pub async fn get<'a, T: DeserializeOwned>(&self, path: &str) -> Result<T, RestError> {
        let resp = self.execute_request(Method::GET, path, None).await?;
        let data = serde_json::from_str(&resp)?;
        Ok(data)
    }

    pub async fn list<'a, T: DeserializeOwned>(&self, path: &str) -> Result<Vec<T>, RestError> {
        let resp = self.execute_request(Method::GET, path, None).await?;
        let data = serde_json::from_str(&resp)?;
        Ok(data)
    }

    pub async fn put<'a, S: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        o: &S,
    ) -> Result<T, RestError> {
        let input = serde_json::to_string(o)?;
        let resp = self.execute_request(Method::PUT, path, Some(input)).await?;
        let data = serde_json::from_str(&resp)?;
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
        let data = serde_json::from_str(&resp)?;
        Ok(data)
    }

    pub async fn delete<'a, T: DeserializeOwned>(&self, path: &str) -> Result<T, RestError> {
        let resp = self.execute_request(Method::DELETE, path, None).await?;
        let data = serde_json::from_str(&resp)?;
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
        let data = serde_json::from_str(&resp)?;
        Ok(data)
    }

    async fn execute_request(
        &self,
        method: Method,
        path: &str,
        data: Option<String>,
    ) -> Result<String, RestError> {
        let schema = "https";
        let url = format!("{}://{}/{}", schema, self.address, path.trim_matches('/'));

        let body = Bytes::from(data.clone().unwrap_or_default());
        tracing::debug!(
            "Method: {method}, URL: {url}, Auth: <{}/***>",
            self.user,
        );

        let req = self
            .client
            .request(method, url.clone())
            .header(ACCEPT, HeaderValue::from_static("application/json"))
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .body(body)
            .basic_auth(&self.user, Some(self.password.clone()))
            .build()?;
        let resp = self.client.execute(req).await?;

        let status = resp.status();
        let body = resp.text().await?;

        match status {
            StatusCode::OK
            | StatusCode::CREATED
            | StatusCode::ACCEPTED
            | StatusCode::NO_CONTENT => Ok(body),
            StatusCode::NOT_FOUND => Err(RestError::NotFound(url)),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(RestError::AuthFailure(body)),
            _ => Err(RestError::Http(format!("HTTP {}: {}", status, body))),
        }
    }
}
