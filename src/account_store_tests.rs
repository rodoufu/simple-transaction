use crate::{Chargeback, Dispute, Resolve, Transaction, account_store::AccountStore};

#[test]
fn test_apply_corner_cases() {
    struct Case {
        name: &'static str,
        ops: Vec<Transaction>,
        expect_err_on_last: Result<(), &'static str>,
        // (available, held, total, locked) expected for client 1, or None if no row should exist.
        expect_row: Vec<(&'static str, &'static str, &'static str, &'static str)>,
    }

    let cases = [
        Case {
            name: "deposit creates the account",
            ops: vec![Transaction::deposit(1, 1, 1.0)],
            expect_err_on_last: Ok(()),
            expect_row: vec![("1", "0", "1", "false")],
        },
        Case {
            name: "zero amount deposit is a no-op and creates no account",
            ops: vec![Transaction::deposit(1, 1, 0.0)],
            expect_err_on_last: Ok(()),
            expect_row: vec![],
        },
        Case {
            name: "withdrawal without an existing account fails",
            ops: vec![Transaction::withdrawal(1, 1, 0.5)],
            expect_err_on_last: Err(
                "errors processing transactions account_not_found:1, not_enough_balance:0, transaction_not_found:0",
            ),
            expect_row: vec![],
        },
        Case {
            name: "zero amount withdrawal without an account is a no-op",
            ops: vec![Transaction::withdrawal(1, 1, 0.0)],
            expect_err_on_last: Ok(()),
            expect_row: vec![],
        },
        Case {
            name: "withdrawal exceeding balance fails and leaves state unchanged",
            ops: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::withdrawal(1, 2, 2.0),
            ],
            expect_err_on_last: Err(
                "errors processing transactions account_not_found:0, not_enough_balance:1, transaction_not_found:0",
            ),
            expect_row: vec![("1", "0", "1", "false")],
        },
        Case {
            name: "dispute without an existing account fails",
            ops: vec![Transaction::Dispute(Dispute {
                client: 1,
                transaction_id: 1,
            })],
            expect_err_on_last: Err(
                "errors processing transactions account_not_found:1, not_enough_balance:0, transaction_not_found:0",
            ),
            expect_row: vec![],
        },
        Case {
            name: "resolve without an existing account fails",
            ops: vec![Transaction::Resolve(Resolve {
                client: 1,
                transaction_id: 1,
            })],
            expect_err_on_last: Err(
                "errors processing transactions account_not_found:1, not_enough_balance:0, transaction_not_found:0",
            ),
            expect_row: vec![],
        },
        Case {
            name: "chargeback without an existing account fails",
            ops: vec![Transaction::Chargeback(Chargeback {
                client: 1,
                transaction_id: 1,
            })],
            expect_err_on_last: Err(
                "errors processing transactions account_not_found:1, not_enough_balance:0, transaction_not_found:0",
            ),
            expect_row: vec![],
        },
        Case {
            name: "dispute referencing an unknown transaction fails, account is untouched",
            ops: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::Dispute(Dispute {
                    client: 1,
                    transaction_id: 999,
                }),
            ],
            expect_err_on_last: Err(
                "errors processing transactions account_not_found:0, not_enough_balance:0, transaction_not_found:1",
            ),
            expect_row: vec![("1", "0", "1", "false")],
        },
        Case {
            name: "deposit, dispute, resolve, chargeback locks the account",
            ops: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::Dispute(Dispute {
                    client: 1,
                    transaction_id: 1,
                }),
                Transaction::Resolve(Resolve {
                    client: 1,
                    transaction_id: 1,
                }),
                Transaction::Chargeback(Chargeback {
                    client: 1,
                    transaction_id: 1,
                }),
            ],
            expect_err_on_last: Ok(()),
            expect_row: vec![("0", "0", "0", "true")],
        },
        Case {
            name: "deposit, deposit, deposit, withdrawal, withdrawal",
            ops: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::deposit(2, 2, 2.0),
                Transaction::deposit(1, 3, 2.0),
                Transaction::withdrawal(1, 4, 1.5),
                Transaction::withdrawal(2, 5, 3.0),
            ],
            expect_err_on_last: Err(
                "errors processing transactions account_not_found:0, not_enough_balance:1, transaction_not_found:0",
            ),
            expect_row: vec![("1.5", "0", "1.5", "false"), ("2", "0", "2", "false")],
        },
    ];

    for case in cases {
        let store = AccountStore::default();
        let last_result = store.apply_transactions(case.ops);

        assert_eq!(
            last_result.map_err(|err| err.to_string()),
            case.expect_err_on_last.map_err(|err| err.to_string()),
            "{}: unexpected error",
            case.name
        );

        let mut buf = Vec::new();
        store.write(&mut buf).expect("write should succeed");
        let actual_row = String::from_utf8(buf).expect("output should be utf8");
        for (i, expected_row) in case.expect_row.iter().enumerate() {
            let (a, h, t, l) = expected_row;
            let expected_row = format!("{a},{h},{t},{l}");
            assert!(
                actual_row.contains(&expected_row),
                "{}: row {i} not found",
                case.name
            )
        }
    }
}

#[test]
fn test_write_outputs_one_row_per_client_with_header() {
    let store = AccountStore::default();

    // Client 1: a plain deposit.
    store
        .apply_transactions(std::iter::once(Transaction::deposit(1, 1, 1.0)))
        .expect("deposit should succeed");

    // Client 2: deposit followed by a partial withdrawal.
    store
        .apply_transactions(std::iter::once(Transaction::deposit(2, 2, 2.0)))
        .expect("deposit should succeed");
    store
        .apply_transactions(std::iter::once(Transaction::withdrawal(2, 3, 0.5)))
        .expect("withdrawal should succeed");

    // Client 3: deposit, disputed, resolved, then charged back and locked.
    store
        .apply_transactions(std::iter::once(Transaction::deposit(3, 4, 3.0)))
        .expect("deposit should succeed");
    store
        .apply_transactions(std::iter::once(Transaction::Dispute(Dispute {
            client: 3,
            transaction_id: 4,
        })))
        .expect("dispute should succeed");
    store
        .apply_transactions(std::iter::once(Transaction::Resolve(Resolve {
            client: 3,
            transaction_id: 4,
        })))
        .expect("resolve should succeed");
    store
        .apply_transactions(std::iter::once(Transaction::Chargeback(Chargeback {
            client: 3,
            transaction_id: 4,
        })))
        .expect("chargeback should succeed");

    let mut buf = Vec::new();
    store.write(&mut buf).expect("write should succeed");
    let csv_string = String::from_utf8(buf).expect("output should be utf8");

    let mut lines: Vec<&str> = csv_string.lines().collect();
    let header = lines.remove(0);
    assert_eq!(header, "client,available,held,total,locked");
    assert_eq!(lines.len(), 3, "expected one row per client");

    let mut buf = Vec::new();
    store.write(&mut buf).expect("write should succeed");
    let actual_row = String::from_utf8(buf).expect("output should be utf8");
    let expected_rows = [
        "client,available,held,total,locked",
        "2,1.5,0,1.5,false",
        "1,1,0,1,false",
        "3,0,0,0,true",
    ];
    for expected_row in expected_rows {
        assert!(actual_row.contains(expected_row));
    }
}
