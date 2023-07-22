use std::vec;

use futures::{stream, StreamExt};

use crate::chainweb_client::{self, Bounds, ChainId, Hash};
use crate::indexer::Indexer;
use crate::{db::DbError, repository::BlocksRepository};

pub async fn fill_gaps(
    blocks_repo: &BlocksRepository,
    indexer: &Indexer,
) -> Result<(), Box<dyn std::error::Error>> {
    let cut = chainweb_client::get_cut().await.unwrap();

    let gaps = cut
        .hashes
        .iter()
        .map(|(chain, last_block_hash)| {
            let gaps = find_gaps(&chain, blocks_repo).unwrap();
            let missing_blocks = gaps
                .iter()
                .map(|gap| gap.1 - gap.0 - 1)
                .reduce(|acc, e| acc + e)
                .unwrap();
            log::info!("Chain {}, is missing {} blocks.", chain, missing_blocks);
            let bounds = Bounds {
                lower: vec![],
                upper: vec![Hash(last_block_hash.hash.clone())],
            };
            (chain, bounds, gaps)
        })
        .collect::<Vec<(&ChainId, Bounds, Vec<(i64, i64)>)>>();

    for el in gaps {
        let (chain, bounds, gaps) = el;
        log::info!("Filling {} gaps for chain: {:?}", gaps.len(), chain);
        gaps.iter()
            .for_each(|e| log::info!("Gap: {:?}, size: {}", e, e.1 - e.0 - 1));
        stream::iter(gaps)
            .map(|gap| (chain.clone(), bounds.clone(), gap))
            .map(|(chain, bounds, gap)| async move {
                let response = chainweb_client::get_block_headers_branches(
                    &chain,
                    &bounds,
                    &None,
                    &Some(gap.0 as u64),
                    &Some(gap.1 as u64),
                )
                .await
                .unwrap();
                log::info!("Filling gap: {:?}", gap);
                indexer.process_headers(response.items, &chain).await
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

#[allow(dead_code)]
pub fn find_gaps(
    chain_id: &ChainId,
    repository: &BlocksRepository,
) -> Result<Vec<(i64, i64)>, DbError> {
    let count = repository.count(chain_id.0 as i64).unwrap();
    if count == 0 {
        return Ok(vec![]);
    }
    match repository
        .find_min_max_height_blocks(chain_id.0 as i64)
        .unwrap()
    {
        (Some(min_block), Some(max_block)) => {
            // Chains 0-9 have height 0 for genesis block, 10 to 19 were added later (height 852054)
            let no_gaps = if min_block.height == 0 {
                max_block.height + 1 == count
            } else {
                max_block.height - min_block.height + 1 == 0
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
/// Once a gap is found continue looking for gaps in the remaining blocks by checking if there are any in the first place
/// And then finding them
fn find_gaps_in_range(
    min_height: i64,
    max_height: i64,
    chain: i64,
    repository: &BlocksRepository,
) -> Result<Vec<(i64, i64)>, DbError> {
    let batch_size = 100;
    let mut max_height = max_height;
    let mut gaps: Vec<(i64, i64)> = vec![];
    let mut last_height = None;
    loop {
        if max_height <= min_height {
            return Ok(gaps);
        }
        let blocks = repository.find_by_range(max_height - batch_size, max_height, chain)?;
        if blocks.is_empty() {
            max_height -= batch_size;
            continue;
        }
        let mut previous_height = if last_height.is_none() {
            blocks.last().unwrap().height
        } else {
            last_height.unwrap()
        };

        for block in blocks.iter() {
            if previous_height - block.height > 1 {
                gaps.push((block.height, previous_height));
            }
            previous_height = block.height;
        }

        max_height -= batch_size;
        last_height = Some(previous_height);
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
        assert!(find_gaps(&ChainId(0), &blocks).is_ok_and(|gaps| gaps == vec![(5, 9), (2, 4)]));
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
        assert!(find_gaps_in_range(0, 10, chain.0 as i64, &blocks)
            .is_ok_and(|gaps| gaps == vec![(8, 10), (6, 8), (2, 4)]));
    }
}
