use crate::config::ChainId;
use async_trait::async_trait;

#[async_trait]
pub trait ContractStore: Send + Sync {
    async fn get_contract_by_address(
        &self,
        address: &str,
    ) -> anyhow::Result<Option<Contract>>;

    async fn get_contract_by_name_for_chain_id(
        &self,
        name: &str,
        chain_id: ChainId,
    ) -> anyhow::Result<Option<Contract>>;

    async fn list_contract_addresses_for_chain(&self, chain_id: ChainId) -> Vec<String>;

    async fn list_contracts(&self) -> Vec<Contract>;
}

#[derive(Debug, Clone)]
pub struct Contract {
    pub name: String,
    pub address: String,
    pub chain_id: ChainId,
}

pub struct InMemoryContractStore {
    contracts: Vec<Contract>,
}

impl InMemoryContractStore {
    pub fn new(contracts: Vec<Contract>) -> Self {
        Self { contracts }
    }
}

#[async_trait]
impl ContractStore for InMemoryContractStore {
    async fn get_contract_by_address(
        &self,
        address: &str,
    ) -> anyhow::Result<Option<Contract>> {
        let address_lower = address.to_lowercase();
        Ok(self
            .contracts
            .iter()
            .find(|c| c.address.to_lowercase() == address_lower)
            .cloned())
    }

    async fn get_contract_by_name_for_chain_id(
        &self,
        name: &str,
        chain_id: ChainId,
    ) -> anyhow::Result<Option<Contract>> {
        Ok(self
            .contracts
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name) && c.chain_id == chain_id)
            .cloned())
    }

    async fn list_contract_addresses_for_chain(&self, chain_id: ChainId) -> Vec<String> {
        self.contracts
            .iter()
            .filter(|c| c.chain_id == chain_id)
            .map(|c| c.address.clone())
            .collect()
    }

    async fn list_contracts(&self) -> Vec<Contract> {
        self.contracts.clone()
    }
}

