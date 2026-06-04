use crate::error::ConnectorError;
use crate::models::{ColumnValue, QueryResult};
use crate::traits::WarehouseConnector;
use async_stream::stream;
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use may_secrets::{DwhSecret, SecretsProvider};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

struct CachedToken {
    token: String,
    expires_at: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BqQueryResponse {
    job_complete: bool,
    job_reference: BqJobReference,
    schema: Option<BqSchema>,
    rows: Option<Vec<BqRow>>,
    page_token: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BqJobReference {
    job_id: String,
}

#[derive(Deserialize, Debug)]
struct BqSchema {
    fields: Vec<BqField>,
}

#[derive(Deserialize, Debug)]
struct BqField {
    name: String,
    #[serde(rename = "type")]
    field_type: String,
}

#[derive(Deserialize, Debug)]
struct BqRow {
    f: Vec<BqCell>,
}

#[derive(Deserialize, Debug)]
struct BqCell {
    v: Value,
}

/// Maps a `BigQuery` JSON cell to a `ColumnValue`.
///
/// # Errors
///
/// Returns an error if parsing the value fails for the expected field type.
fn map_cell(
    value: &Value,
    field_type: &str,
    col_name: &str,
) -> Result<ColumnValue, ConnectorError> {
    if value.is_null() {
        return Ok(ColumnValue::Null);
    }
    match field_type {
        "STRING" => {
            if let Some(s) = value.as_str() {
                Ok(ColumnValue::Text(s.to_string()))
            } else {
                Ok(ColumnValue::Text(value.to_string()))
            }
        }
        "INTEGER" | "INT64" => {
            let n = if let Some(s) = value.as_str() {
                s.parse::<i64>().map_err(|_| {
                    ConnectorError::QueryFailed(format!(
                        "Invalid integer for column '{col_name}': {s}"
                    ))
                })?
            } else if let Some(n) = value.as_i64() {
                n
            } else {
                return Err(ConnectorError::QueryFailed(format!(
                    "Invalid integer for column '{col_name}': {value:?}"
                )));
            };
            Ok(ColumnValue::Int64(n))
        }
        "FLOAT" | "FLOAT64" => {
            let f = if let Some(s) = value.as_str() {
                s.parse::<f64>().map_err(|_| {
                    ConnectorError::QueryFailed(format!(
                        "Invalid float for column '{col_name}': {s}"
                    ))
                })?
            } else if let Some(f) = value.as_f64() {
                f
            } else {
                return Err(ConnectorError::QueryFailed(format!(
                    "Invalid float for column '{col_name}': {value:?}"
                )));
            };
            Ok(ColumnValue::Float64(f))
        }
        "BOOLEAN" | "BOOL" => {
            let b = if let Some(s) = value.as_str() {
                s == "true"
            } else if let Some(b) = value.as_bool() {
                b
            } else {
                return Err(ConnectorError::QueryFailed(format!(
                    "Invalid bool for column '{col_name}': {value:?}"
                )));
            };
            Ok(ColumnValue::Bool(b))
        }
        "BYTES" => {
            let s = value.as_str().ok_or_else(|| {
                ConnectorError::QueryFailed(format!(
                    "Expected string for BYTES column '{col_name}'"
                ))
            })?;
            let decoded = general_purpose::STANDARD.decode(s).map_err(|e| {
                ConnectorError::QueryFailed(format!("Invalid base64 for column '{col_name}': {e}"))
            })?;
            Ok(ColumnValue::Bytes(decoded))
        }
        _ => Ok(ColumnValue::Text(value.to_string())),
    }
}

pub struct BigQueryConnector {
    project_id: String,
    secret_name: String,
    secrets: Arc<dyn SecretsProvider>,
    auth_manager: RwLock<Option<Arc<dyn gcp_auth::TokenProvider>>>,
    // NOTE: Manual caching is used on top of gcp_auth because the 60-second
    // pre-emptive refresh buffer is a deliberate product decision.
    token_cache: Arc<RwLock<Option<CachedToken>>>,
    client: Client,
}

async fn fetch_or_refresh_token(
    token_cache: &Arc<RwLock<Option<CachedToken>>>,
    auth_manager: &Arc<dyn gcp_auth::TokenProvider>,
) -> Result<String, ConnectorError> {
    {
        let cache = token_cache.read().await;
        if let Some(cached) = cache.as_ref() {
            #[allow(clippy::collapsible_if, reason = "nested if is clearer")]
            if Utc::now() + chrono::Duration::seconds(60) < cached.expires_at {
                return Ok(cached.token.clone());
            }
        }
    }

    let token_arc = auth_manager
        .token(&["https://www.googleapis.com/auth/bigquery"])
        .await
        .map_err(|e| ConnectorError::ConnectionFailed(format!("Failed to get oauth token: {e}")))?;

    let token_str = token_arc.as_str().to_owned();

    let mut cache = token_cache.write().await;
    *cache = Some(CachedToken {
        token: token_str.clone(),
        expires_at: token_arc.expires_at(),
    });

    Ok(token_str)
}

impl BigQueryConnector {
    /// Creates a new `BigQueryConnector`.
    ///
    /// # Errors
    ///
    /// Returns a `ConnectorError::ConnectionFailed` if the underlying HTTP client cannot be built.
    pub fn new(
        project_id: impl Into<String>,
        secret_name: impl Into<String>,
        secrets: Arc<dyn SecretsProvider>,
    ) -> Result<Self, ConnectorError> {
        let client = reqwest::ClientBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ConnectorError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            project_id: project_id.into(),
            secret_name: secret_name.into(),
            secrets,
            auth_manager: RwLock::new(None),
            token_cache: Arc::new(RwLock::new(None)),
            client,
        })
    }

