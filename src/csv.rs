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
    transaction_type: TransactionType,
    #[serde(rename = "client")]
    client_id: u16,
    #[serde(rename = "tx")]
    transaction_id: TransactionId,
    amount: Option<f64>,
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
            (TransactionType::Withdrawal, Some(amount)) => Ok(crate::Transaction::withdrawwal(
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

#[cfg(test)]

mod test {
    use csv::{Reader, Writer};

    use crate::csv::Transaction;

    #[test]
    fn serialize_transaction() {
        let transaction = Transaction {
            transaction_type: crate::csv::TransactionType::Deposit,
            client_id: 1,
            transaction_id: 1,
            amount: Some(1.0),
        };

        let mut writer = Writer::from_writer(vec![]);
        writer.serialize(transaction).expect("ok");

        let inner_bytes = writer.into_inner().expect("ok");
        let csv_string = String::from_utf8(inner_bytes).expect("ok");

        assert_eq!(csv_string, "type,client,tx,amount\ndeposit,1,1,1.0\n");
    }

    #[test]
    fn deserialize_transaction() {
        struct Test {
            csv: &'static str,
            transaction: Transaction,
        }
        let tests = [
            Test {
                csv: r#"type,client,tx,amount
deposit,1,1,1.0"#,
                transaction: Transaction {
                    transaction_type: crate::csv::TransactionType::Deposit,
                    client_id: 1,
                    transaction_id: 1,
                    amount: Some(1.0),
                },
            },
            Test {
                csv: r#"deposit,1,1,1.0"#,
                transaction: Transaction {
                    transaction_type: crate::csv::TransactionType::Deposit,
                    client_id: 1,
                    transaction_id: 1,
                    amount: Some(1.0),
                },
            },
            Test {
                csv: r#"type, client, tx, amount
deposit, 1, 1, 1.0"#,
                transaction: Transaction {
                    transaction_type: crate::csv::TransactionType::Deposit,
                    client_id: 1,
                    transaction_id: 1,
                    amount: Some(1.0),
                },
            },
            Test {
                csv: r#"deposit, 1, 1, 1.0"#,
                transaction: Transaction {
                    transaction_type: crate::csv::TransactionType::Deposit,
                    client_id: 1,
                    transaction_id: 1,
                    amount: Some(1.0),
                },
            },
        ];

        for (idx, test) in tests.iter().enumerate() {
            let mut reader = Reader::from_reader(test.csv.as_bytes());
            for result in reader.deserialize() {
                let record: Transaction = result.expect("ok");
                assert_eq!(record, test.transaction, "{idx} - not expected transation");
            }
        }
    }
}
