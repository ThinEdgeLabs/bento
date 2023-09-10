use bento::chainweb_client::ChainwebClient;
use bento::db;
use bento::gaps;
use bento::indexer::*;
use bento::repository::*;
use clap::{Parser, Subcommand};
use dotenvy::dotenv;

#[derive(Parser)]
/// By default new blocks are indexed as they are mined. For backfilling and filling gaps use the
/// subcommands.
struct IndexerCli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Backfill blocks
    Backfill,
    /// Index missed blocks
    Gaps,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    dotenv().ok();

    let pool = db::initialize_db_pool();
    db::run_migrations(&mut pool.get().unwrap()).unwrap();

    let blocks = BlocksRepository { pool: pool.clone() };
    let events = EventsRepository { pool: pool.clone() };
    let transactions = TransactionsRepository { pool: pool.clone() };
    let transfers_repo = TransfersRepository { pool: pool.clone() };
    let chainweb_client = ChainwebClient::new();
    let indexer = Indexer {
        chainweb_client: &chainweb_client,
        blocks: blocks.clone(),
        events: events.clone(),
        transactions: transactions.clone(),
        transfers: transfers_repo.clone(),
    };

    let args = IndexerCli::parse();
    match args.command {
        Some(Command::Backfill) => {
            log::info!("Backfilling blocks...");
            indexer.backfill().await?;
        }
        Some(Command::Gaps) => {
            log::info!("Filling gaps...");
            gaps::fill_gaps(&chainweb_client, &blocks, &indexer).await?;
        }
        None => {
            log::info!("Indexing blocks...");
            indexer.listen_headers_stream().await?;
        }
    }

    Ok(())
}
