use super::models::*;
use diesel::pg::PgConnection;
use diesel::prelude::*;

pub struct BlocksRepository<'a> {
    pub db: &'a mut PgConnection,
}

impl<'a> BlocksRepository<'a> {
    pub fn find_all(&mut self) -> Result<Vec<Block>, diesel::result::Error> {
        use crate::schema::blocks::dsl::*;
        let results = blocks.select(Block::as_select()).load::<Block>(self.db)?;
        Ok(results)
    }

    pub fn insert(&mut self, block: &Block) -> Result<Block, diesel::result::Error> {
        use crate::schema::blocks::dsl::*;
        let new_block = diesel::insert_into(blocks)
            .values(block)
            .returning(Block::as_returning())
            .get_result(self.db)?;
        Ok(new_block)
    }

    pub fn delete_all(&mut self) -> Result<usize, diesel::result::Error> {
        use crate::schema::blocks::dsl::*;
        let deleted = diesel::delete(blocks).execute(self.db)?;
        Ok(deleted)
    }

    pub fn delete_one(&mut self, hash: &str) -> Result<usize, diesel::result::Error> {
        use crate::schema::blocks::dsl::*;
        let deleted = diesel::delete(blocks.filter(hash.eq(hash))).execute(self.db)?;
        Ok(deleted)
    }
}

pub struct EventsRepository<'a> {
    pub db: &'a mut PgConnection,
}

impl<'a> EventsRepository<'a> {
    pub fn find_all(&mut self) -> Result<Vec<Event>, diesel::result::Error> {
        use crate::schema::events::dsl::*;
        let results = events.select(Event::as_select()).load::<Event>(self.db)?;
        Ok(results)
    }

    pub fn insert(&mut self, event: &Event) -> Result<Event, diesel::result::Error> {
        use crate::schema::events::dsl::*;
        let new_event = diesel::insert_into(events)
            .values(event)
            .returning(Event::as_returning())
            .get_result(self.db)?;
        Ok(new_event)
    }

    pub fn delete_all(&mut self) -> Result<usize, diesel::result::Error> {
        use crate::schema::events::dsl::*;
        let deleted = diesel::delete(events).execute(self.db)?;
        Ok(deleted)
    }

    pub fn delete_one(
        &mut self,
        block: &str,
        idx: i64,
        request_key: &str,
    ) -> Result<usize, diesel::result::Error> {
        use crate::schema::events::dsl::*;
        let deleted = diesel::delete(
            events
                .filter(block.eq(block))
                .filter(idx.eq(idx))
                .filter(request_key.eq(request_key)),
        )
        .execute(self.db)?;
        Ok(deleted)
    }
}

pub struct TransactionsRepository<'a> {
    pub db: &'a mut PgConnection,
}

impl<'a> TransactionsRepository<'a> {
    pub fn find_all(&mut self) -> Result<Vec<Transaction>, diesel::result::Error> {
        use crate::schema::transactions::dsl::*;
        let results = transactions
            .select(Transaction::as_select())
            .load::<Transaction>(self.db)?;
        Ok(results)
    }

    pub fn insert(
        &mut self,
        transaction: &Transaction,
    ) -> Result<Transaction, diesel::result::Error> {
        use crate::schema::transactions::dsl::*;
        let transaction = diesel::insert_into(transactions)
            .values(transaction)
            .returning(Transaction::as_returning())
            .get_result(self.db)?;
        Ok(transaction)
    }

    pub fn delete_all(&mut self) -> Result<usize, diesel::result::Error> {
        use crate::schema::transactions::dsl::*;
        let deleted = diesel::delete(transactions).execute(self.db)?;
        Ok(deleted)
    }

    pub fn delete_one(
        &mut self,
        block: &str,
        request_key: &str,
    ) -> Result<usize, diesel::result::Error> {
        use crate::schema::transactions::dsl::*;
        let deleted = diesel::delete(
            transactions
                .filter(block.eq(block))
                .filter(request_key.eq(request_key)),
        )
        .execute(self.db)?;
        Ok(deleted)
    }
}
