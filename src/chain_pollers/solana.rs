use crate::chain_pollers::persistence::*;
use crate::clients::solana::{Client, SolanaSlot};
use crate::config::ChainId;
use crate::contract_store::ContractStore;
use crate::transaction_log_parser::LogParser;
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

pub struct SolanaChainPollerConfig {
    pub chain_id: ChainId,
    pub polling_interval: Duration,
    pub interesting_programs: Vec<String>,
    pub max_reorg_depth: usize,
    pub slot_history_size: usize,
    pub reorg_check_enabled: bool,
}

impl Default for SolanaChainPollerConfig {
    fn default() -> Self {
        Self {
            chain_id: 101, // Mainnet
            polling_interval: Duration::from_secs(12),
            interesting_programs: Vec::new(),
            max_reorg_depth: 10,
            slot_history_size: 100,
            reorg_check_enabled: true,
        }
    }
}

pub struct SolanaChainPoller {
    client: Arc<dyn Client>,
    log_parser: Arc<dyn LogParser>,
    config: SolanaChainPollerConfig,
    contract_store: Arc<dyn ContractStore>,
    store: Arc<dyn ChainPollerPersistence>,
    slot_handler: Arc<dyn SlotHandler>,
}

impl SolanaChainPoller {
    pub fn new(
        client: Arc<dyn Client>,
        log_parser: Arc<dyn LogParser>,
        config: SolanaChainPollerConfig,
        contract_store: Arc<dyn ContractStore>,
        store: Arc<dyn ChainPollerPersistence>,
        slot_handler: Arc<dyn SlotHandler>,
    ) -> Self {
        let mut config = config;
        if config.max_reorg_depth == 0 {
            config.max_reorg_depth = 10;
        }
        if config.slot_history_size == 0 {
            config.slot_history_size = 100;
        }
        if !config.reorg_check_enabled && config.max_reorg_depth > 0 {
            config.reorg_check_enabled = true;
        }

        info!(
            chain_id = config.chain_id,
            polling_interval = ?config.polling_interval,
            "Creating Solana chain poller"
        );

        for (i, program) in config.interesting_programs.iter().enumerate() {
            info!("InterestingProgram {}: {}", i, program);
        }

        Self {
            client,
            log_parser,
            config,
            contract_store,
            store,
            slot_handler,
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!(
            chain_id = self.config.chain_id,
            polling_interval = ?self.config.polling_interval,
            "Starting Solana Listener"
        );

        let last_slot_record = self
            .store
            .get_last_processed_slot(self.config.chain_id)
            .await
            .context("Failed to get last processed slot")?;

        let last_slot_record = if let Some(record) = last_slot_record {
            record
        } else {
            info!("Poller could not get last processed slot, using latest slot");
            let latest_slot = self
                .client
                .get_latest_slot()
                .await
                .context("Error getting latest slot")?;

            let last_canon_slot = self
                .client
                .get_slot_by_number(latest_slot)
                .await
                .context("Couldn't get last canonical slot")?;

            let record = SlotRecord {
                slot: last_canon_slot.slot,
                blockhash: last_canon_slot.blockhash.clone(),
                parent: last_canon_slot.parent.unwrap_or(0),
                block_time: last_canon_slot.block_time.unwrap_or(0) as u64,
                chain_id: self.config.chain_id,
            };

            self.store
                .save_slot(&record)
                .await
                .context("Failed to save last processed slot")?;

            record
        };

        info!(
            slot = last_slot_record.slot,
            "Starting from slot: {}",
            last_slot_record.slot
        );

        self.poll_for_slots().await;

        Ok(())
    }

    async fn poll_for_slots(&self) {
        info!("Starting Solana Chain Listener poll loop");
        let mut interval = interval(self.config.polling_interval);

        loop {
            interval.tick().await;
            if let Err(e) = self.process_next_slot().await {
                error!("Error processing next slot: {}", e);
            }
        }
    }

    async fn process_next_slot(&self) -> Result<()> {
        let latest_slot_record = self
            .store
            .get_last_processed_slot(self.config.chain_id)
            .await
            .context("Error getting last processed slot")?
            .context("Last processed slot must exist")?;

        let latest_slot_num = self
            .client
            .get_latest_slot()
            .await
            .context("Error getting latest slot number")?;

        if latest_slot_record.slot == latest_slot_num {
            debug!(
                last_observed_slot = latest_slot_record.slot,
                latest_slot = latest_slot_num,
                "Skipping slot processing as the last observed slot is the same as the latest slot"
            );
            return Ok(());
        }

        let mut slots_to_fetch = Vec::new();
        if latest_slot_num > latest_slot_record.slot {
            for i in (latest_slot_record.slot + 1)..=latest_slot_num {
                slots_to_fetch.push(i);
            }
        }

        let slots_count = slots_to_fetch.len();
        debug!("Fetching slots with logs: {} slots", slots_count);

        for slot_num in slots_to_fetch {
            let new_canon_slot = self
                .client
                .get_slot_by_number(slot_num)
                .await
                .context("Failed to fetch slot for reorg check")?;

            let parent_slot = new_canon_slot.parent.unwrap_or(0);
            if parent_slot != latest_slot_record.slot {
                warn!(
                    slot_number = slot_num,
                    expected_parent = latest_slot_record.slot,
                    actual_parent = parent_slot,
                    chain_id = self.config.chain_id,
                    "Blockchain reorganization detected"
                );

                if let Err(e) = self.reconcile_reorg(&new_canon_slot).await {
                    error!("Failed to reconcile reorg: {}", e);
                }
                return Ok(());
            }

            if let Err(e) = self.slot_handler.handle_slot(&new_canon_slot).await {
                error!("Error handling new slot: slot={}, error={}", slot_num, e);
            }

            let latest_slot_record = self
                .process_slot_logs(&new_canon_slot)
                .await
                .context("Error fetching slot with logs")?;

            if let Some(record) = latest_slot_record {
                if self.config.slot_history_size > 0
                    && record.slot > self.config.slot_history_size as u64
                {
                    let old_slot_num = record.slot - self.config.slot_history_size as u64;
                    if let Err(e) = self
                        .store
                        .delete_slot(self.config.chain_id, old_slot_num)
                        .await
                    {
                        debug!(
                            "Failed to prune old slot: slot={}, error={}",
                            old_slot_num, e
                        );
                    }
                }
            }
        }

        debug!("All slots processed: {} slots", slots_count);

        Ok(())
    }

    async fn process_slot_logs(
        &self,
        slot: &SolanaSlot,
    ) -> Result<Option<SlotRecord>> {
        let logs = self
            .fetch_logs_for_interesting_programs_for_slot(slot.slot)
            .await
            .context("Error fetching logs for slot")?;

        info!(
            latest_slot_num = slot.slot,
            blockhash = slot.blockhash,
            log_count = logs.len(),
            "Slot fetched with logs"
        );

        for log in logs {
            let decoded_log = self
                .log_parser
                .decode_log(&log.program_id, &log)
                .await
                .context("Failed to decode log")?;

            let log_with_slot = LogWithSlot {
                log: decoded_log,
                raw_log: log.clone(),
                slot: slot.clone(),
            };

            if let Err(e) = self.slot_handler.handle_log(&log_with_slot).await {
                error!("Error handling log: {}", e);
                return Err(e);
            }
        }

        debug!("Processed logs for slot: {}", slot.slot);

        let slot_record = SlotRecord {
            slot: slot.slot,
            blockhash: slot.blockhash.clone(),
            parent: slot.parent.unwrap_or(0),
            block_time: slot.block_time.unwrap_or(0) as u64,
            chain_id: self.config.chain_id,
        };

        self.store
            .save_slot(&slot_record)
            .await
            .context("Failed to save slot info")?;

        Ok(Some(slot_record))
    }

    async fn fetch_logs_for_interesting_programs_for_slot(
        &self,
        slot_number: u64,
    ) -> Result<Vec<crate::clients::solana::SolanaProgramLog>> {
        let all_programs: Vec<String> = self
            .config
            .interesting_programs
            .iter()
            .filter(|p| !p.is_empty())
            .map(|s| s.to_lowercase())
            .collect();

        info!(
            programs = ?all_programs,
            slot = slot_number,
            "Fetching logs for interesting programs"
        );

        let mut all_logs = Vec::new();

        for program in all_programs {
            debug!(
                program = program,
                slot = slot_number,
                "Fetching logs for program"
            );

            match self
                .client
                .get_program_logs(&program, slot_number, slot_number)
                .await
            {
                Ok(logs) => {
                    if logs.is_empty() {
                        debug!(
                            program = program,
                            slot = slot_number,
                            "No logs found for program"
                        );
                    } else {
                        info!(
                            program = program,
                            slot = slot_number,
                            log_count = logs.len(),
                            "Fetched logs for program"
                        );
                        all_logs.extend(logs);
                    }
                }
                Err(e) => {
                    error!(
                        program = program,
                        slot = slot_number,
                        error = %e,
                        "Failed to fetch logs for program"
                    );
                    return Err(anyhow::anyhow!(
                        "Failed to fetch logs for program {}: {}",
                        program,
                        e
                    ));
                }
            }
        }

        info!(
            slot = slot_number,
            log_count = all_logs.len(),
            "All logs fetched for programs"
        );

        Ok(all_logs)
    }

    async fn reconcile_reorg(&self, start_slot: &SolanaSlot) -> Result<()> {
        let orphaned_slots = self
            .find_orphaned_slots(start_slot, self.config.max_reorg_depth)
            .await
            .context("Failed to find orphaned slots")?;

        if orphaned_slots.is_empty() {
            anyhow::bail!("No orphaned slots found");
        }

        for orphaned_slot in orphaned_slots {
            self.slot_handler.handle_reorg_slot(orphaned_slot.slot).await;

            if let Err(e) = self
                .store
                .delete_slot(orphaned_slot.chain_id, orphaned_slot.slot)
                .await
            {
                if !matches!(e, PersistenceError::NotFound) {
                    return Err(anyhow::anyhow!(
                        "Failed to delete orphaned slot: {}",
                        e
                    ));
                }
            }
        }

        Ok(())
    }

    async fn find_orphaned_slots(
        &self,
        start_slot: &SolanaSlot,
        max_depth: usize,
    ) -> Result<Vec<SlotRecord>> {
        let mut orphaned_slots = Vec::new();
        let start_slot_number = start_slot.slot;

        for parent_slot_num in
            (start_slot_number.saturating_sub(max_depth as u64)..start_slot_number).rev()
        {
            if parent_slot_num == 0 {
                break;
            }

            let canon_parent_slot = self
                .client
                .get_slot_by_number(parent_slot_num)
                .await
                .context(format!("Failed to fetch slot {} from chain", parent_slot_num))?;

            let parent_slot_record = match self
                .store
                .get_slot(self.config.chain_id, parent_slot_num)
                .await
            {
                Ok(Some(record)) => record,
                Ok(None) => {
                    debug!(
                        slot_number = parent_slot_num,
                        "Slot not found in storage"
                    );
                    let record = SlotRecord {
                        slot: canon_parent_slot.slot,
                        blockhash: canon_parent_slot.blockhash.clone(),
                        parent: canon_parent_slot.parent.unwrap_or(0),
                        block_time: canon_parent_slot.block_time.unwrap_or(0) as u64,
                        chain_id: self.config.chain_id,
                    };
                    if let Err(e) = self.store.save_slot(&record).await {
                        warn!(
                            slot_number = parent_slot_num,
                            error = %e,
                            "Failed to save missing slot to storage"
                        );
                    }
                    record
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to fetch slot {}: {}",
                        parent_slot_num,
                        e
                    ));
                }
            };

            if canon_parent_slot.blockhash != parent_slot_record.blockhash {
                info!(
                    slot_number = parent_slot_num,
                    stored_blockhash = parent_slot_record.blockhash,
                    canon_blockhash = canon_parent_slot.blockhash,
                    search_depth = start_slot_number - parent_slot_num,
                    "Found orphaned slot"
                );

                orphaned_slots.push(parent_slot_record);
                continue;
            }

            info!(
                slot_number = parent_slot_num,
                stored_blockhash = parent_slot_record.blockhash,
                canon_blockhash = canon_parent_slot.blockhash,
                "Slot hash match, stopping reorg ancestry search"
            );

            self.store
                .save_slot(&parent_slot_record)
                .await
                .context("Failed to save parent slot")?;

            return Ok(orphaned_slots);
        }

        warn!("Reached max reorg search depth");
        Ok(orphaned_slots)
    }
}

