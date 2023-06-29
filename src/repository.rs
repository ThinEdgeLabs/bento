use super::db::DbPool;
use super::models::*;
use diesel::prelude::*;

pub struct BlocksRepository<'a> {
    pub pool: &'a DbPool,
}

impl<'a> BlocksRepository<'a> {
    #[allow(dead_code)]
    pub fn find_all(&self) -> Result<Vec<Block>, diesel::result::Error> {
        use crate::schema::blocks::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let results = blocks.select(Block::as_select()).load::<Block>(&mut conn)?;
        Ok(results)
    }

    #[allow(dead_code)]
    pub fn insert(&self, block: &Block) -> Result<Block, diesel::result::Error> {
        use crate::schema::blocks::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let new_block = diesel::insert_into(blocks)
            .values(block)
            .returning(Block::as_returning())
            .get_result(&mut conn)?;
        Ok(new_block)
    }

    pub fn insert_batch(&self, blocks: &Vec<Block>) -> Result<Vec<Block>, diesel::result::Error> {
        use crate::schema::blocks::dsl::blocks as blocks_table;
        let mut conn = self.pool.get().unwrap();
        let inserted = diesel::insert_into(blocks_table)
            .values(blocks)
            .returning(Block::as_returning())
            .get_results(&mut conn)?;
        Ok(inserted)
    }

    pub fn delete_all(&self) -> Result<usize, diesel::result::Error> {
        use crate::schema::blocks::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(blocks).execute(&mut conn)?;
        Ok(deleted)
    }

    #[allow(dead_code)]
    pub fn delete_one(&self, hash: &str) -> Result<usize, diesel::result::Error> {
        use crate::schema::blocks::dsl::{blocks as blocks_table, hash as hash_column};
        let mut conn = self.pool.get().unwrap();
        let deleted =
            diesel::delete(blocks_table.filter(hash_column.eq(hash))).execute(&mut conn)?;
        Ok(deleted)
    }
}

pub struct EventsRepository<'a> {
    pub pool: &'a DbPool,
}

impl<'a> EventsRepository<'a> {
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

    pub fn insert_batch(&self, events: &Vec<Event>) -> Result<Vec<Event>, diesel::result::Error> {
        use crate::schema::events::dsl::events as events_table;
        let mut conn = self.pool.get().unwrap();
        let inserted = diesel::insert_into(events_table)
            .values(events)
            .get_results(&mut conn)?;
        Ok(inserted)
    }

    pub fn delete_all(&self) -> Result<usize, diesel::result::Error> {
        use crate::schema::events::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(events).execute(&mut conn)?;
        Ok(deleted)
    }

    #[allow(dead_code)]
    pub fn delete_one(
        &self,
        block: &str,
        idx: i64,
        request_key: &str,
    ) -> Result<usize, diesel::result::Error> {
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
}

pub struct TransactionsRepository<'a> {
    pub pool: &'a DbPool,
}

impl<'a> TransactionsRepository<'a> {
    #[allow(dead_code)]
    pub fn find_all(&self) -> Result<Vec<Transaction>, diesel::result::Error> {
        use crate::schema::transactions::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let results = transactions
            .select(Transaction::as_select())
            .load::<Transaction>(&mut conn)?;
        Ok(results)
    }

    #[allow(dead_code)]
    pub fn insert(&self, transaction: &Transaction) -> Result<Transaction, diesel::result::Error> {
        use crate::schema::transactions::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let transaction = diesel::insert_into(transactions)
            .values(transaction)
            .returning(Transaction::as_returning())
            .get_result(&mut conn)?;
        Ok(transaction)
    }

    pub fn insert_batch(
        &self,
        transactions: &Vec<Transaction>,
    ) -> Result<Vec<Transaction>, diesel::result::Error> {
        use crate::schema::transactions::dsl::transactions as transactions_table;
        let mut conn = self.pool.get().unwrap();
        let inserted = diesel::insert_into(transactions_table)
            .values(transactions)
            .returning(Transaction::as_returning())
            .get_results(&mut conn)?;
        Ok(inserted)
    }

    pub fn delete_all(&mut self) -> Result<usize, diesel::result::Error> {
        use crate::schema::transactions::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let deleted = diesel::delete(transactions).execute(&mut conn)?;
        Ok(deleted)
    }

    #[allow(dead_code)]
    pub fn delete_one(
        &self,
        block: &str,
        request_key: &str,
    ) -> Result<usize, diesel::result::Error> {
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
