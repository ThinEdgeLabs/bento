use std::vec;

use crate::{chainweb_client::ChainId, db::DbError, repository::BlocksRepository};

pub fn fill_gaps() -> Result<(), Box<dyn std::error::Error>> {
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
    fn go(
        min_height: i64,
        max_height: i64,
        chain: i64,
        repository: &BlocksRepository,
        gaps_found: Vec<(i64, i64)>,
        last_height: Option<i64>,
        batch_size: i64,
    ) -> Result<Vec<(i64, i64)>, DbError> {
        if max_height <= min_height {
            return Ok(gaps_found);
        }
        let blocks = repository.find_by_range(max_height - batch_size, max_height, chain)?;
        if blocks.is_empty() {
            return go(
                min_height,
                max_height - batch_size,
                chain,
                repository,
                gaps_found,
                last_height,
                batch_size,
            );
        }
        let mut gaps: Vec<(i64, i64)> = vec![];
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

        go(
            min_height,
            max_height - batch_size,
            chain,
            repository,
            [gaps_found, gaps].concat(),
            Some(previous_height),
            batch_size,
        )
    }
    let gaps = go(min_height, max_height, chain, repository, vec![], None, 100)?;
    Ok(gaps)
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
