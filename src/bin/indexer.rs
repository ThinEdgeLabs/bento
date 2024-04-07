use bento::chainweb_client::ChainwebClient;
use bento::db;
use bento::gaps;
use bento::indexer::*;
use bento::repository::*;
use clap::ValueEnum;
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
    /// Backfill data
    Backfill(BackfillArgs),
    /// Find and index missed blocks
    Gaps,
}

#[derive(clap::Args)]
#[command(version, about, long_about = None)]
struct BackfillArgs {
    #[arg(long, value_enum)]
    module: Option<Modules>,
    #[arg(long)]
    chain_id: Option<u16>,
    #[arg(long)]
    min_height: Option<u64>,
    #[arg(long)]
    max_height: Option<u64>,
    #[arg(short, long, default_value_t = 4)]
    /// Number of chains to backfill concurrently
    concurrency: usize,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Modules {
    MarmaladeV2,
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
        Some(Command::Backfill(args)) => {
            log::info!("Backfilling blocks...");
            match args.module {
                Some(Modules::MarmaladeV2) => {
                    log::info!("Backfilling MarmaladeV2 data...");
                    use bento::modules::marmalade_v2::backfill;
                    backfill::run(
                        pool.clone(),
                        args.chain_id,
                        args.min_height,
                        args.max_height,
                    )
                    .await?;
                }
                None => {
                    indexer
                        .backfill(
                            args.chain_id,
                            args.min_height,
                            args.max_height,
                            args.concurrency,
                        )
                        .await?;
                }
            }
        }
        Some(Command::Gaps) => {
            gaps::fill_gaps(&chainweb_client, &blocks, &indexer).await?;
        }
        None => {
            indexer.index_new_blocks().await?;
        }
    }

    Ok(())
}
