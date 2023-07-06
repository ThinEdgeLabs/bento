use dotenvy::dotenv;

use bento::db;
use bento::indexer::*;
use bento::repository::*;

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

    log::info!("Starting chainweb indexing...");

    indexer.run().await?;

    log::info!("Done");

    Ok(())
}
