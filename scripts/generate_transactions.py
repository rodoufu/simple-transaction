#!/usr/bin/env python3
"""Generate a random transactions.csv input file for simple-transaction.

Usage:
    python3 scripts/generate_transactions.py 1000
    python3 scripts/generate_transactions.py 1000000 -o big.csv --clients 500 --seed 42
"""

import argparse
import csv
import random
import sys

MAX_CLIENT_ID = 65_535  # u16


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("size", type=int, help="number of transaction rows to generate")
    parser.add_argument("-o", "--output", default="transactions.csv", help="output CSV path (default: transactions.csv)")
    parser.add_argument("-c", "--clients", type=int, default=None, help="number of distinct client ids (default: max(1, size // 20))")
    parser.add_argument("--seed", type=int, default=None, help="random seed, for reproducible output")
    parser.add_argument("--withdrawal-rate", type=float, default=0.3, help="chance a new transaction is a withdrawal rather than a deposit (default: 0.3)")
    parser.add_argument("--dispute-rate", type=float, default=0.05, help="chance a row disputes an existing transaction instead of creating a new one (default: 0.05)")
    parser.add_argument("--resolve-rate", type=float, default=0.03, help="chance a row resolves an open dispute (default: 0.03)")
    parser.add_argument("--chargeback-rate", type=float, default=0.02, help="chance a row charges back a resolved dispute (default: 0.02)")
    parser.add_argument("--min-amount", type=float, default=0.01, help="minimum deposit/withdrawal amount (default: 0.01)")
    parser.add_argument("--max-amount", type=float, default=1000.0, help="maximum deposit/withdrawal amount (default: 1000.0)")
    return parser.parse_args()


def choose_action(disputable, open_disputes, resolved_disputes, args, rng):
    options = [("new", 1.0)]
    if disputable:
        options.append(("dispute", args.dispute_rate))
    if open_disputes:
        options.append(("resolve", args.resolve_rate))
    if resolved_disputes:
        options.append(("chargeback", args.chargeback_rate))
    actions, weights = zip(*options)
    return rng.choices(actions, weights=weights, k=1)[0]


def generate_rows(args, rng):
    clients = list(range(1, args.clients + 1))
    balances = {client: 0.0 for client in clients}

    disputable = []        # [(client, tx_id)] deposits/withdrawals not currently disputed
    open_disputes = []     # [(client, tx_id)] disputed, awaiting resolve
    resolved_disputes = []  # [(client, tx_id)] resolved, eligible for chargeback

    rows = []
    tx_id = 1

    while len(rows) < args.size:
        action = choose_action(disputable, open_disputes, resolved_disputes, args, rng)

        if action == "dispute":
            entry = rng.choice(disputable)
            disputable.remove(entry)
            open_disputes.append(entry)
            rows.append(("dispute", entry[0], entry[1], ""))
        elif action == "resolve":
            entry = rng.choice(open_disputes)
            open_disputes.remove(entry)
            resolved_disputes.append(entry)
            rows.append(("resolve", entry[0], entry[1], ""))
        elif action == "chargeback":
            entry = rng.choice(resolved_disputes)
            resolved_disputes.remove(entry)
            rows.append(("chargeback", entry[0], entry[1], ""))
        else:
            client = rng.choice(clients)
            can_withdraw = balances[client] >= args.min_amount
            if can_withdraw and rng.random() < args.withdrawal_rate:
                amount = round(rng.uniform(args.min_amount, min(balances[client], args.max_amount)), 4)
                balances[client] -= amount
                rows.append(("withdrawal", client, tx_id, amount))
            else:
                amount = round(rng.uniform(args.min_amount, args.max_amount), 4)
                balances[client] += amount
                rows.append(("deposit", client, tx_id, amount))
            disputable.append((client, tx_id))
            tx_id += 1

    return rows


def main():
    args = parse_args()

    if args.size <= 0:
        sys.exit("size must be a positive integer")
    if args.clients is None:
        args.clients = max(1, args.size // 20)
    if not (1 <= args.clients <= MAX_CLIENT_ID):
        sys.exit(f"clients must be between 1 and {MAX_CLIENT_ID}")
    if args.min_amount <= 0 or args.max_amount < args.min_amount:
        sys.exit("min-amount must be positive and no greater than max-amount")

    rng = random.Random(args.seed)
    rows = generate_rows(args, rng)

    with open(args.output, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["type", "client", "tx", "amount"])
        writer.writerows(rows)

    print(f"Wrote {len(rows)} rows across {args.clients} clients to {args.output}")


if __name__ == "__main__":
    main()
