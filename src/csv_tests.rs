use csv::Writer;

use crate::Transaction;
use crate::csv::{Transaction as CsvTransaction, TransactionType};

#[test]
fn test_serialize_transaction() {
    let transaction = CsvTransaction {
        transaction_type: TransactionType::Deposit,
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
fn test_deserialize_transaction() {
    struct Case {
        name: &'static str,
        csv: &'static str,
        has_headers: bool,
        expected: CsvTransaction,
    }

    let cases = [
        Case {
            name: "header with no spaces",
            csv: "type,client,tx,amount\ndeposit,1,1,1.0",
            has_headers: true,
            expected: CsvTransaction {
                transaction_type: TransactionType::Deposit,
                client_id: 1,
                transaction_id: 1,
                amount: Some(1.0),
            },
        },
        Case {
            name: "no header",
            csv: "deposit,1,1,1.0",
            has_headers: false,
            expected: CsvTransaction {
                transaction_type: TransactionType::Deposit,
                client_id: 1,
                transaction_id: 1,
                amount: Some(1.0),
            },
        },
        Case {
            name: "header with spaces is trimmed",
            csv: "type, client, tx, amount\ndeposit, 1, 1, 1.0",
            has_headers: true,
            expected: CsvTransaction {
                transaction_type: TransactionType::Deposit,
                client_id: 1,
                transaction_id: 1,
                amount: Some(1.0),
            },
        },
        Case {
            name: "no header, with spaces is trimmed",
            csv: "deposit, 1, 1, 1.0",
            has_headers: false,
            expected: CsvTransaction {
                transaction_type: TransactionType::Deposit,
                client_id: 1,
                transaction_id: 1,
                amount: Some(1.0),
            },
        },
    ];

    for case in cases {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(case.has_headers)
            .trim(csv::Trim::All)
            .from_reader(case.csv.as_bytes());

        let mut seen = 0;
        for result in reader.deserialize() {
            let record: CsvTransaction =
                result.unwrap_or_else(|err| panic!("{}: {err}", case.name));
            assert_eq!(record, case.expected, "{}", case.name);
            seen += 1;
        }
        assert_eq!(seen, 1, "{}: expected exactly one record", case.name);
    }
}

#[test]
fn test_try_from_csv_transaction() {
    struct Case {
        name: &'static str,
        input: CsvTransaction,
        expect: Result<Transaction, &'static str>,
    }

    let cases = [
        Case {
            name: "deposit with amount converts",
            input: CsvTransaction {
                transaction_type: TransactionType::Deposit,
                client_id: 1,
                transaction_id: 10,
                amount: Some(1.5),
            },
            expect: Ok(Transaction::Deposit {
                client: 1,
                transaction_id: 10,
                amount: 15_000,
            }),
        },
        Case {
            name: "withdrawal with amount converts",
            input: CsvTransaction {
                transaction_type: TransactionType::Withdrawal,
                client_id: 2,
                transaction_id: 20,
                amount: Some(2.25),
            },
            expect: Ok(Transaction::Withdrawal {
                client: 2,
                transaction_id: 20,
                amount: 22_500,
            }),
        },
        Case {
            name: "deposit without amount errors",
            input: CsvTransaction {
                transaction_type: TransactionType::Deposit,
                client_id: 1,
                transaction_id: 1,
                amount: None,
            },
            expect: Err("amount is mandatory for deposit and withdrawal transactions"),
        },
        Case {
            name: "withdrawal without amount errors",
            input: CsvTransaction {
                transaction_type: TransactionType::Withdrawal,
                client_id: 1,
                transaction_id: 1,
                amount: None,
            },
            expect: Err("amount is mandatory for deposit and withdrawal transactions"),
        },
        Case {
            name: "dispute with amount errors",
            input: CsvTransaction {
                transaction_type: TransactionType::Dispute,
                client_id: 1,
                transaction_id: 1,
                amount: Some(1.0),
            },
            expect: Err(
                "amount should not be present for dispute, resolve or chargeback transactions",
            ),
        },
        Case {
            name: "resolve with amount errors",
            input: CsvTransaction {
                transaction_type: TransactionType::Resolve,
                client_id: 1,
                transaction_id: 1,
                amount: Some(1.0),
            },
            expect: Err(
                "amount should not be present for dispute, resolve or chargeback transactions",
            ),
        },
        Case {
            name: "chargeback with amount errors",
            input: CsvTransaction {
                transaction_type: TransactionType::Chargeback,
                client_id: 1,
                transaction_id: 1,
                amount: Some(1.0),
            },
            expect: Err(
                "amount should not be present for dispute, resolve or chargeback transactions",
            ),
        },
        Case {
            name: "dispute without amount converts",
            input: CsvTransaction {
                transaction_type: TransactionType::Dispute,
                client_id: 1,
                transaction_id: 7,
                amount: None,
            },
            expect: Ok(Transaction::Dispute(crate::Dispute {
                client: 1,
                transaction_id: 7,
            })),
        },
        Case {
            name: "resolve without amount converts",
            input: CsvTransaction {
                transaction_type: TransactionType::Resolve,
                client_id: 1,
                transaction_id: 8,
                amount: None,
            },
            expect: Ok(Transaction::Resolve(crate::Resolve {
                client: 1,
                transaction_id: 8,
            })),
        },
        Case {
            name: "chargeback without amount converts",
            input: CsvTransaction {
                transaction_type: TransactionType::Chargeback,
                client_id: 1,
                transaction_id: 9,
                amount: None,
            },
            expect: Ok(Transaction::Chargeback(crate::Chargeback {
                client: 1,
                transaction_id: 9,
            })),
        },
    ];

    for (i, case) in cases.into_iter().enumerate() {
        let result = Transaction::try_from(case.input).map_err(|err| err.to_string());
        assert_eq!(
            result,
            case.expect.map_err(|err| err.to_string()),
            "{i}: {} - got unexpected result",
            case.name
        );
    }
}
