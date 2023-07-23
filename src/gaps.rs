use std::vec;

use futures::{stream, StreamExt};

use crate::chainweb_client::{self, Bounds, ChainId, Hash};
use crate::indexer::Indexer;
use crate::models::Block;
use crate::{db::DbError, repository::BlocksRepository};

pub async fn fill_gaps(
    blocks_repo: &BlocksRepository,
    indexer: &Indexer,
) -> Result<(), Box<dyn std::error::Error>> {
    let cut = chainweb_client::get_cut().await.unwrap();
    let gaps = cut
        .hashes
        .iter()
        .map(|(chain, _)| {
            let gaps = find_gaps(&chain, blocks_repo).unwrap();
            let missing_blocks = gaps
                .iter()
                .map(|gap| gap.1.height - gap.0.height - 1)
                .reduce(|acc, e| acc + e)
                .unwrap_or(0);
            log::info!("Chain {}, is missing {} blocks.", chain, missing_blocks);
            (chain, gaps)
        })
        .collect::<Vec<(&ChainId, Vec<(Block, Block)>)>>();

    for el in gaps {
        let (chain, gaps) = el;
        log::info!("Filling {} gaps for chain: {:?}", gaps.len(), chain);
        gaps.iter().for_each(|e| {
            log::info!(
                "Gap: {} - {}, size: {}",
                e.0.height,
                e.1.height,
                e.1.height - e.0.height - 1
            )
        });
        stream::iter(gaps)
            .map(|(lower_bound, upper_bound)| async move {
                indexer
                    .index_chain(
                        Bounds {
                            lower: vec![Hash(lower_bound.hash.clone())],
                            upper: vec![Hash(upper_bound.hash.clone())],
                        },
                        &chain,
                    )
                    .await
            })
            .buffer_unordered(4)
            .for_each(|result| {
                match result {
                    Err(e) => log::error!("Error filling gap: {:?}", e),
                    _ => {}
                }
                async {}
            })
            .await;
    }
    Ok(())
}

/// Check if there are any gaps in the blocks table
/// by comparing number of blocks with the difference between max and min height
/// If there are gaps, find them and return a list of tuples (lower_bound, upper_bound)
pub fn find_gaps(
    chain_id: &ChainId,
    repository: &BlocksRepository,
) -> Result<Vec<(Block, Block)>, DbError> {
    let before = std::time::Instant::now();
    let count = repository.count(chain_id.0 as i64).unwrap();
    log::info!(
        "Counted {} blocks for chain {} in {} ms",
        count,
        chain_id.0,
        before.elapsed().as_millis()
    );
    if count == 0 {
        return Ok(vec![]);
    }
    let before = std::time::Instant::now();
    match repository
        .find_min_max_height_blocks(chain_id.0 as i64)
        .unwrap()
    {
        (Some(min_block), Some(max_block)) => {
            log::info!(
                "Found min and max blocks for chain {} in {} ms",
                chain_id.0,
                before.elapsed().as_millis()
            );
            log::info!(
                "Min block: {}, max block: {}, count: {}",
                min_block.height,
                max_block.height,
                count
            );
            // Chains 0-9 have height 0 for genesis block, 10 to 19 were added later (height 852054)
            let no_gaps = if min_block.height == 0 {
                (max_block.height + 1) == count
            } else {
                (max_block.height - min_block.height + 1) == 0
            };
            if !no_gaps {
                find_gaps_in_range(
                    min_block.height,
                    max_block.height,
                    chain_id.0 as i64,
                    repository,
                )
            } else {
                Ok(vec![])
            }
        }
        _ => Ok(vec![]),
    }
}

