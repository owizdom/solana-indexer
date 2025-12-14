use crate::clients::solana::types::*;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RPCRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct RPCError {
    code: i64,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RPCResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RPCError>,
}

#[async_trait]
pub trait Client: Send + Sync {
    async fn get_latest_slot(&self) -> Result<u64>;
    async fn get_slot_by_number(&self, slot_number: u64) -> Result<SolanaSlot>;
    async fn get_program_logs(
        &self,
        program_id: &str,
        from_slot: u64,
        to_slot: u64,
    ) -> Result<Vec<SolanaProgramLog>>;
}

pub struct SolanaClient {
    http_client: reqwest::Client,
    base_url: String,
    block_commitment: BlockCommitment,
}

#[derive(Debug, Clone)]
pub struct SolanaClientConfig {
    pub base_url: String,
    pub block_commitment: BlockCommitment,
}

impl Default for SolanaClientConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            block_commitment: BlockCommitment::default(),
        }
    }
}

impl SolanaClient {
    pub fn new(config: SolanaClientConfig) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("Failed to create HTTP client")?;

        debug!("Creating new Solana client: {:?}", config);

        Ok(Self {
            http_client,
            base_url: config.base_url,
            block_commitment: config.block_commitment,
        })
    }

    pub fn set_http_client(&mut self, client: reqwest::Client) {
        self.http_client = client;
    }

    async fn call(&self, request: RPCRequest) -> Result<RPCResponse> {
        let backoffs = vec![1, 3, 5, 10, 20, 30, 60];

        for (attempt, &backoff) in backoffs.iter().enumerate() {
            let response = self.call_internal(&request).await;

            match response {
                Ok(resp) => {
                    if attempt > 0 {
                        info!(
                            "Successfully called after backoff: {}s, request: {:?}",
                            backoff, request
                        );
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    error!(
                        "Failed to call: {}, backoff: {}s, request: {:?}",
                        e, backoff, request
                    );
                    if attempt < backoffs.len() - 1 {
                        tokio::time::sleep(Duration::from_secs(backoff)).await;
                    }
                }
            }
        }

        error!("Exceeded retries for call: {:?}", request);
        anyhow::bail!("Exceeded retries for call")
    }

    async fn call_internal(&self, request: &RPCRequest) -> Result<RPCResponse> {
        let request_body = serde_json::to_string(request)
            .context("Failed to serialize request")?;

        debug!("Request body: {}", request_body);

        let response = self
            .http_client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(request_body)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .context("Request failed")?;

        if response.status() != reqwest::StatusCode::OK {
            anyhow::bail!("Received HTTP error code: {}", response.status());
        }

        let rpc_response: RPCResponse = response
            .json()
            .await
            .context("Failed to parse response")?;

        if let Some(error) = &rpc_response.error {
            anyhow::bail!("Received error response: {:?}", error);
        }

        Ok(rpc_response)
    }
}

#[async_trait]
impl Client for SolanaClient {
    async fn get_latest_slot(&self) -> Result<u64> {
        let request = RPCRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getSlot".to_string(),
            params: Some(serde_json::json!({
                "commitment": self.block_commitment.as_str()
            })),
        };

        let response = self.call(request).await?;

        let slot: u64 = serde_json::from_value(
            response.result.context("No result in response")?,
        )
        .context("Failed to parse slot")?;

        Ok(slot)
    }

    async fn get_slot_by_number(&self, slot_number: u64) -> Result<SolanaSlot> {
        let request = RPCRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getBlock".to_string(),
            params: Some(serde_json::json!([
                slot_number,
                {
                    "encoding": "json",
                    "transactionDetails": "full",
                    "rewards": false,
                    "commitment": self.block_commitment.as_str()
                }
            ])),
        };

        let response = self.call(request).await?;

        let mut slot: SolanaSlot = serde_json::from_value(
            response.result.context("No result in response")?,
        )
        .context("Failed to parse slot")?;

        slot.slot = slot_number;
        Ok(slot)
    }

    async fn get_program_logs(
        &self,
        program_id: &str,
        _from_slot: u64,
        _to_slot: u64,
    ) -> Result<Vec<SolanaProgramLog>> {
        let request = RPCRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getProgramLogs".to_string(),
            params: Some(serde_json::json!([
                program_id,
                {
                    "filters": [
                        {
                            "memcmp": {
                                "offset": 0,
                                "bytes": ""
                            }
                        }
                    ]
                }
            ])),
        };

        let response = self.call(request).await?;

        #[derive(Deserialize)]
        struct ProgramLogsResult {
            context: ContextInfo,
            value: Vec<ProgramLogEntry>,
        }

        #[derive(Deserialize)]
        struct ContextInfo {
            slot: u64,
        }

        #[derive(Deserialize)]
        struct ProgramLogEntry {
            signature: String,
            logs: Vec<String>,
        }

        let result: ProgramLogsResult = serde_json::from_value(
            response.result.context("No result in response")?,
        )
        .context("Failed to parse program logs")?;

        let mut logs = Vec::new();
        for entry in result.value {
            for (i, log_msg) in entry.logs.iter().enumerate() {
                if log_msg.starts_with(&format!("Program {}", program_id)) {
                    logs.push(SolanaProgramLog {
                        program_id: program_id.to_string(),
                        log_index: i as u64,
                        signature: entry.signature.clone(),
                        slot: result.context.slot,
                        block_time: None,
                        log_message: log_msg.clone(),
                        instruction_index: 0,
                    });
                }
            }
        }

        Ok(logs)
    }
}

