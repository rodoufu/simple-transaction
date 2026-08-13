use core::panic;

use crate::{Chargeback, Dispute, Resolve, Transaction, account::Account};

#[test]
fn test_deposit() {
    struct Case {
        name: &'static str,
        deposits: Vec<Transaction>,
        expect_available: f64,
        expect_held: f64,
        expect_total: f64,
    }

    let cases = [
        Case {
            name: "single deposit",
            deposits: vec![Transaction::deposit(1, 1, 1.0)],
            expect_available: 1.0,
            expect_held: 0.0,
            expect_total: 1.0,
        },
        Case {
            name: "multiple deposits accumulate",
            deposits: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::deposit(1, 2, 2.5),
            ],
            expect_available: 3.5,
            expect_held: 0.0,
            expect_total: 3.5,
        },
        Case {
            name: "zero amount deposit is a no-op",
            deposits: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::deposit(1, 2, 0.0),
            ],
            expect_available: 1.0,
            expect_held: 0.0,
            expect_total: 1.0,
        },
    ];

    for case in cases {
        let mut account = Account::new(1);
        for tx in case.deposits {
            let Transaction::Deposit {
                transaction_id,
                amount,
                ..
            } = tx
            else {
                panic!("unexpected test");
            };
            account.deposit(transaction_id, amount);
        }

        assert_eq!(
            account.available(),
            case.expect_available,
            "{} available",
            case.name
        );
        assert_eq!(account.held(), case.expect_held, "{} held", case.name);
        assert_eq!(account.total(), case.expect_total, "{} total", case.name);
        assert!(!account.locked(), "{} locked", case.name);
    }
}

#[test]
fn test_withdrawal() {
    struct Case {
        name: &'static str,
        initial_deposit: u64,
        withdrawal: u64,
        expect_err: Result<(), &'static str>,
        expect_total: f64,
    }

    let cases = [
        Case {
            name: "withdrawal within balance succeeds",
            initial_deposit: 10_000,
            withdrawal: 4_000,
            expect_err: Ok(()),
            expect_total: 0.6,
        },
        Case {
            name: "withdrawal of exact balance succeeds",
            initial_deposit: 10_000,
            withdrawal: 10_000,
            expect_err: Ok(()),
            expect_total: 0.0,
        },
        Case {
            name: "withdrawal exceeding balance fails and leaves balance unchanged",
            initial_deposit: 10_000,
            withdrawal: 20_000,
            expect_err: Err("not enough balance for withdrawal"),
            expect_total: 1.0,
        },
        Case {
            name: "withdrawal from empty account fails",
            initial_deposit: 0,
            withdrawal: 100,
            expect_err: Err("not enough balance for withdrawal"),
            expect_total: 0.0,
        },
    ];

    for case in cases {
        let mut account = Account::new(1);
        if case.initial_deposit > 0 {
            account.deposit(1, case.initial_deposit);
        }

        let result = account
            .withdrawal(2, case.withdrawal)
            .map_err(|err| err.to_string());
        assert_eq!(
            result,
            case.expect_err.map_err(|err| err.to_string()),
            "{}: unexpected result",
            case.name
        );

        assert_eq!(account.total(), case.expect_total, "{} total", case.name);
    }
}

