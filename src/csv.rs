use serde::Deserialize;
#[cfg(test)]
use serde::Serialize;

use crate::TransactionId;

#[cfg_attr(test, derive(Serialize, PartialEq, Eq))]
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[cfg_attr(test, derive(Serialize, PartialEq))]
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Transaction {
    #[serde(rename = "type")]
    pub(super) transaction_type: TransactionType,
    #[serde(rename = "client")]
    pub(super) client_id: u16,
    #[serde(rename = "tx")]
    pub(super) transaction_id: TransactionId,
    pub(super) amount: Option<f64>,
}

impl TryFrom<Transaction> for crate::Transaction {
    type Error = anyhow::Error;

    fn try_from(value: Transaction) -> Result<Self, Self::Error> {
        match (value.transaction_type, value.amount) {
            (TransactionType::Deposit | TransactionType::Withdrawal, None) => {
                anyhow::bail!("amount is mandatory for deposit and withdrawal transactions")
            }
            (TransactionType::Deposit, Some(amount)) => Ok(crate::Transaction::deposit(
                value.client_id,
                value.transaction_id,
                amount,
            )),
            (TransactionType::Withdrawal, Some(amount)) => Ok(crate::Transaction::withdrawal(
                value.client_id,
                value.transaction_id,
                amount,
            )),
            (
                TransactionType::Dispute | TransactionType::Resolve | TransactionType::Chargeback,
                Some(_),
            ) => {
                anyhow::bail!(
                    "amount should not be present for dispute, resolve or chargeback transactions"
                )
            }
            (TransactionType::Dispute, None) => Ok(crate::Transaction::Dispute(crate::Dispute {
                client: value.client_id,
                transaction_id: value.transaction_id,
            })),
            (TransactionType::Resolve, None) => Ok(crate::Transaction::Resolve(crate::Resolve {
                client: value.client_id,
                transaction_id: value.transaction_id,
            })),
            (TransactionType::Chargeback, None) => {
                Ok(crate::Transaction::Chargeback(crate::Chargeback {
                    client: value.client_id,
                    transaction_id: value.transaction_id,
                }))
            }
        }
    }
}
