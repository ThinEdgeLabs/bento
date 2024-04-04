use crate::db::{DbError, DbPool};

use super::models::Collection;
use super::models::Token;
use diesel::query_dsl::methods::FilterDsl;
use diesel::ExpressionMethods;
use diesel::RunQueryDsl;
use diesel::SelectableHelper;

#[derive(Clone)]
pub struct CollectionsRepository {
    pub pool: DbPool,
}

impl CollectionsRepository {
    #[allow(dead_code)]
    pub fn insert_one(&self, collection: &Collection) -> Result<Collection, DbError> {
        use crate::schema::marmalade_v2_collections::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let result = diesel::insert_into(marmalade_v2_collections)
            .values(collection)
            .returning(Collection::as_returning())
            .get_result(&mut conn)?;
        Ok(result)
    }

    pub fn insert_many(&self, collections: &[Collection]) -> Result<Vec<Collection>, DbError> {
        use crate::schema::marmalade_v2_collections::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let inserted = diesel::insert_into(marmalade_v2_collections)
            .values(collections)
            .on_conflict_do_nothing()
            .returning(Collection::as_returning())
            .get_results(&mut conn)?;
        Ok(inserted)
    }
}

#[derive(Clone)]
pub struct TokensRepository {
    pub pool: DbPool,
}

impl TokensRepository {
    pub fn insert_many(&self, tokens: &[Token]) -> Result<Vec<Token>, DbError> {
        use crate::schema::marmalade_v2_tokens::dsl::*;
        let mut conn = self.pool.get().unwrap();
        let inserted = diesel::insert_into(marmalade_v2_tokens)
            .values(tokens)
            .on_conflict_do_nothing()
            .returning(Token::as_returning())
            .get_results(&mut conn)?;
        Ok(inserted)
    }

    pub fn update_collection_id(
        &self,
        token_id: &String,
        collection_id: &String,
    ) -> Result<Token, DbError> {
        use crate::schema::marmalade_v2_tokens::dsl::{
            collection_id as collection_id_field, id, marmalade_v2_tokens,
        };
        let mut conn = self.pool.get().unwrap();
        let result = diesel::update(marmalade_v2_tokens.filter(id.eq(token_id)))
            .set(collection_id_field.eq(collection_id))
            .returning(Token::as_returning())
            .get_result(&mut conn)?;
        Ok(result)
    }
}
