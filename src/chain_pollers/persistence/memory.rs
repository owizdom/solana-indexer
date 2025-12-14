use crate::chain_pollers::persistence::*;
use crate::config::ChainId;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct InMemoryChainPollerPersistence {
    last_processed_slots: Arc<DashMap<String, u64>>,
    slots: Arc<DashMap<String, SlotRecord>>,
    closed: Arc<RwLock<bool>>,
}

impl InMemoryChainPollerPersistence {
    pub fn new() -> Self {
        Self {
            last_processed_slots: Arc::new(DashMap::new()),
            slots: Arc::new(DashMap::new()),
            closed: Arc::new(RwLock::new(false)),
        }
    }

    fn make_slot_key(chain_id: ChainId) -> String {
        format!("{}", chain_id)
    }

    fn make_slot_record_key(chain_id: ChainId, slot_number: u64) -> String {
        format!("slot:{}:{}", chain_id, slot_number)
    }
}

#[async_trait::async_trait]
impl ChainPollerPersistence for InMemoryChainPollerPersistence {
    async fn get_last_processed_slot(
        &self,
        chain_id: ChainId,
    ) -> Result<Option<SlotRecord>, PersistenceError> {
        let closed = *self.closed.read().await;
        if closed {
            return Err(PersistenceError::StoreClosed);
        }

        let key = Self::make_slot_key(chain_id);
        let slot_num = match self.last_processed_slots.get(&key) {
            Some(v) => *v.value(),
            None => return Ok(None),
        };

        let slot_key = Self::make_slot_record_key(chain_id, slot_num);
        match self.slots.get(&slot_key) {
            Some(v) => Ok(Some(v.value().clone())),
            None => Ok(None),
        }
    }

    async fn save_slot(&self, slot: &SlotRecord) -> Result<(), PersistenceError> {
        let closed = *self.closed.read().await;
        if closed {
            return Err(PersistenceError::StoreClosed);
        }

        let slot_key = Self::make_slot_record_key(slot.chain_id, slot.slot);
        self.slots.insert(slot_key.clone(), slot.clone());

        let key = Self::make_slot_key(slot.chain_id);
        self.last_processed_slots.insert(key, slot.slot);

        Ok(())
    }

    async fn get_slot(
        &self,
        chain_id: ChainId,
        slot_number: u64,
    ) -> Result<Option<SlotRecord>, PersistenceError> {
        let closed = *self.closed.read().await;
        if closed {
            return Err(PersistenceError::StoreClosed);
        }

        let key = Self::make_slot_record_key(chain_id, slot_number);
        match self.slots.get(&key) {
            Some(v) => Ok(Some(v.value().clone())),
            None => Ok(None),
        }
    }

    async fn delete_slot(
        &self,
        chain_id: ChainId,
        slot_number: u64,
    ) -> Result<(), PersistenceError> {
        let closed = *self.closed.read().await;
        if closed {
            return Err(PersistenceError::StoreClosed);
        }

        let key = Self::make_slot_record_key(chain_id, slot_number);
        if self.slots.remove(&key).is_none() {
            return Err(PersistenceError::NotFound);
        }

        Ok(())
    }

    async fn close(&self) -> Result<(), PersistenceError> {
        let mut closed = self.closed.write().await;
        if *closed {
            return Err(PersistenceError::StoreClosed);
        }

        *closed = true;
        self.last_processed_slots.clear();
        self.slots.clear();

        Ok(())
    }
}

impl Default for InMemoryChainPollerPersistence {
    fn default() -> Self {
        Self::new()
    }
}

