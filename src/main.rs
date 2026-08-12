use anyhow::{Context, Result};
use simple_transaction::{Transaction, account_store::AccountStore};
use tracing::error;
use tracing_subscriber::{EnvFilter, prelude::*};

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .init();

    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(args.len() == 2, "only one param is expected");

    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(args.get(1).context("file name not provided")?)?;

    let mut account_store = AccountStore::default();

    for (line, result) in reader.deserialize().enumerate() {
        let result: Result<simple_transaction::csv::Transaction> = result.context("parsing line");
        let Ok(transaction) = result.inspect_err(|err| error!(?err, line, "unexpected format"))
        else {
            continue;
        };
        let Ok(transaction) = Transaction::try_from(transaction)
            .inspect_err(|err| error!(?err, line, "problem converting transaction"))
        else {
            continue;
        };
        let _ = account_store
            .apply(transaction)
            .inspect_err(|err| error!(?err, line, "unable to apply transasction"));
    }

    account_store
        .write(std::io::stdout())
        .context("writing response")
}
