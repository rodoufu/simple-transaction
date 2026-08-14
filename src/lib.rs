use crate::account::ACCOUNT_MULTIPLIER;

pub mod account;
pub mod account_store;
#[cfg(test)]
mod account_store_tests;
#[cfg(test)]
mod account_tests;
pub mod csv;
#[cfg(test)]
mod csv_tests;
#[cfg(test)]
mod lib_tests;

pub type ClientId = u16;
pub type TransactionId = u32;

/// Represents the content of a dispute.
#[allow(dead_code)]
#[cfg_attr(test, derive(PartialEq, Eq))]
#[derive(Debug, Clone, Copy)]
pub struct Dispute {
    client: ClientId,
    transaction_id: TransactionId,
}

/// Represents the content of a dispute resolution.
#[allow(dead_code)]
#[cfg_attr(test, derive(PartialEq, Eq))]
#[derive(Debug, Clone, Copy)]
pub struct Resolve {
    client: ClientId,
    transaction_id: TransactionId,
}

/// Represents the content of a chargeback.
#[allow(dead_code)]
#[cfg_attr(test, derive(PartialEq, Eq))]
#[derive(Debug, Clone, Copy)]
pub struct Chargeback {
    client: ClientId,
    transaction_id: TransactionId,
}

#[cfg_attr(test, derive(PartialEq, Eq))]
#[derive(Debug)]
pub enum Transaction {
    /// A deposit is a credit to the client's asset account, meaning it should increase the available and
    /// total funds of the client account
    Deposit {
        client: ClientId,
        transaction_id: TransactionId,
        amount: u64,
    },
    /// A withdraw is a debit to the client's asset account, meaning it should decrease the available and
    /// total funds of the client account.
    /// If a client does not have sufficient available funds the withdrawal should fail and the total amount
    /// of funds should not change
    Withdrawal {
        client: ClientId,
        transaction_id: TransactionId,
        amount: u64,
    },
    /// A dispute represents a client's claim that a transaction was erroneous and should be reversed.
    /// The transaction shouldn't be reversed yet but the associated funds should be held.
    /// This means that the clients available funds should decrease by the amount disputed, their held funds should
    /// increase by the amount disputed, while their total funds should remain the same.
    /// Notice that a dispute does not state the amount disputed. Instead a dispute references the
    /// transaction that is disputed by ID. If the tx specified by the dispute doesn't exist you can ignore it
    /// and assume this is an error on our partners side.
    Dispute(Dispute),
    /// A resolve represents a resolution to a dispute, releasing the associated held funds. Funds that
    /// were previously disputed are no longer disputed. This means that the clients held funds should
    /// decrease by the amount no longer disputed, their available funds should increase by the amount
    /// no longer disputed, and their total funds should remain the same.
    /// Like disputes, resolves do not specify an amount. Instead they refer to a transaction that was
    /// under dispute by ID. If the tx specified doesn't exist, or the tx isn't under dispute, you can ignore
    /// the resolve and assume this is an error on our partner's side.
    Resolve(Resolve),
    /// A chargeback is the final state of a dispute and represents the client reversing a transaction.
    /// Funds that were held have now been withdrawn. This means that the clients held funds and total
    /// funds should decrease by the amount previously disputed. If a chargeback occurs the client's
    /// account should be immediately frozen.
    /// Like a dispute and a resolve a chargeback refers to the transaction by ID (tx) and does not
    /// specify an amount. Like a resolve, if the tx specified doesn't exist, or the tx isn't under dispute,
    /// you can ignore chargeback and assume this is an error on our partner's side.
    Chargeback(Chargeback),
}

impl Transaction {
    pub fn deposit(client: ClientId, transaction_id: TransactionId, amount: f64) -> Self {
        Self::Deposit {
            client,
            transaction_id,
            amount: (amount * ACCOUNT_MULTIPLIER + 0.5) as u64,
        }
    }

    pub fn withdrawal(client: ClientId, transaction_id: TransactionId, amount: f64) -> Self {
        Self::Withdrawal {
            client,
            transaction_id,
            amount: (amount * ACCOUNT_MULTIPLIER + 0.5) as u64,
        }
    }
}
