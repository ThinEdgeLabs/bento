use bento::db;
use bento::gaps;
use bento::indexer::*;
use bento::repository::*;
use bento::transfers;
use dotenvy::dotenv;
use std::env;
use std::process;

/// List of available commands:
/// - `cargo run --bin indexer` - index blocks from current height
/// - `cargo run --bin indexer -- --backfill` - index all blocks from current height to genesis
/// - `cargo run --bin indexer -- --gaps` - fill gaps
/// - `cargo run --bin indexer -- --transfers` - backfill transfers
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
    let transfers_repo = TransfersRepository { pool: pool.clone() };

    let indexer = Indexer {
        blocks: blocks.clone(),
        events: events.clone(),
        transactions: transactions.clone(),
        transfers: transfers_repo.clone(),
    };

    let args: Vec<String> = env::args().collect();
    let backfill = args.contains(&"--backfill".to_string());
    let gaps = args.contains(&"--gaps".to_string());
    let backfill_range = args.contains(&"--backfill-range".to_string());
    let transfers = args.contains(&"--transfers".to_string());

    if backfill {
        log::info!("Backfilling blocks...");
        indexer.backfill().await?;
    } else if backfill_range {
        if args.len() < 5 {
            log::error!("Not enough arguments for backfill-range.");
            log::info!("Usage: cargo run --bin indexer -- --backfill-range <min-height> <max-height> <chain-id>");
            process::exit(1);
        }
        let min_height = args[2].parse::<i64>().unwrap();
        let max_height = args[3].parse::<i64>().unwrap();
        let chain_id = args[4].parse::<i64>().unwrap();
        indexer
            .backfill_range(min_height, max_height, chain_id)
            .await?;
    } else if gaps {
        log::info!("Filling gaps...");
        gaps::fill_gaps(&blocks, &indexer).await?;
    } else if transfers {
        log::info!("Backfilling transfers...");
        transfers::backfill(50, &events, &transfers_repo).await?;
    } else {
        log::info!("Indexing blocks...");
        indexer.listen_headers_stream().await?;
    }

    Ok(())
}
