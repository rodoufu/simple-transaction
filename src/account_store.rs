use crate::{ClientId, Transaction, account::Account};
use anyhow::{Context, Result};
use std::sync::RwLock;

const NUM_SHARDS: usize = 128;
const SHARD_BITS: u32 = (ClientId::MAX as usize + 1).trailing_zeros() - NUM_SHARDS.trailing_zeros();
const SHARD_SIZE: usize = 1 << SHARD_BITS;

#[derive(Debug)]
pub struct AccountStore {
    shards: Box<[RwLock<Box<[Option<Account>]>>]>,
}

impl Default for AccountStore {
    fn default() -> Self {
        Self {
            shards: (0..NUM_SHARDS)
                .map(|_| RwLock::new(vec![None; SHARD_SIZE].into_boxed_slice()))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }
}

impl AccountStore {
    fn locate_shard_index(client_id: ClientId) -> (usize, usize) {
        let client_id = client_id as usize;
        (client_id >> SHARD_BITS, client_id & (SHARD_SIZE - 1))
    }

    pub fn apply_transactions<T: IntoIterator<Item = Transaction>>(
        &self,
        transactions: T,
    ) -> Result<()> {
        // Keep the previous lock
        let mut shard_and_lock = None;
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
                        return Ok(());
                    }
                    let (shard, id) = Self::locate_shard_index(client);
                    match shard_and_lock {
                        None => {
                            let shard_lock = self
                                .shards
                                .get(shard)
                                .expect("invalid shard should not happen")
                                .write()
                                .expect("poisoned lock");
                            shard_and_lock = Some((shard, shard_lock));
                        }
                        Some((previous_shard, _)) if previous_shard != shard => {
                            let shard_lock = self
                                .shards
                                .get(shard)
                                .expect("invalid shard should not happen")
                                .write()
                                .expect("poisoned lock");
                            shard_and_lock = Some((shard, shard_lock));
                        }
                        Some(_) => {}
                    }

                    let (_, shard) = shard_and_lock.as_mut().expect("lock should be present");
                    let account = shard.get_mut(id).expect("id not found in shard");
                    match account {
                        Some(account) => {
                            account.deposit(transaction_id, amount);
                        }
                        None => {
                            let mut new_account = Account::new(client);
                            new_account.deposit(transaction_id, amount);
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
                        return Ok(());
                    }

                    let (shard, id) = Self::locate_shard_index(client);
                    match shard_and_lock {
                        None => {
                            let shard_lock = self
                                .shards
                                .get(shard)
                                .expect("invalid shard should not happen")
                                .write()
                                .expect("poisoned lock");
                            shard_and_lock = Some((shard, shard_lock));
                        }
                        Some((previous_shard, _)) if previous_shard != shard => {
                            let shard_lock = self
                                .shards
                                .get(shard)
                                .expect("invalid shard should not happen")
                                .write()
                                .expect("poisoned lock");
                            shard_and_lock = Some((shard, shard_lock));
                        }
                        Some(_) => {}
                    }

                    let (_, shard) = shard_and_lock.as_mut().expect("lock should be present");
                    let Some(account) = shard.get_mut(id).expect("id not found in shard").as_mut()
                    else {
                        account_not_found += 1;
                        continue;
                    };
                    let _ = account
                        .withdrawal(transaction_id, amount)
                        .inspect_err(|_| not_enough_balance += 1);
                }
                Transaction::Dispute(dispute) => {
                    let (shard, id) = Self::locate_shard_index(dispute.client);
                    match shard_and_lock {
                        None => {
                            let shard_lock = self
                                .shards
                                .get(shard)
                                .expect("invalid shard should not happen")
                                .write()
                                .expect("poisoned lock");
                            shard_and_lock = Some((shard, shard_lock));
                        }
                        Some((previous_shard, _)) if previous_shard != shard => {
                            let shard_lock = self
                                .shards
                                .get(shard)
                                .expect("invalid shard should not happen")
                                .write()
                                .expect("poisoned lock");
                            shard_and_lock = Some((shard, shard_lock));
                        }
                        Some(_) => {}
                    }

                    let (_, shard) = shard_and_lock.as_mut().expect("lock should be present");
                    let Some(account) = shard.get_mut(id).expect("id not found in shard").as_mut()
                    else {
                        account_not_found += 1;
                        continue;
                    };
                    let _ = account
                        .start_dispute(dispute.transaction_id)
                        .inspect_err(|_| transaction_not_found += 1);
                }
                Transaction::Resolve(resolve) => {
                    let (shard, id) = Self::locate_shard_index(resolve.client);
                    match shard_and_lock {
                        None => {
                            let shard_lock = self
                                .shards
                                .get(shard)
                                .expect("invalid shard should not happen")
                                .write()
                                .expect("poisoned lock");
                            shard_and_lock = Some((shard, shard_lock));
                        }
                        Some((previous_shard, _)) if previous_shard != shard => {
                            let shard_lock = self
                                .shards
                                .get(shard)
                                .expect("invalid shard should not happen")
                                .write()
                                .expect("poisoned lock");
                            shard_and_lock = Some((shard, shard_lock));
                        }
                        Some(_) => {}
                    }

                    let (_, shard) = shard_and_lock.as_mut().expect("lock should be present");
                    let Some(account) = shard.get_mut(id).expect("id not found in shard").as_mut()
                    else {
                        account_not_found += 1;
                        continue;
                    };
                    let _ = account
                        .resolve_dispute(resolve.transaction_id)
                        .inspect_err(|_| transaction_not_found += 1);
                }
                Transaction::Chargeback(chargeback) => {
                    let (shard, id) = Self::locate_shard_index(chargeback.client);
                    match shard_and_lock {
                        None => {
                            let shard_lock = self
                                .shards
                                .get(shard)
                                .expect("invalid shard should not happen")
                                .write()
                                .expect("poisoned lock");
                            shard_and_lock = Some((shard, shard_lock));
                        }
                        Some((previous_shard, _)) if previous_shard != shard => {
                            let shard_lock = self
                                .shards
                                .get(shard)
                                .expect("invalid shard should not happen")
                                .write()
                                .expect("poisoned lock");
                            shard_and_lock = Some((shard, shard_lock));
                        }
                        Some(_) => {}
                    }

                    let (_, shard) = shard_and_lock.as_mut().expect("lock should be present");
                    let Some(account) = shard.get_mut(id).expect("id not found in shard").as_mut()
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

    pub fn write<W: std::io::Write>(&self, writer: W) -> Result<()> {
        let mut wtr = csv::Writer::from_writer(writer);
        wtr.write_record(["client", "available", "held", "total", "locked"])
            .context("writing header")?;

        for shard in self.shards.as_ref() {
            for account in shard.read().expect("poisoned lock").as_ref() {
                if let Some(account) = account {
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
        }

        wtr.flush().context("flushing data")?;
        Ok(())
    }
}
