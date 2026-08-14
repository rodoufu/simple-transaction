# simple-transaction

A simple payments engine: it reads a series of deposit, withdrawal, dispute, resolve and
chargeback transactions from a CSV, updates client account balances accordingly, and
writes the resulting per-client account state back out as a CSV.

## Usage

```sh
cargo run -- transactions.csv > accounts.csv
```

The input file is the only argument.
Output is written to stdout; logs (record counts, parse/apply errors, timing) go to stderr
and can be tuned with `RUST_LOG` (e.g. `RUST_LOG=info`).

### Input

A CSV with columns `type, client, tx, amount`:

```csv
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
withdrawal, 1, 3, 1.5
dispute, 1, 1,
```

Columns:
- `type` is one of `deposit`, `withdrawal`, `dispute`, `resolve`, `chargeback`.
- `client` is a `u16` identifing the client account
- `tx` is a `u32`. `tx` is globally unique across the whole file
- `amount` is present (up to 4 decimal places) for deposits/withdrawals, and absent for
  dispute/resolve/chargeback, which instead reference a previous `tx` by id.
- Whitespace around fields, and rows with or without a trailing empty `amount` column, are both accepted.

### Output

```csv
client, available, held, total, locked
1, 1.5, 0.0, 1.5, false
2, 2.0, 0.0, 2.0, false
```

Columns:
- `client` is a `u16` identifing the client account
- `available = total - held`
- `held` is funds frozen by an open dispute
- `total = available + held`,
- `locked` becomes `true` once an account has had a chargeback applied to it.

## Design decisions

### Fixed-point balances

Balances are stored internally as `u64` "minor units" (amount x 10,000), not `f64`,
specifically to avoid floating-point representation error accumulating across many
deposits/withdrawals (e.g. `0.1` has no exact `f64` representation), also operations on integers are usually faster.
Amounts are converted once, at the CSV boundary, with `+ 0.5` before truncating to an integer so the
conversion rounds to the nearest minor unit instead of always truncating down (
this matters: `0.57 * 10_000.0` evaluates to `5699.999999999999` in `f64`,
which would otherwise truncate to `5699` instead of `5700`).

### Balances never overflow/underflow silently

Every mutation to `total`/`held` goes through `checked_add`/`checked_sub` and a matching
precondition check (e.g. a dispute can only hold up to what's currently available; a
withdrawal can only draw down what's currently available, not what's already held for a
separate dispute).

### Errors are per-record, not fatal

A malformed CSV row, a dispute referencing an unknown transaction, or a withdrawal with
insufficient funds are all expected, not reasons to abort the whole run.
`main.rs` counts and logs these (`parsing_line_error`, `convert_error`,
and per-transaction-type counters from `apply_transactions`) and keeps going;
the CSV output always reflects everything that *could* be applied.
Only unrecoverable conditions (bad CLI args, an unreadable input file) stop the program before it produces output.

### Built to be usable from multiple threads, even though nothing here needs that yet

The CLI entry point is single-threaded, one file in, one `apply_transactions` call, one
CSV out, so thread-safety is not a functional requirement for this program.
But the spec explicitly raises the question of what happens if this were bundled into a
server handling many concurrent transaction streams, so `AccountStore` was deliberately
designed to make that a non-issue rather than a rewrite:

- Client accounts are partitioned into 128 shards by client id, each behind its own `RwLock`.
  `AccountStore` only needs a `&self` (not `&mut self`) to apply transactions,
  so an `Arc<AccountStore>` can be cloned across threads/tasks and have
  `apply_transactions` called concurrently on it, each thread only ever contends with
  others touching the *same* shard, not a single global lock.
- `apply_transactions` takes an iterator, and reuses the write lock for a shard across
  consecutive transactions that land in it (see `ShardLocker`) rather than
  releasing/re-acquiring on every single transaction, a small win when a batch happens
  to be grouped by client or different clients in the same shard,
  and no worse than a per-transaction lock otherwise.

