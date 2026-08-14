use std::time::Instant;

use anyhow::{Context, Result};
use simple_transaction::{Transaction, account_store::AccountStore};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(EnvFilter::from_default_env())
        .init();

    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(
        args.len() == 2,
        "only one param with the input CSV file name is expected"
    );

    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_path(args.get(1).context("file name not provided")?)?;

    let account_store = AccountStore::default();
    let transactions = reader.deserialize::<simple_transaction::csv::Transaction>();
    let mut parsing_line_error = 0;
    let mut convert_error = 0;
    let mut processed_lines = 0;
    let start = Instant::now();
    let result = account_store.apply_transactions(transactions.filter_map(|x| {
        processed_lines += 1;
        Transaction::try_from(x.inspect_err(|_| parsing_line_error += 1).ok()?)
            .inspect_err(|_| convert_error += 1)
            .ok()
    }));
    let processing_time = start.elapsed();
    tracing::info!(
        processed_lines,
        parsing_line_error,
        convert_error,
        err = ?result.err(),
        ?processing_time,
        processing_time_per_tx=?processing_time.checked_div(processed_lines).unwrap_or_default(),
        "finished applying transactions"
    );

    account_store
        .write(std::io::stdout())
        .context("writing response")
}
