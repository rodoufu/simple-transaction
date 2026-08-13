use crate::{ClientId, TransactionId};
use anyhow::{Context, Result};

pub(super) const ACCOUNT_MULTIPLIER: f64 = 1e4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccountTransactionDisppute {
    #[default]
    None,
    DisputeInitiated,
    Resolved,
    ChargeBackOccurred,
}

#[derive(Debug, Clone, Copy)]
pub enum AccountTransaction {
    Deposit {
        id: TransactionId,
        amount: u64,
        dispute: AccountTransactionDisppute,
    },
    Withdrawal {
        id: TransactionId,
        amount: u64,
        dispute: AccountTransactionDisppute,
    },
}

impl AccountTransaction {
    pub(super) fn id(&self) -> TransactionId {
        match self {
            Self::Deposit { id, .. } => *id,
            Self::Withdrawal { id, .. } => *id,
        }
    }
}

/// Account represents the data for eache client.
/// Internally it uses `u64` instead of `f64` to avoid representations issues such as `0.1` not
/// being possible to exactly represent as a `f64`.
/// `ACCOUNT_MULTIPLIER` is used to convert the `f64` to `u64` using the expected precision.
#[derive(Debug, Clone)]
pub struct Account {
    // Client identification
    pub(super) id: ClientId,

    total: u64,
    held: u64,

    locked: bool,
    transactions: Vec<AccountTransaction>,
}

impl Account {
    pub(super) fn new(id: ClientId) -> Self {
        Self {
            id,
            total: Default::default(),
            held: Default::default(),
            locked: false,
            transactions: Default::default(),
        }
    }

    /// The total funds that are available for trading, staking, withdrawal, etc.
    /// This should be equal to the total - held amounts
    pub(crate) fn available(&self) -> f64 {
        (self.total - self.held) as f64 / ACCOUNT_MULTIPLIER
    }

    /// The total funds that are held for dispute.
    /// This should be equal to total - available amounts
    pub(crate) fn held(&self) -> f64 {
        self.held as f64 / ACCOUNT_MULTIPLIER
    }

    /// The total funds that are available or held.
    /// This should be equal to available + held
    pub(crate) fn total(&self) -> f64 {
        (self.total) as f64 / ACCOUNT_MULTIPLIER
    }

    /// Whether the account is locked.
    /// An account is locked if a charge back occur
    pub(crate) fn locked(&self) -> bool {
        self.locked
    }

    pub(crate) fn deposit(&mut self, id: TransactionId, amount: u64) {
        self.total += amount;
        self.transactions.push(AccountTransaction::Deposit {
            id,
            amount,
            dispute: AccountTransactionDisppute::None,
        });
    }

    pub(crate) fn withdrawal(&mut self, id: TransactionId, amount: u64) -> Result<()> {
        anyhow::ensure!(self.total >= amount, "not enough balance for withdrawal");
        self.total -= amount;
        self.transactions.push(AccountTransaction::Withdrawal {
            id,
            amount,
            dispute: AccountTransactionDisppute::None,
        });
        Ok(())
    }

    pub(crate) fn start_dispute(&mut self, transaction_id: TransactionId) -> Result<()> {
        let transaction = self
            .transactions
            .iter_mut()
            .find(|x| x.id() == transaction_id)
            .context("transaction not found for dispute")?;

        match transaction {
            AccountTransaction::Deposit {
                amount, dispute, ..
            } => {
                anyhow::ensure!(
                    matches!(dispute, AccountTransactionDisppute::None),
                    "there is already a dispute for transaction"
                );
                anyhow::ensure!(self.total >= *amount, "not enough balance for dispute");
                self.held += *amount;
                *dispute = AccountTransactionDisppute::DisputeInitiated;
            }
            AccountTransaction::Withdrawal {
                amount, dispute, ..
            } => {
                anyhow::ensure!(
                    matches!(dispute, AccountTransactionDisppute::None),
                    "there is already a dispute for transaction"
                );
                anyhow::ensure!(self.held >= *amount, "not enough held for dispute");
                self.held -= *amount;
                *dispute = AccountTransactionDisppute::DisputeInitiated;
            }
        }
        Ok(())
    }

    pub(crate) fn resolve_dispute(&mut self, transaction_id: TransactionId) -> Result<()> {
        let transaction = self
            .transactions
            .iter_mut()
            .find(|x| x.id() == transaction_id)
            .context("transaction not found to resolve dispute")?;

        match transaction {
            AccountTransaction::Deposit {
                amount, dispute, ..
            } => {
                anyhow::ensure!(
                    matches!(dispute, AccountTransactionDisppute::DisputeInitiated),
                    "there is not a dispute for transaction"
                );
                anyhow::ensure!(self.held >= *amount, "not enough balance for dispute");
                self.held -= *amount;
                *dispute = AccountTransactionDisppute::Resolved;
            }
            AccountTransaction::Withdrawal {
                amount, dispute, ..
            } => {
                anyhow::ensure!(
                    matches!(dispute, AccountTransactionDisppute::DisputeInitiated),
                    "there is not a dispute for transaction"
                );
                anyhow::ensure!(self.total >= *amount, "not enough balance for dispute");
                self.held += *amount;
                *dispute = AccountTransactionDisppute::Resolved;
            }
        }
        Ok(())
    }

    pub(crate) fn chargeback(&mut self, transaction_id: TransactionId) -> Result<()> {
        let transaction = self
            .transactions
            .iter_mut()
            .find(|x| x.id() == transaction_id)
            .context("transaction not found for chargeback")?;

        match transaction {
            AccountTransaction::Deposit {
                amount, dispute, ..
            } => {
                anyhow::ensure!(
                    matches!(dispute, AccountTransactionDisppute::Resolved),
                    "there is not a dispute for transaction"
                );
                anyhow::ensure!(self.total >= *amount, "not enough balance for dispute");
                self.total -= *amount;
                *dispute = AccountTransactionDisppute::ChargeBackOccurred;
                self.locked = true;
            }
            AccountTransaction::Withdrawal {
                amount, dispute, ..
            } => {
                anyhow::ensure!(
                    matches!(dispute, AccountTransactionDisppute::Resolved),
                    "there is not a dispute for transaction"
                );
                anyhow::ensure!(self.total >= *amount, "not enough held for dispute");
                self.total += *amount;
                *dispute = AccountTransactionDisppute::ChargeBackOccurred;
                self.locked = true;
            }
        }
        Ok(())
    }
}