Initially I considered using a `HashMap<ClientId, Account>` for the (maybe `dashmap` for concurrency),
but since the `ClientId` was small enough I decided to avoid the indirections of a `HashMap` and the hashing function,
since those could dominate the CPU time with very high load.

### CSV parsing matches the spec's documented format exactly

`ReaderBuilder` is configured with `.trim(csv::Trim::All)` and `.flexible(true)`.
Without `flexible(true)`, rows for dispute/resolve/chargeback — which the spec's own examples
write with no trailing comma for the omitted `amount` field — fail to parse against a
4-column header, since `csv` otherwise requires every row to match the header's field count exactly.

## Performance

Measured on a 1,000,000-row input (`cargo run --release`):

- The number of 128 shards was intuitively choosen, for a real production environment
  tests and comparisions would be required to be able to select the best suited value.
- ~340ms end to end (parse + convert + apply), ~56MB peak.
- The per-account transaction lookup (`HashMap<TransactionId, AccountTransaction>`, used
  to look up a transaction by id when a dispute/resolve/chargeback references it) uses
  `rustc_hash::FxHashMap` instead of the standard library's default Hash. 
  Measured on 1M inserts in isolation: ~52ms with the default hasher vs. ~26ms with `FxHashMap`.
- `benches/apply_transactions.rs` (Criterion) and the `same_shard`/`distinct_shards`
  examples exist specifically to measure the shard-locking design's actual cost: applying
  20,000 deposits that all land in one shard (so the lock is acquired once and reused)
  takes ~58ms; the same 20,000 deposits spread evenly across all 128 shards (so the
  lock is released and re-acquired on every transaction) takes \~65ms.
  That ~11% gap is the real, measured price of keeping the store thread-ready in a workload that never
  actually contends — run `cargo run --release --example same_shard` /
  `--example distinct_shards`, or `cargo bench`, to reproduce.
- Memory at this scale is dominated by retained transaction history (every account keeps
  every transaction it has ever seen, so a dispute can reference any of them at any
  point), not by the fixed shard layout (`Account` is 72 bytes; the full 65,536-slot
  shard array is a one-time ~4.5MB allocation).
  This is inherent to the requirement that disputes can reference arbitrarily old transactions, not something to optimize away.
  Maybe a time to live for the transactions, or removing them once a dispute is finalized.

Performance possible improvements:

- Currently the `AccountStore` holds a slice of shards, where each shard is a slice of accounts.
  That is basically an Array of Strucutres (AoS), considering the possibility of multiple transactions affecting more often
  only the total (deposits/withdrawals), a possible optimization would be to try using a Struct of Arrays (SoA),
  which can improve cache hits, increase performance with better usage of prefetching.
- For scenarios where we do not have too many transactions by client one could test using a 
  `transactions: SmallVec<[AccountTransaction; SelectedSize]>` instead of `transactions: FxHashMap<TransactionId, AccountTransaction>`,
  that would give a stack allocated vector, less memory indirections, and possibly more cache friendly,
  but would require to iterate over all values to find transactions.

   

## Testing

Unit tests use a table-driven pattern (a `Case` struct + an array of cases run through a
shared assertion loop) and live next to the code they cover:
`account_tests.rs`, `account_store_tests.rs`, `csv_tests.rs`, `lib_tests.rs`.
They cover the deposit/withdrawal/dispute/resolve/chargeback lifecycle, CSV parsing edge cases
(whitespace, missing trailing fields), amount-to-minor-units rounding, and the
account-store-level corner cases (unknown account/transaction, insufficient funds, zero
amounts, duplicate transaction ids, the `held`/`total` invariant regression above).

Run with `cargo test`.
Run the benchmark suite with `cargo bench` (HTML report under `target/criterion/`).

## Assumptions

- **Duplicate transaction ids.** The spec guarantees `tx` is globally unique, so this
  should never happen on well-formed input; if it does, the second transaction reusing an
  id is rejected outright rather than silently overwriting the first (which would
  otherwise orphan any dispute already tracked against it).
- **Zero-amount deposits/withdrawals** are accepted but treated as a no-op (no balance
  change, no transaction recorded), rather than an error.