    async fn get_token(&self) -> Result<String, ConnectorError> {
        {
            let cache = self.token_cache.read().await;
            if let Some(cached) = cache.as_ref() {
                #[allow(clippy::collapsible_if, reason = "nested if is clearer")]
                if Utc::now() + chrono::Duration::seconds(60) < cached.expires_at {
                    return Ok(cached.token.clone());
                }
            }
        }

        let auth_manager = {
            let lock = self.auth_manager.read().await;
            if let Some(am) = lock.as_ref() {
                am.clone()
            } else {
                drop(lock);
                let mut write_lock = self.auth_manager.write().await;
                if write_lock.is_none() {
                    let secret = self
                        .secrets
                        .get_secret(&self.secret_name)
                        .await
                        .map_err(|e| ConnectorError::ConnectionFailed(e.to_string()))?;

                    let DwhSecret::ServiceAccountKey { json } = secret else {
                        return Err(ConnectorError::ConnectionFailed(
                            "Expected ServiceAccountKey secret for BigQuery".to_string(),
                        ));
                    };

                    let sa = gcp_auth::CustomServiceAccount::from_json(&json).map_err(|e| {
                        ConnectorError::ConnectionFailed(format!("Failed to parse SA json: {e}"))
                    })?;

                    *write_lock = Some(Arc::new(sa));
                }
                write_lock
                    .as_ref()
                    .ok_or_else(|| {
                        ConnectorError::ConnectionFailed(
                            "auth_manager not initialised — this is a bug".to_string(),
                        )
                    })?
                    .clone()
            }
        };

        fetch_or_refresh_token(&self.token_cache, &auth_manager).await
    }

    #[cfg(test)]
    pub async fn seed_cache_for_test(&self, token: String, expires_in_seconds: i64) {
        let mut cache = self.token_cache.write().await;
        *cache = Some(CachedToken {
            token,
            expires_at: Utc::now() + chrono::Duration::seconds(expires_in_seconds),
        });
    }
}

