use crate::db::DbError;

use super::db::DbPool;
use super::models::*;
use diesel::prelude::*;

#[derive(Clone)]
pub struct BlocksRepository {
    pub pool: DbPool,
}

impl BlocksRepository {
    #[allow(dead_code)]
    pub fn find_all(&self) -> Result<Vec<Block>, diesel::result::Error> {
        use crate::schema::blocks::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let results = blocks.select(Block::as_select()).load::<Block>(&mut conn)?;
        Ok(results)
    }

    #[allow(dead_code)]
    pub fn find_by_hash(
        &self,
        hash: &str,
        chain_id: i64,
    ) -> Result<Option<Block>, diesel::result::Error> {
        use crate::schema::blocks::dsl::{
            blocks as blocks_table, chain_id as chain_id_column, hash as hash_column,
        };
        let mut conn = self.pool.get().unwrap();
        let result = blocks_table
            .filter(hash_column.eq(hash))
            .filter(chain_id_column.eq(chain_id))
            .select(Block::as_select())
            .first::<Block>(&mut conn)
            .optional()?;
        Ok(result)
    }

    pub fn find_by_height(&self, height: i64, chain_id: i64) -> Result<Option<Block>, DbError> {
        use crate::schema::blocks::dsl::{
            blocks as blocks_table, chain_id as chain_id_column, height as height_column,
        };
        let mut conn = self.pool.get().unwrap();
        let result = blocks_table
            .filter(height_column.eq(height))
            .filter(chain_id_column.eq(chain_id))
            .select(Block::as_select())
            .first::<Block>(&mut conn)
            .optional()?;
        Ok(result)
    }

    pub fn find_by_range(
        &self,
        min_height: i64,
        max_height: i64,
        chain_id: i64,
    ) -> Result<Vec<Block>, DbError> {
        use crate::schema::blocks::dsl::{
            blocks as blocks_table, chain_id as chain_id_column, height as height_column,
        };
        let mut conn = self.pool.get().unwrap();
        let results = blocks_table
            .filter(height_column.ge(min_height))
            .filter(height_column.le(max_height))
            .filter(chain_id_column.eq(chain_id))
            .select(Block::as_select())
            .order(height_column.desc())
            .load::<Block>(&mut conn)?;
        Ok(results)
    }

    pub fn find_min_max_height_blocks(
        &self,
        chain_id: i64,
    ) -> Result<(Option<Block>, Option<Block>), DbError> {
        use crate::schema::blocks::dsl::{
            blocks as blocks_table, chain_id as chain_id_column, height,
        };
        let mut conn = self.pool.get().unwrap();
        let query = blocks_table.filter(chain_id_column.eq(chain_id));
        let min_block = query
            .order_by(height.asc())
            .select(Block::as_select())
            .first::<Block>(&mut conn)
            .optional()?;
        let max_block = query
            .order_by(height.desc())
            .select(Block::as_select())
            .first::<Block>(&mut conn)
            .optional()?;
        Ok((min_block, max_block))
    }

    pub fn count(&self, chain_id: i64) -> Result<i64, DbError> {
        use crate::schema::blocks::dsl::{blocks, chain_id as chain_id_col, height};
        use diesel::dsl::count;
        let mut conn = self.pool.get().unwrap();
        let count = blocks
            .select(count(height))
            .filter(chain_id_col.eq(chain_id))
            .first(&mut conn)?;
        Ok(count)
    }

    #[allow(dead_code)]
    pub fn insert(&self, block: &Block) -> Result<Block, DbError> {
        use crate::schema::blocks::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let new_block = diesel::insert_into(blocks)
            .values(block)
            .returning(Block::as_returning())
            .get_result(&mut conn)?;
        Ok(new_block)
    }

    pub fn insert_batch(&self, blocks: &Vec<Block>) -> Result<Vec<Block>, DbError> {
        use crate::schema::blocks::dsl::blocks as blocks_table;
        let mut conn = self.pool.get().unwrap();
        let inserted = diesel::insert_into(blocks_table)
            .values(blocks)
            .on_conflict_do_nothing()
            .returning(Block::as_returning())
            .get_results(&mut conn)?;
        Ok(inserted)
    }

