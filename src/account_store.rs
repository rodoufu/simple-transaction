use crate::{ClientId, Transaction, account::Account};
use anyhow::{Context, Result};
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct AccountStore {
    accounts: HashMap<ClientId, Account>,
}

impl AccountStore {
    pub fn apply(&mut self, transaction: Transaction) -> Result<()> {
        match transaction {
            Transaction::Deposit {
                client,
                transaction_id,
                amount,
            } => {
                if amount == 0 {
                    return Ok(());
                }
                let account = self.accounts.entry(client).or_insert(Account::new(client));
                account.deposit(transaction_id, amount)?;
            }
            Transaction::Withdrawal {
                client,
                transaction_id,
                amount,
            } => {
                if amount == 0 {
                    return Ok(());
                }
                let account = self
                    .accounts
                    .get_mut(&client)
                    .context("account not found for withdrawal")?;
                account.withdrawal(transaction_id, amount)?;
            }
            Transaction::Dispute(dispute) => {
                let account = self
                    .accounts
                    .get_mut(&dispute.client)
                    .context("account not found dispute")?;
                account.start_dispute(dispute.transaction_id)?;
            }
            Transaction::Resolve(resolve) => {
                let account = self
                    .accounts
                    .get_mut(&resolve.client)
                    .context("account not found for dispute resolve")?;
                account.resolve_dispute(resolve.transaction_id)?;
            }
            Transaction::Chargeback(chargeback) => {
                let account = self
                    .accounts
                    .get_mut(&chargeback.client)
                    .context("account not found for chargeback")?;
                account.chargeback(chargeback.transaction_id)?;
            }
        }
        Ok(())
    }

    pub fn write<W: std::io::Write>(&self, writer: W) -> Result<()> {
        let mut wtr = csv::Writer::from_writer(writer);

        wtr.write_record(["client", "available", "held", "total", "locked"])
            .context("writing header")?;

        for account in self.accounts.values() {
            wtr.write_record([
                account.id.to_string(),
                account.available().to_string(),
                account.held().to_string(),
                account.total().to_string(),
                account.locked().to_string(),
            ])
            .context("writing account")?;
        }

        wtr.flush().context("flushing data")?;
        Ok(())
    }
}