#[async_trait]
impl WarehouseConnector for BigQueryConnector {
    #[allow(
        clippy::too_many_lines,
        reason = "stream! macro makes extracting inner async loops complex"
    )]
    async fn execute(&self, sql: &str) -> Result<QueryResult, ConnectorError> {
        let token = self.get_token().await?;

        let url = format!(
            "https://bigquery.googleapis.com/bigquery/v2/projects/{}/queries",
            self.project_id
        );
        // NOTE: We intentionally use the runQuery endpoint here as a shortcut
        // instead of the jobs.insert API.
        let body = serde_json::json!({
            "query": sql,
            "useLegacySql": false,
            "timeoutMs": 30_000
        });

        let res = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| ConnectorError::QueryFailed(e.to_string()))?;

        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(ConnectorError::QueryFailed(err_text));
        }

        let bq_res: BqQueryResponse = res.json().await.map_err(|e| {
            ConnectorError::QueryFailed(format!("Failed to parse query response: {e}"))
        })?;

        let project_id = self.project_id.clone();
        let client = self.client.clone();

        let auth_manager = {
            let lock = self.auth_manager.read().await;
            lock.as_ref().unwrap().clone()
        };

        let token_cache = self.token_cache.clone();

        let stream = stream! {
            let mut current_res = bq_res;

            loop {
                while !current_res.job_complete {
                    let job_id = &current_res.job_reference.job_id;

                    let poll_url = format!(
                        "https://bigquery.googleapis.com/bigquery/v2/projects/{project_id}/queries/{job_id}"
                    );

                    let token = match fetch_or_refresh_token(&token_cache, &auth_manager).await {
                        Ok(t) => t,
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    };

                    let res = match client.get(&poll_url).bearer_auth(token.as_str()).send().await {
                        Ok(r) => r,
                        Err(e) => {
                            yield Err(ConnectorError::QueryFailed(format!("Poll request failed: {e}")));
                            return;
                        }
                    };

                    if !res.status().is_success() {
                        let err_text = res.text().await.unwrap_or_default();
                        yield Err(ConnectorError::QueryFailed(format!("Poll failed: {err_text}")));
                        return;
                    }

                    current_res = match res.json().await {
                        Ok(r) => r,
                        Err(e) => {
                            yield Err(ConnectorError::QueryFailed(format!("Failed to parse poll response: {e}")));
                            return;
                        }
                    };

                    if !current_res.job_complete {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }

                let Some(schema) = current_res.schema.as_ref() else {
                    return;
                };

                if let Some(rows) = current_res.rows.as_ref() {
                    for row in rows {
                        let mut out_row = Vec::with_capacity(schema.fields.len());
                        for (i, cell) in row.f.iter().enumerate() {
                            let field = &schema.fields[i];
                            match map_cell(&cell.v, &field.field_type, &field.name) {
                                Ok(val) => out_row.push(val),
                                Err(e) => {
                                    yield Err(e);
                                    return;
                                }
                            }
                        }
                        yield Ok(out_row);
                    }
                }

                if let Some(page_token) = current_res.page_token {
                    let job_id = &current_res.job_reference.job_id;

                    let next_url = format!(
                        "https://bigquery.googleapis.com/bigquery/v2/projects/{project_id}/queries/{job_id}?pageToken={page_token}"
                    );

                    let token = match fetch_or_refresh_token(&token_cache, &auth_manager).await {
                        Ok(t) => t,
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    };

                    let res = match client.get(&next_url).bearer_auth(token.as_str()).send().await {
                        Ok(r) => r,
                        Err(e) => {
                            yield Err(ConnectorError::QueryFailed(format!("Next page request failed: {e}")));
                            return;
                        }
                    };

                    if !res.status().is_success() {
                        let err_text = res.text().await.unwrap_or_default();
                        yield Err(ConnectorError::QueryFailed(format!("Next page failed: {err_text}")));
                        return;
                    }

                    current_res = match res.json().await {
                        Ok(r) => r,
                        Err(e) => {
                            yield Err(ConnectorError::QueryFailed(format!("Failed to parse next page: {e}")));
                            return;
                        }
                    };
                } else {
                    break;
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
#[path = "bigquery_tests.rs"]
mod tests;