    #[allow(dead_code)]
    pub fn delete_all(&self) -> Result<usize, diesel::result::Error> {
        use crate::schema::blocks::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(blocks).execute(&mut conn)?;
        Ok(deleted)
    }

    #[allow(dead_code)]
    pub fn delete_one(&self, height: i64, chain_id: i64) -> Result<usize, DbError> {
        use crate::schema::blocks::dsl::{
            blocks as blocks_table, chain_id as chain_id_col, height as height_col,
        };
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(
            blocks_table
                .filter(height_col.eq(height))
                .filter(chain_id_col.eq(chain_id)),
        )
        .execute(&mut conn)?;
        Ok(deleted)
    }

    pub fn delete_by_hash(&self, hash: &str, chain_id: i64) -> Result<usize, DbError> {
        use crate::schema::blocks::dsl::{
            blocks as blocks_table, chain_id as chain_id_col, hash as hash_col,
        };
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(
            blocks_table
                .filter(hash_col.eq(hash))
                .filter(chain_id_col.eq(chain_id)),
        )
        .execute(&mut conn)?;
        Ok(deleted)
    }
}

#[derive(Clone)]
pub struct EventsRepository {
    pub pool: DbPool,
}

impl EventsRepository {
    #[allow(dead_code)]
    pub fn find_all(&self) -> Result<Vec<Event>, diesel::result::Error> {
        use crate::schema::events::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let results = events.select(Event::as_select()).load::<Event>(&mut conn)?;
        Ok(results)
    }

    #[allow(dead_code)]
    pub fn insert(&self, event: &Event) -> Result<Event, diesel::result::Error> {
        use crate::schema::events::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let new_event = diesel::insert_into(events)
            .values(event)
            .returning(Event::as_returning())
            .get_result(&mut conn)?;
        Ok(new_event)
    }

    pub fn insert_batch(&self, events: &[Event]) -> Result<usize, diesel::result::Error> {
        use crate::schema::events::dsl::events as events_table;
        let mut inserted = 0;
        let mut conn = self.pool.get().unwrap();
        for chunk in events.chunks(1000) {
            inserted += diesel::insert_into(events_table)
                .values(chunk)
                .on_conflict_do_nothing()
                .execute(&mut conn)?;
        }
        Ok(inserted)
    }

    #[allow(dead_code)]
    pub fn delete_all(&self) -> Result<usize, DbError> {
        use crate::schema::events::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(events).execute(&mut conn)?;
        Ok(deleted)
    }

    #[allow(dead_code)]
    pub fn delete_one(&self, block: &str, idx: i64, request_key: &str) -> Result<usize, DbError> {
        use crate::schema::events::dsl::{
            block as block_col, events, idx as idx_col, request_key as request_key_col,
        };
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(
            events
                .filter(block_col.eq(block))
                .filter(idx_col.eq(idx))
                .filter(request_key_col.eq(request_key)),
        )
        .execute(&mut conn)?;
        Ok(deleted)
    }

    pub fn delete_all_by_block(&self, hash: &str) -> Result<usize, DbError> {
        use crate::schema::events::dsl::{block as block_col, events};
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(events.filter(block_col.eq(hash))).execute(&mut conn)?;
        Ok(deleted)
    }
}

#[derive(Clone)]
pub struct TransactionsRepository {
    pub pool: DbPool,
}

impl TransactionsRepository {
    #[allow(dead_code)]
    pub fn find_all(&self) -> Result<Vec<Transaction>, DbError> {
        use crate::schema::transactions::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let results = transactions
            .select(Transaction::as_select())
            .load::<Transaction>(&mut conn)?;
        Ok(results)
    }

