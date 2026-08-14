//! Applies a batch of transactions whose client ids cycle through every shard, so
//! `apply_transactions` has to release and re-acquire the shard lock on every single
//! transaction. Run with:
//!
//! cargo run --release --example distinct_shards

use std::time::Instant;

use anyhow::Result;
use simple_transaction::{ClientId, Transaction, TransactionId, account_store::AccountStore};

const TRANSACTION_COUNT: usize = 1_000_000;

/// Mirrors the shard layout of `AccountStore::default()` (128 shards spread evenly
/// across the full `ClientId` range), so the client ids picked below land in a known
/// shard.
const NUM_SHARDS: usize = 128;
const SHARD_SIZE: ClientId = ((ClientId::MAX as usize + 1) / NUM_SHARDS) as ClientId;

fn main() -> Result<()> {
    let transactions = (0..TRANSACTION_COUNT)
        .map(|i| {
            let client = (i % NUM_SHARDS) as ClientId * SHARD_SIZE;
            Transaction::deposit(client, i as TransactionId, 10.0)
        })
        .collect::<Vec<_>>();

    let store = AccountStore::default();
    let start = Instant::now();
    store.apply_transactions(transactions)?;
    println!(
        "distinct shards: applied {TRANSACTION_COUNT} transactions across {NUM_SHARDS} shards in {:?}",
        start.elapsed()
    );

    Ok(())
}
