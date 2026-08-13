use crate::Transaction;

#[test]
fn test_deposit() {
    struct Case {
        name: &'static str,
        client: u16,
        transaction_id: u32,
        amount: f64,
        expected_amount: u64,
    }

    let cases = [
        Case {
            name: "simple amount converts to minor units",
            client: 1,
            transaction_id: 1,
            amount: 1.5,
            expected_amount: 15_000,
        },
        Case {
            name: "whole number amount",
            client: 2,
            transaction_id: 2,
            amount: 100.0,
            expected_amount: 1_000_000,
        },
        // 0.57 * 10_000.0 evaluates to 5699.999999999999 in f64, so without the
        // `+ 0.5` rounding this would truncate to 5_699 instead of 5_700.
        Case {
            name: "amount that truncates incorrectly without rounding",
            client: 3,
            transaction_id: 3,
            amount: 0.57,
            expected_amount: 5_700,
        },
    ];

    for case in cases {
        let actual = Transaction::deposit(case.client, case.transaction_id, case.amount);
        let expected = Transaction::Deposit {
            client: case.client,
            transaction_id: case.transaction_id,
            amount: case.expected_amount,
        };
        assert_eq!(actual, expected, "{}", case.name);
    }
}

#[test]
fn test_withdrawal() {
    struct Case {
        name: &'static str,
        client: u16,
        transaction_id: u32,
        amount: f64,
        expected_amount: u64,
    }

    let cases = [
        Case {
            name: "simple amount converts to minor units",
            client: 1,
            transaction_id: 1,
            amount: 2.25,
            expected_amount: 22_500,
        },
        Case {
            name: "whole number amount",
            client: 2,
            transaction_id: 2,
            amount: 50.0,
            expected_amount: 500_000,
        },
        // 1.13 * 10_000.0 evaluates to 11299.999999999998 in f64, so without the
        // `+ 0.5` rounding this would truncate to 11_299 instead of 11_300.
        Case {
            name: "amount that truncates incorrectly without rounding",
            client: 3,
            transaction_id: 3,
            amount: 1.13,
            expected_amount: 11_300,
        },
    ];

    for case in cases {
        let actual = Transaction::withdrawal(case.client, case.transaction_id, case.amount);
        let expected = Transaction::Withdrawal {
            client: case.client,
            transaction_id: case.transaction_id,
            amount: case.expected_amount,
        };
        assert_eq!(actual, expected, "{}", case.name);
    }
}
