use crate::{ClientId, TransactionId};
use anyhow::{Context, Result};
use rustc_hash::FxHashMap;

/// Multiplier used to convert the float point numbers from the input into integer.
pub(super) const ACCOUNT_MULTIPLIER: f64 = 1e4;

/// Specify the state of a transaction within an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccountTransactionDisppute {
    #[default]
    None,
    DisputeInitiated,
    Resolved,
    ChargeBackOccurred,
}

/// Transaction within an account with its state information.
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

/// Account represents the data for eache client.
/// Internally it uses `u64` instead of `f64` to avoid representations issues such as `0.1` not
/// being possible to exactly represent as a `f64`.
/// `ACCOUNT_MULTIPLIER` is used to convert the `f64` to `u64` using the expected precision.
#[derive(Debug, Clone)]
pub struct Account {
    /// Client identification
    pub(super) id: ClientId,

    total: u64,
    held: u64,

    locked: bool,
    transactions: FxHashMap<TransactionId, AccountTransaction>,
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
        self.available_u64() as f64 / ACCOUNT_MULTIPLIER
    }

    fn available_u64(&self) -> u64 {
        self.total - self.held
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

    /// Adds a deposit for the specified parameters.
    pub(crate) fn deposit(&mut self, id: TransactionId, amount: u64) -> Result<()> {
        anyhow::ensure!(
            !self.transactions.contains_key(&id),
            "transaction already exists"
        );

        self.total = self.total.checked_add(amount).context("balance overflow")?;
        self.transactions.insert(
            id,
            AccountTransaction::Deposit {
                id,
                amount,
                dispute: AccountTransactionDisppute::None,
            },
        );
        Ok(())
    }

    /// Adds a withdrawal for the specified parameters.
    /// The withdrawal will fail if there is no available balance and no change to the balance is applied.
    pub(crate) fn withdrawal(&mut self, id: TransactionId, amount: u64) -> Result<()> {
        anyhow::ensure!(
            !self.transactions.contains_key(&id),
            "transaction already exists"
        );
        anyhow::ensure!(
            self.available_u64() >= amount,
            "not enough balance for withdrawal"
        );

        self.total = self
            .total
            .checked_sub(amount)
            .context("balance underflow")?;
        self.transactions.insert(
            id,
            AccountTransaction::Withdrawal {
                id,
                amount,
                dispute: AccountTransactionDisppute::None,
            },
        );
        Ok(())
    }

    /// Starts a dispute for a specified transaction.
    /// If the transaction is not found the dispute fails.
    pub(crate) fn start_dispute(&mut self, transaction_id: TransactionId) -> Result<()> {
        let transaction = self
            .transactions
            .get_mut(&transaction_id)
            .context("transaction not found for dispute")?;
        match transaction {
            AccountTransaction::Deposit {
                amount, dispute, ..
            } => {
                anyhow::ensure!(
                    matches!(dispute, AccountTransactionDisppute::None),
                    "there is already a dispute for transaction"
                );
                anyhow::ensure!(
                    self.total - self.held >= *amount,
                    "not enough balance for dispute"
                );
                self.held = self.held.checked_add(*amount).context("held overflow")?;
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
                self.held = self.held.checked_sub(*amount).context("held underflow")?;
                *dispute = AccountTransactionDisppute::DisputeInitiated;
            }
        }
        Ok(())
    }

    // Resolve a dispute.
    /// If the transaction is not found or has not a dispute already started the dispute resolve fails.
    pub(crate) fn resolve_dispute(&mut self, transaction_id: TransactionId) -> Result<()> {
        let transaction = self
            .transactions
            .get_mut(&transaction_id)
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
                self.held = self.held.checked_sub(*amount).context("held underflow")?;
                *dispute = AccountTransactionDisppute::Resolved;
            }
            AccountTransaction::Withdrawal {
                amount, dispute, ..
            } => {
                anyhow::ensure!(
                    matches!(dispute, AccountTransactionDisppute::DisputeInitiated),
                    "there is not a dispute for transaction"
                );
                anyhow::ensure!(
                    self.total - self.held >= *amount,
                    "not enough balance for dispute"
                );
                self.held = self.held.checked_add(*amount).context("held overflow")?;
                *dispute = AccountTransactionDisppute::Resolved;
            }
        }
        Ok(())
    }

    /// Process a chargeback.
    /// If the transaction is not found or has not a dispute resolved already the chargeback fails.
    pub(crate) fn chargeback(&mut self, transaction_id: TransactionId) -> Result<()> {
        let transaction = self
            .transactions
            .get_mut(&transaction_id)
            .context("transaction not found for chargeback")?;

        match transaction {
            AccountTransaction::Deposit {
                amount, dispute, ..
            } => {
                anyhow::ensure!(
                    matches!(dispute, AccountTransactionDisppute::Resolved),
                    "there is not a dispute for transaction"
                );
                anyhow::ensure!(self.total >= *amount, "not enough balance for chargeback");
                self.total = self
                    .total
                    .checked_sub(*amount)
                    .context("balance underflow")?;
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
                anyhow::ensure!(self.total >= *amount, "not enough balance for chargeback");
                self.total = self.total.checked_add(*amount).context("total overflow")?;
                *dispute = AccountTransactionDisppute::ChargeBackOccurred;
                self.locked = true;
            }
        }
        Ok(())
    }
}