    #[allow(dead_code)]
    pub fn find_by_request_key(
        &self,
        request_keys: &Vec<String>,
    ) -> Result<Vec<Transaction>, DbError> {
        use crate::schema::transactions::dsl::{
            request_key as request_key_column, transactions as transactions_table,
        };
        let mut conn = self.pool.get().unwrap();
        let result = transactions_table
            .filter(request_key_column.eq_any(request_keys))
            .select(Transaction::as_select())
            .load(&mut conn)?;
        Ok(result)
    }

    #[allow(dead_code)]
    pub fn find_all_related(
        &self,
        request_keys: &Vec<String>,
    ) -> Result<Vec<Vec<Transaction>>, DbError> {
        use itertools::Itertools;
        match self.find_by_request_key(request_keys) {
            Ok(transactions) => {
                let pact_ids = transactions
                    .iter()
                    .filter_map(|e| e.pact_id.clone())
                    .collect::<Vec<String>>();
                match self.find_by_pact_id(&pact_ids) {
                    Ok(multi_step_txs) => {
                        let mut single_step_txs = transactions
                            .iter()
                            .filter(|e| e.pact_id.is_none())
                            .map(|e| vec![e.clone()])
                            .collect::<Vec<Vec<Transaction>>>();
                        let multi_step_txs_by_pact_id = multi_step_txs
                            .into_iter()
                            .filter(|e| e.pact_id.is_some())
                            .group_by(|e| e.pact_id.clone().unwrap());
                        let mut multi_step_txs = Vec::new();
                        for (_, group) in &multi_step_txs_by_pact_id {
                            multi_step_txs.push(group.collect::<Vec<Transaction>>());
                        }
                        single_step_txs.append(&mut multi_step_txs);
                        Ok(single_step_txs)
                    }
                    Err(err) => Err(err),
                }
            }
            Err(err) => Err(err),
        }
    }

    #[allow(dead_code)]
    pub fn find_by_pact_id(&self, pact_ids: &Vec<String>) -> Result<Vec<Transaction>, DbError> {
        use crate::schema::transactions::dsl::{
            pact_id as pact_id_column, transactions as transactions_table,
        };
        let mut conn = self.pool.get().unwrap();
        let result = transactions_table
            .filter(pact_id_column.eq_any(pact_ids))
            .select(Transaction::as_select())
            .load(&mut conn)?;
        Ok(result)
    }

    #[allow(dead_code)]
    pub fn insert(&self, transaction: &Transaction) -> Result<Transaction, DbError> {
        use crate::schema::transactions::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let transaction = diesel::insert_into(transactions)
            .values(transaction)
            .returning(Transaction::as_returning())
            .get_result(&mut conn)?;
        Ok(transaction)
    }

    pub fn insert_batch(&self, transactions: &[Transaction]) -> Result<usize, DbError> {
        use crate::schema::transactions::dsl::transactions as transactions_table;
        let mut conn = self.pool.get().unwrap();
        let mut inserted = 0;
        for chunk in transactions.chunks(1000) {
            inserted += diesel::insert_into(transactions_table)
                .values(chunk)
                .on_conflict_do_nothing()
                .execute(&mut conn)?;
        }
        Ok(inserted)
    }

    #[allow(dead_code)]
    pub fn delete_all(&self) -> Result<usize, DbError> {
        use crate::schema::transactions::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(transactions).execute(&mut conn)?;
        Ok(deleted)
    }

    pub fn delete_all_by_block(&self, hash: &str) -> Result<usize, DbError> {
        use crate::schema::transactions::dsl::{block as block_col, transactions};
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(transactions.filter(block_col.eq(hash))).execute(&mut conn)?;
        Ok(deleted)
    }

    #[allow(dead_code)]
    pub fn delete_one(&self, block: &str, request_key: &str) -> Result<usize, DbError> {
        use crate::schema::transactions::dsl::{
            block as block_column, request_key as request_key_column, transactions,
        };
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(
            transactions
                .filter(block_column.eq(block))
                .filter(request_key_column.eq(request_key)),
        )
        .execute(&mut conn)?;
        Ok(deleted)
    }
}
