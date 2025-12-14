use crate::config::ChainId;
use crate::transaction_log_parser::DecodedLog;
use async_trait::async_trait;
use thiserror::Error;

pub mod memory;

#[derive(Debug, Clone)]
pub struct SlotRecord {
    pub slot: u64,
    pub blockhash: String,
    pub parent: u64,
    pub block_time: u64,
    pub chain_id: ChainId,
}

#[derive(Debug, Clone)]
pub struct LogWithSlot {
    pub log: DecodedLog,
    pub raw_log: crate::clients::solana::SolanaProgramLog,
    pub slot: crate::clients::solana::SolanaSlot,
}

#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("Item not found")]
    NotFound,
    #[error("Item already exists")]
    AlreadyExists,
    #[error("Storage is closed")]
    StoreClosed,
    #[error("Invalid chain ID")]
    InvalidChainId,
    #[error("Other error: {0}")]
    Other(String),
}

#[async_trait]
pub trait ChainPollerPersistence: Send + Sync {
    async fn get_last_processed_slot(
        &self,
        chain_id: ChainId,
    ) -> Result<Option<SlotRecord>, PersistenceError>;

    async fn save_slot(&self, slot: &SlotRecord) -> Result<(), PersistenceError>;

    async fn get_slot(
        &self,
        chain_id: ChainId,
        slot_number: u64,
    ) -> Result<Option<SlotRecord>, PersistenceError>;

    async fn delete_slot(
        &self,
        chain_id: ChainId,
        slot_number: u64,
    ) -> Result<(), PersistenceError>;

    async fn close(&self) -> Result<(), PersistenceError>;
}

#[async_trait]
pub trait SlotHandler: Send + Sync {
    async fn handle_slot(
        &self,
        slot: &crate::clients::solana::SolanaSlot,
    ) -> anyhow::Result<()>;

    async fn handle_log(&self, log_with_slot: &LogWithSlot) -> anyhow::Result<()>;

    async fn handle_reorg_slot(&self, slot_number: u64);
}

