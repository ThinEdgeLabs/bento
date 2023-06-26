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
    let mut blocks = BlocksRepository {
        db: &mut pool.get().unwrap(),
    };
    let mut events = EventsRepository {
        db: &mut pool.get().unwrap(),
    };
    let mut transactions = TransactionsRepository {
        db: &mut pool.get().unwrap(),
    };

    blocks.delete_all();
    transactions.delete_all();
    events.delete_all();

    let mut indexer = Indexer {
        blocks,
        events,
        transactions,
    };
    log::info!("Starting chainweb indexing");

    indexer.run().await.unwrap();

    log::info!("Finished");
}