/// Start from max height and go backwards
/// Query blocks in batches and look for gaps
/// Return a list of tuples (lower_bound, upper_bound)
fn find_gaps_in_range(
    min_height: i64,
    max_height: i64,
    chain: i64,
    repository: &BlocksRepository,
) -> Result<Vec<(Block, Block)>, DbError> {
    let batch_size = 100;
    let mut max_height = max_height;
    let mut gaps: Vec<(Block, Block)> = vec![];
    let mut last_block = None;
    loop {
        if max_height <= min_height {
            return Ok(gaps);
        }
        let blocks = repository.find_by_range(max_height - batch_size, max_height, chain)?;
        if blocks.is_empty() {
            max_height -= batch_size;
            continue;
        }
        let mut previous_block = if last_block.is_none() {
            blocks.last().unwrap().clone()
        } else {
            last_block.unwrap()
        };

        for block in blocks.iter() {
            if previous_block.height - block.height > 1 {
                gaps.push((block.clone(), previous_block));
            }
            previous_block = block.clone();
        }

        max_height -= batch_size;
        last_block = Some(previous_block);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::Block;
    use crate::repository::BlocksRepository;
    use bigdecimal::BigDecimal;
    use chrono::Utc;

    fn make_block(chain_id: i64, height: i64) -> Block {
        Block {
            chain_id: chain_id,
            hash: format!("hash-{}", height).to_string(),
            height: height,
            parent: "parent".to_string(),
            weight: BigDecimal::from(0),
            creation_time: Utc::now().naive_utc(),
            epoch: Utc::now().naive_utc(),
            flags: BigDecimal::from(0),
            miner: "miner".to_string(),
            nonce: BigDecimal::from(0),
            payload: "payload".to_string(),
            pow_hash: "".to_string(),
            predicate: "predicate".to_string(),
            target: BigDecimal::from(1),
        }
    }

    #[test]
    fn test_fill_gaps() {}

    #[test]
    fn test_find_gaps_returns_an_empty_vec_if_no_gaps_were_found() {
        dotenvy::from_filename(".env.test").ok();
        let pool = db::initialize_db_pool();
        let blocks = BlocksRepository { pool: pool.clone() };
        blocks.delete_all().unwrap();
        blocks
            .insert_batch(&vec![
                make_block(0, 0),
                make_block(0, 1),
                make_block(0, 2),
                make_block(0, 3),
                make_block(0, 4),
                make_block(0, 5),
            ])
            .unwrap();

        let chain = ChainId(0);
        assert!(find_gaps(&chain, &blocks).is_ok_and(|result| result.is_empty()));
    }
    #[test]
    fn test_find_gaps() {
        dotenvy::from_filename(".env.test").ok();
        let pool = db::initialize_db_pool();
        let blocks = BlocksRepository { pool: pool.clone() };
        blocks.delete_all().unwrap();
        blocks
            .insert_batch(&vec![
                make_block(0, 0),
                make_block(0, 1),
                make_block(0, 2),
                make_block(0, 4),
                make_block(0, 5),
                make_block(0, 9),
                make_block(0, 10),
            ])
            .unwrap();
        let gaps = find_gaps(&ChainId(0), &blocks);
        assert!(gaps.is_ok());
        let gaps_heights = gaps
            .unwrap()
            .iter()
            .map(|(a, b)| (a.height, b.height))
            .collect::<Vec<_>>();
        assert!(gaps_heights == vec![(5, 9), (2, 4)]);
    }

    #[test]
    fn test_find_gaps_in_range() {
        dotenvy::from_filename(".env.test").ok();
        let pool = db::initialize_db_pool();
        let blocks = BlocksRepository { pool: pool.clone() };
        blocks.delete_all().unwrap();
        blocks
            .insert_batch(&vec![
                make_block(0, 0),
                make_block(0, 1),
                make_block(0, 2),
                make_block(0, 4),
                make_block(0, 5),
                make_block(0, 6),
                make_block(0, 8),
                make_block(0, 10),
            ])
            .unwrap();

        let chain = ChainId(0);
        let gaps = find_gaps_in_range(0, 10, chain.0 as i64, &blocks);
        let gaps_heights = gaps
            .unwrap()
            .iter()
            .map(|(a, b)| (a.height, b.height))
            .collect::<Vec<_>>();
        assert!(gaps_heights == vec![(8, 10), (6, 8), (2, 4)]);
    }
}
