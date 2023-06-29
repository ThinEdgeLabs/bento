mod chainweb_client;
mod db;
mod indexer;
mod models;
mod repository;
mod schema;

use dotenvy::dotenv;
use indexer::*;
use log;
use repository::*;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    dotenv().ok();
    let pool = db::initialize_db_pool();
    let mut blocks = BlocksRepository { pool: &pool };
    let mut events = EventsRepository { pool: &pool };
    let mut transactions = TransactionsRepository { pool: &pool };

    log::info!("Deleted {} events", events.delete_all().unwrap());
    log::info!(
        "Deleted {} transactions",
        transactions.delete_all().unwrap()
    );
    log::info!("Deleted {} blocks", blocks.delete_all().unwrap());

    let indexer = Indexer {
        blocks: &mut blocks,
        events: &mut events,
        transactions: &mut transactions,
    };
    log::info!("Starting chainweb indexing");

    indexer.run().await.unwrap();

    log::info!("Finished");
}
