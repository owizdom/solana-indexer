use crate::clients::solana::SolanaProgramLog;
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct DecodedLog {
    pub log_index: u64,
    pub address: String,
    pub arguments: Vec<Argument>,
    pub event_name: String,
    pub output_data: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct Argument {
    pub name: String,
    pub arg_type: String,
    pub value: serde_json::Value,
    pub indexed: bool,
}

#[async_trait]
pub trait LogParser: Send + Sync {
    async fn decode_log(
        &self,
        program_id: &str,
        log: &SolanaProgramLog,
    ) -> anyhow::Result<DecodedLog>;
}

pub struct TransactionLogParser {
    // For future use: program IDL store
}

impl TransactionLogParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for TransactionLogParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LogParser for TransactionLogParser {
    async fn decode_log(
        &self,
        _program_id: &str,
        log: &SolanaProgramLog,
    ) -> anyhow::Result<DecodedLog> {
        debug!(
            "Decoding log with signature: '{}' programId: '{}'",
            log.signature, log.program_id
        );

        let mut decoded_log = DecodedLog {
            address: log.program_id.clone(),
            log_index: log.log_index,
            event_name: String::new(),
            arguments: Vec::new(),
            output_data: HashMap::new(),
        };

        // Parse the log message to extract event information
        // Solana logs typically follow patterns like:
        // "Program <programId> invoke [1]"
        // "Program log: <message>"
        // "Program <programId> success"

        let log_msg = &log.log_message;
        if log_msg.starts_with("Program log: ") {
            let event_data = log_msg.strip_prefix("Program log: ").unwrap_or(log_msg);
            decoded_log.event_name = "ProgramLog".to_string();
            decoded_log.output_data.insert(
                "message".to_string(),
                serde_json::Value::String(event_data.to_string()),
            );
            decoded_log.arguments.push(Argument {
                name: "message".to_string(),
                arg_type: "string".to_string(),
                value: serde_json::Value::String(event_data.to_string()),
                indexed: false,
            });
        } else {
            // For other log types, store the raw message
            decoded_log.event_name = "Unknown".to_string();
            decoded_log
                .output_data
                .insert("raw".to_string(), serde_json::Value::String(log_msg.clone()));
            decoded_log.arguments.push(Argument {
                name: "raw".to_string(),
                arg_type: "string".to_string(),
                value: serde_json::Value::String(log_msg.clone()),
                indexed: false,
            });
        }

        Ok(decoded_log)
    }
}

