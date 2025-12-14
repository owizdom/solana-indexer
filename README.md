# solana-chain-indexer

A Rust library for indexing and monitoring Solana blockchain events. solana-chain-indexer provides infrastructure for polling slots, detecting reorganizations, and parsing smart program logs.

## Features

- **Blockchain Polling**: Configurable polling intervals for monitoring new slots on Solana
- **Reorg Detection**: Automatic detection and reconciliation of blockchain reorganizations
- **Event Parsing**: Decode program logs using program IDLs with support for multiple IDL versions
- **Batch Processing**: Efficient RPC batching with automatic chunking and retry logic
- **Pluggable Persistence**: Interface-based storage for slot tracking with in-memory implementation included
- **Concurrent Operations**: Thread-safe operations with support for monitoring multiple chains

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
solana-chain-indexer = { git = "https://github.com/owizdom/solana-chain-indexer" }
```

## Quick Start

```rust
use solana_chain_indexer::*;
use std::sync::Arc;
use std::time::Duration;

// Create a Solana client
let client_config = SolanaClientConfig {
    base_url: "https://your-rpc-endpoint".to_string(),
    block_commitment: BlockCommitment::Finalized,
};
let client = Arc::new(SolanaClient::new(client_config)?);

// Create persistence store
let store = Arc::new(InMemoryChainPollerPersistence::new());

// Implement SlotHandler to process slots and logs
struct MySlotHandler;

#[async_trait::async_trait]
impl SlotHandler for MySlotHandler {
    async fn handle_slot(&self, slot: &SolanaSlot) -> anyhow::Result<()> {
        // Process slot
        Ok(())
    }

    async fn handle_log(&self, log_with_slot: &LogWithSlot) -> anyhow::Result<()> {
        // Process decoded program log
        Ok(())
    }

    async fn handle_reorg_slot(&self, slot_number: u64) {
        // Handle reorg by invalidating data from this slot
    }
}

// Create and start the chain poller
let log_parser = Arc::new(TransactionLogParser::new());
let contract_store = Arc::new(InMemoryContractStore::new(vec![]));
let slot_handler = Arc::new(MySlotHandler);

let poller_config = SolanaChainPollerConfig {
    chain_id: 101, // Mainnet
    polling_interval: Duration::from_secs(12),
    interesting_programs: vec!["ProgramAddress".to_string()],
    max_reorg_depth: 10,
    slot_history_size: 100,
    reorg_check_enabled: true,
};

let poller = SolanaChainPoller::new(
    client,
    log_parser,
    poller_config,
    contract_store,
    store,
    slot_handler,
);

poller.start().await?;
```

## Development

### Prerequisites

- Rust 1.70+
- Cargo

### Setup

```bash
# Install dependencies
cargo build

# Run tests
cargo test

# Run linter
cargo clippy

# Format code
cargo fmt
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific package tests
cargo test --package solana-chain-indexer --lib chain_pollers::solana
```

## Architecture

The library follows a modular architecture:

- **Chain Pollers** (`src/chain_pollers/`): Main polling logic for Solana slots
- **Clients** (`src/clients/`): Solana RPC client implementation
- **Transaction Log Parser** (`src/transaction_log_parser/`): Program log decoding
- **Contract Store** (`src/contract_store/`): Program metadata management
- **Persistence** (`src/chain_pollers/persistence/`): Slot tracking storage

## Disclaimer

**🚧 solana-chain-indexer is under active development, and has not been audited.**

- Features may be added, removed, or modified
- Interfaces may have breaking changes
- Should be used **only for testing purposes** and **not in production**
- Provided "as is" without guarantee of functionality or production support

**Eigen Labs, Inc. does not provide support for production use.**

## Security

If you discover a vulnerability, please **do not** open an issue. Instead contact the maintainers directly at `security@eigenlabs.org`.

## License

MIT

