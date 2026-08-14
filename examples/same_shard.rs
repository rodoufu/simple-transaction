//! Applies a batch of transactions that all target client ids within a single shard,
//! so `apply_transactions` reuses the same shard lock for the whole run instead of
//! releasing and re-acquiring it. Run with:
//!
//! cargo run --release --example same_shard

use std::time::Instant;

use anyhow::Result;
use simple_transaction::{ClientId, Transaction, TransactionId, account_store::AccountStore};

const TRANSACTION_COUNT: usize = 1_000_000;
const CLIENT_COUNT: ClientId = 16;

fn main() -> Result<()> {
    let transactions = (0..TRANSACTION_COUNT)
        .map(|i| Transaction::deposit((i as ClientId) % CLIENT_COUNT, i as TransactionId, 10.0))
        .collect::<Vec<_>>();

    let store = AccountStore::default();
    let start = Instant::now();
    store.apply_transactions(transactions)?;
    println!(
        "same shard: applied {TRANSACTION_COUNT} transactions across {CLIENT_COUNT} clients (1 shard) in {:?}",
        start.elapsed()
    );

    Ok(())
}
