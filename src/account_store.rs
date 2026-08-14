use crate::{ClientId, Transaction, account::Account};
use anyhow::{Context, Result};
use std::sync::{RwLock, RwLockWriteGuard};

/// Slice of `Account`s.
type Shard = Box<[Option<Account>]>;
/// Lock of a `Shard`.
type ShardLock = RwLock<Shard>;

/// Holds the write lock for the shard last accessed, reusing it across consecutive
/// transactions that target the same shard instead of re-locking every time.
struct ShardLocker<'a> {
    shards: &'a [ShardLock],
    current: Option<(usize, RwLockWriteGuard<'a, Shard>)>,
}

impl<'a> ShardLocker<'a> {
    fn new(shards: &'a [ShardLock]) -> Self {
        Self {
            shards,
            current: None,
        }
    }

    fn get_write(&mut self, shard_number: usize) -> &mut Shard {
        let needs_new_lock = !matches!(&self.current, Some((previous_shard_number, _)) if *previous_shard_number == shard_number);
        if needs_new_lock {
            let shard_lock = self
                .shards
                .get(shard_number)
                .expect("invalid shard number should not happen")
                .write()
                .expect("poisoned lock");
            self.current = Some((shard_number, shard_lock));
        }
        &mut self.current.as_mut().expect("lock should be present").1
    }
}

/// Stores the account information.
/// Since `ClientId` is a `u16` the max number of clients is `65536` so it is reasanable to keep it
/// all in memory, avoiding the time of processing a hash function and the indirections of a
/// `HashMap`.
/// `AccountStore` is `Send` and `Sync` this way a reference to it can be shared accross distinct
/// threads in case that is necessary.
#[derive(Debug)]
pub struct AccountStore {
    shard_bits: u32,
    shard_size: usize,
    /// Shards used for the accounts.
    shards: Box<[ShardLock]>,
}

impl Default for AccountStore {
    fn default() -> Self {
        Self::new(128)
    }
}

impl AccountStore {
    /// Number of bits used by the shard.
    fn shard_bits(number_of_shards: usize) -> u32 {
        (ClientId::MAX as usize + 1).trailing_zeros() - number_of_shards.trailing_zeros()
    }

    /// Number of elements in a shard.
    fn shard_size(shard_bits: u32) -> usize {
        1 << shard_bits
    }