#[test]
fn test_dispute_lifecycle() {
    fn apply(account: &mut Account, action: Transaction) -> anyhow::Result<()> {
        match action {
            Transaction::Deposit {
                transaction_id,
                amount,
                ..
            } => {
                account.deposit(transaction_id, amount);
                Ok(())
            }
            Transaction::Withdrawal {
                transaction_id,
                amount,
                ..
            } => account.withdrawal(transaction_id, amount),
            Transaction::Dispute(dispute) => account.start_dispute(dispute.transaction_id),
            Transaction::Resolve(resolve) => account.resolve_dispute(resolve.transaction_id),
            Transaction::Chargeback(chargeback) => account.chargeback(chargeback.transaction_id),
        }
    }

    struct Case {
        name: &'static str,
        actions: Vec<Transaction>,
        expect_err_on_last: Result<(), &'static str>,
        expect_available: f64,
        expect_held: f64,
        expect_total: f64,
        expect_locked: bool,
    }

    let cases = [
        Case {
            name: "disputing a deposit moves funds from available to held",
            actions: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::Dispute(Dispute {
                    client: 1,
                    transaction_id: 1,
                }),
            ],
            expect_err_on_last: Ok(()),
            expect_available: 0.0,
            expect_held: 1.0,
            expect_total: 1.0,
            expect_locked: false,
        },
        Case {
            name: "resolving a dispute releases held funds back to available",
            actions: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::Dispute(Dispute {
                    client: 1,
                    transaction_id: 1,
                }),
                Transaction::Resolve(Resolve {
                    client: 1,
                    transaction_id: 1,
                }),
            ],
            expect_err_on_last: Ok(()),
            expect_available: 1.0,
            expect_held: 0.0,
            expect_total: 1.0,
            expect_locked: false,
        },
        Case {
            name: "chargeback after resolve reduces total and locks the account",
            actions: vec![
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
            expect_available: 0.0,
            expect_held: 0.0,
            expect_total: 0.0,
            expect_locked: true,
        },
        Case {
            name: "chargeback without a prior resolve fails",
            actions: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::Dispute(Dispute {
                    client: 1,
                    transaction_id: 1,
                }),
                Transaction::Chargeback(Chargeback {
                    client: 1,
                    transaction_id: 1,
                }),
            ],
            expect_err_on_last: Err("there is not a dispute for transaction"),
            expect_available: 0.0,
            expect_held: 1.0,
            expect_total: 1.0,
            expect_locked: false,
        },
        Case {
            name: "disputing an unknown transaction fails",
            actions: vec![Transaction::Dispute(Dispute {
                client: 1,
                transaction_id: 999,
            })],
            expect_err_on_last: Err("transaction not found for dispute"),
            expect_available: 0.0,
            expect_held: 0.0,
            expect_total: 0.0,
            expect_locked: false,
        },
        Case {
            name: "disputing the same transaction twice fails",
            actions: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::Dispute(Dispute {
                    client: 1,
                    transaction_id: 1,
                }),
                Transaction::Dispute(Dispute {
                    client: 1,
                    transaction_id: 1,
                }),
            ],
            expect_err_on_last: Err("there is already a dispute for transaction"),
            expect_available: 0.0,
            expect_held: 1.0,
            expect_total: 1.0,
            expect_locked: false,
        },
        Case {
            name: "resolving a transaction without an open dispute fails",
            actions: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::Resolve(Resolve {
                    client: 1,
                    transaction_id: 1,
                }),
            ],
            expect_err_on_last: Err("there is not a dispute for transaction"),
            expect_available: 1.0,
            expect_held: 0.0,
            expect_total: 1.0,
            expect_locked: false,
        },
        Case {
            name: "disputing a withdrawal with nothing held fails",
            actions: vec![
                Transaction::deposit(1, 1, 1.0),
                Transaction::withdrawal(1, 2, 0.5),
                Transaction::Dispute(Dispute {
                    client: 1,
                    transaction_id: 1,
                }),
            ],
            expect_err_on_last: Err("not enough balance for dispute"),
            expect_available: 0.5,
            expect_held: 0.0,
            expect_total: 0.5,
            expect_locked: false,
        },
    ];

    for case in cases {
        let mut account = Account::new(1);
        let mut last_result = Ok(());
        for action in case.actions {
            last_result = apply(&mut account, action);
            if last_result.is_err() {
                break;
            }
        }

        assert_eq!(
            last_result.map_err(|err| err.to_string()),
            case.expect_err_on_last.map_err(|err| err.to_string()),
            "{}: unexpected error",
            case.name
        );

        assert_eq!(
            account.available(),
            case.expect_available,
            "{} available",
            case.name
        );
        assert_eq!(account.held(), case.expect_held, "{} held", case.name);
        assert_eq!(account.total(), case.expect_total, "{} total", case.name);
        assert_eq!(account.locked(), case.expect_locked, "{} locked", case.name);
    }
}
