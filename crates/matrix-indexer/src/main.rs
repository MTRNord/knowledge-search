use color_eyre::Result;

use clap::Parser;
use matrix::IndexerBot;
use tracing::info;

mod indradb_utils;
mod matrix;

/// An indexer for the knowledge search that indexes matrix
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::init();
    let _args = Args::parse();

    // TODO: config which rewrites itself to have the data after login
    let mut bot =
        IndexerBot::new(String::new(), String::new(), String::new()).await?;
    bot.start_processing().await;

    Ok(())
}
