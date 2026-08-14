use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use simple_transaction::{ClientId, Transaction, TransactionId, account_store::AccountStore};

const TRANSACTION_COUNT: usize = 20_000;

/// Mirrors the shard layout of `AccountStore::default()` (128 shards spread evenly
/// across the full `ClientId` range), so the client ids picked below land in a known
/// shard.
const NUM_SHARDS: usize = 128;
const SHARD_SIZE: ClientId = ((ClientId::MAX as usize + 1) / NUM_SHARDS) as ClientId;

/// A handful of client ids that all fall inside shard 0, so every transaction reuses
/// the shard lock acquired by the previous one.
fn same_shard_transactions(count: usize) -> Vec<Transaction> {
    (0..count)
        .map(|i| Transaction::deposit((i % 16) as ClientId, i as TransactionId, 10.0))
        .collect()
}

/// Client ids spaced a full shard apart, cycling through every shard, so each
/// transaction forces the locker to release the current shard lock and acquire the
/// next one.
fn distinct_shard_transactions(count: usize) -> Vec<Transaction> {
    (0..count)
        .map(|i| {
            let client = (i % NUM_SHARDS) as ClientId * SHARD_SIZE;
            Transaction::deposit(client, i as TransactionId, 10.0)
        })
        .collect()
}

fn bench_same_shard(c: &mut Criterion) {
    c.bench_function("apply_transactions/same_shard", |b| {
        b.iter_batched(
            || {
                (
                    AccountStore::default(),
                    same_shard_transactions(TRANSACTION_COUNT),
                )
            },
            |(store, transactions)| {
                black_box(store.apply_transactions(black_box(transactions))).ok();
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_distinct_shards(c: &mut Criterion) {
    c.bench_function("apply_transactions/distinct_shards", |b| {
        b.iter_batched(
            || {
                (
                    AccountStore::default(),
                    distinct_shard_transactions(TRANSACTION_COUNT),
                )
            },
            |(store, transactions)| {
                black_box(store.apply_transactions(black_box(transactions))).ok();
            },
            BatchSize::LargeInput,
        );
    });
}

criterion_group!(benches, bench_same_shard, bench_distinct_shards);
criterion_main!(benches);
