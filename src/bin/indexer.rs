use bento::db;
use bento::gaps;
use bento::indexer::*;
use bento::repository::*;
use dotenvy::dotenv;
use std::env;

/// List of available commands:
/// - `cargo run --bin indexer` - index blocks from current height
/// - `cargo run --bin indexer -- --backfill` - index all blocks from current height to genesis
///TODO: - `cargo run --bin indexer -- --gaps` - fill gaps
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    dotenv().ok();

    let mut migration_conn = db::migration_connection();
    db::run_migrations(&mut migration_conn).unwrap();

    let pool = db::initialize_db_pool();
    let blocks = BlocksRepository { pool: pool.clone() };
    let events = EventsRepository { pool: pool.clone() };
    let transactions = TransactionsRepository { pool: pool.clone() };

    let indexer = Indexer {
        blocks: blocks.clone(),
        events: events.clone(),
        transactions: transactions.clone(),
    };

    let args: Vec<String> = env::args().collect();
    let backfill = args.contains(&"--backfill".to_string());
    let gaps = args.contains(&"--gaps".to_string());

    if backfill {
        log::info!("Backfilling blocks...");
        indexer.backfill().await?;
    } else if gaps {
        log::info!("Filling gaps...");
        gaps::fill_gaps(&blocks).await?;
    } else {
        log::info!("Indexing blocks...");
        indexer.listen_headers_stream().await?;
    }

    Ok(())
}