    /// Creates an `AccountStore` for the specified `number_of_shards`.
    /// `number_of_shards` needs to be a power of 2, if not it uses the magic number 128 as default.
    pub fn new(number_of_shards: ClientId) -> Self {
        let number_of_shards = if number_of_shards.is_power_of_two() {
            number_of_shards
        } else {
            128
        } as usize;
        let shard_bits = Self::shard_bits(number_of_shards);
        let shard_size = Self::shard_size(shard_bits);
        Self {
            shard_bits,
            shard_size,
            shards: (0..number_of_shards)
                .map(|_| RwLock::new(vec![None; shard_size].into_boxed_slice()))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    fn locate_shard_index(&self, client_id: ClientId) -> (usize, usize) {
        let client_id = client_id as usize;
        (
            client_id >> self.shard_bits,
            client_id & (self.shard_size - 1),
        )
    }

    /// Processing all transactions from an iterator so it can keep the lock for transactions that
    /// fall into the same shard and avoid releasing and acquiring the lock again on every transaction.
    /// All the transactions are processed, once an error is found the counter for that specific
    /// error is incremented, that is done like that so an error found on the path does not require
    /// the whole process to pause so it can resume later to keep processing the next one.
    pub fn apply_transactions<T: IntoIterator<Item = Transaction>>(
        &self,
        transactions: T,
    ) -> Result<()> {
        let mut shard_locker = ShardLocker::new(&self.shards);
        let mut account_not_found = 0;
        let mut not_enough_balance = 0;
        let mut transaction_not_found = 0;

        for transaction in transactions {
            match transaction {
                Transaction::Deposit {
                    client,
                    transaction_id,
                    amount,
                } => {
                    if amount == 0 {
                        continue;
                    }
                    let (shard_number, index_within_shard) = self.locate_shard_index(client);
                    let account = shard_locker
                        .get_write(shard_number)
                        .get_mut(index_within_shard)
                        .expect("id not found in shard");

                    match account {
                        Some(account) => {
                            let _ = account
                                .deposit(transaction_id, amount)
                                .inspect_err(|_| not_enough_balance += 1);
                        }
                        None => {
                            let mut new_account = Account::new(client);
                            let _ = new_account
                                .deposit(transaction_id, amount)
                                .inspect_err(|_| not_enough_balance += 1);
                            *account = Some(new_account);
                        }
                    }
                }
                Transaction::Withdrawal {
                    client,
                    transaction_id,
                    amount,
                } => {
                    if amount == 0 {
                        continue;
                    }

                    let (shard_number, index_within_shard) = self.locate_shard_index(client);
                    let Some(account) = shard_locker
                        .get_write(shard_number)
                        .get_mut(index_within_shard)
                        .expect("id not found in shard")
                        .as_mut()
                    else {
                        account_not_found += 1;
                        continue;
                    };
                    let _ = account
                        .withdrawal(transaction_id, amount)
                        .inspect_err(|_| not_enough_balance += 1);
                }
                Transaction::Dispute(dispute) => {
                    let (shard_number, index_within_shard) =
                        self.locate_shard_index(dispute.client);
                    let Some(account) = shard_locker
                        .get_write(shard_number)
                        .get_mut(index_within_shard)
                        .expect("id not found in shard")
                        .as_mut()
                    else {
                        account_not_found += 1;
                        continue;
                    };
                    let _ = account
                        .start_dispute(dispute.transaction_id)
                        .inspect_err(|_| transaction_not_found += 1);
                }
                Transaction::Resolve(resolve) => {
                    let (shard_number, index_within_shard) =
                        self.locate_shard_index(resolve.client);
                    let Some(account) = shard_locker
                        .get_write(shard_number)
                        .get_mut(index_within_shard)
                        .expect("id not found in shard")
                        .as_mut()
                    else {
                        account_not_found += 1;
                        continue;
                    };
                    let _ = account
                        .resolve_dispute(resolve.transaction_id)
                        .inspect_err(|_| transaction_not_found += 1);
                }
                Transaction::Chargeback(chargeback) => {
                    let (shard_number, index_within_shard) =
                        self.locate_shard_index(chargeback.client);
                    let Some(account) = shard_locker
                        .get_write(shard_number)
                        .get_mut(index_within_shard)
                        .expect("id not found in shard")
                        .as_mut()
                    else {
                        account_not_found += 1;
                        continue;
                    };
                    let _ = account
                        .chargeback(chargeback.transaction_id)
                        .inspect_err(|_| transaction_not_found += 1);
                }
            }
        }
        anyhow::ensure!(
            account_not_found == 0 && not_enough_balance == 0 && transaction_not_found == 0,
            "errors processing transactions account_not_found:{account_not_found}, not_enough_balance:{not_enough_balance}, transaction_not_found:{transaction_not_found}"
        );

        Ok(())
    }

    /// Writes the accounts current state as a CSV to the specified writer.
    pub fn write<W: std::io::Write>(&self, writer: W) -> Result<()> {
        let mut wtr = csv::Writer::from_writer(writer);
        wtr.write_record(["client", "available", "held", "total", "locked"])
            .context("writing header")?;

        for shard in self.shards.as_ref() {
            for account in shard
                .read()
                .expect("poisoned lock")
                .as_ref()
                .iter()
                .flatten()
            {
                wtr.write_record([
                    account.id.to_string(),
                    account.available().to_string(),
                    account.held().to_string(),
                    account.total().to_string(),
                    account.locked().to_string(),
                ])
                .context("writing account")?;
            }
        }

        wtr.flush().context("flushing data")?;
        Ok(())
    }
}
