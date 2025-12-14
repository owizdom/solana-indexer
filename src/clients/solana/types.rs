use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use crate::config::ChainId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaSlot {
    pub slot: u64,
    pub parent: Option<u64>,
    pub blockhash: String,
    pub block_time: Option<i64>,
    pub transactions: Vec<SolanaTransaction>,
    pub chain_id: ChainId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaTransaction {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,
    pub fee: u64,
    pub success: bool,
    pub account_keys: Vec<String>,
    pub program_ids: Vec<String>,
    pub log_messages: Vec<String>,
    pub pre_balances: Vec<u64>,
    pub post_balances: Vec<u64>,
    pub inner_instructions: Vec<SolanaInnerInstruction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaInnerInstruction {
    pub index: usize,
    pub instructions: Vec<SolanaInstruction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaInstruction {
    pub program_id_index: usize,
    pub accounts: Vec<usize>,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaProgramLog {
    pub program_id: String,
    pub log_index: u64,
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,
    pub log_message: String,
    pub instruction_index: usize,
}

impl SolanaProgramLog {
    pub fn program_id_pubkey(&self) -> anyhow::Result<Pubkey> {
        Pubkey::from_str(&self.program_id)
            .map_err(|e| anyhow::anyhow!("Failed to parse program ID: {}", e))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockCommitment {
    Finalized,
    Confirmed,
    Processed,
}

impl BlockCommitment {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockCommitment::Finalized => "finalized",
            BlockCommitment::Confirmed => "confirmed",
            BlockCommitment::Processed => "processed",
        }
    }
}

impl Default for BlockCommitment {
    fn default() -> Self {
        BlockCommitment::Finalized
    }
}

