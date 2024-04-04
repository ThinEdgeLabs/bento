use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use diesel::{deserialize::Queryable, prelude::Insertable, Selectable};
use serde::Serialize;

#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq)]
#[diesel(table_name = crate::schema::marmalade_v2_collections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(belongs_to(Block, foreign_key = block))]
#[derive(Serialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub operator_guard: serde_json::Value,
    pub chain_id: i64,
    pub block: String,
    pub request_key: String,
    pub creation_time: NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq)]
#[diesel(belongs_to(Collection, foreign_key = collection_id))]
#[diesel(table_name = crate::schema::marmalade_v2_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[derive(Serialize)]
pub struct Token {
    pub id: String,
    pub collection_id: Option<String>,
    pub chain_id: i64,
    pub precision: i32,
    pub uri: String,
    pub supply: BigDecimal,
    pub policies: serde_json::Value,
    pub block: String,
    pub request_key: String,
    pub creation_time: NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq)]
#[diesel(table_name = crate::schema::marmalade_v2_balances)]
#[diesel(belongs_to(Token, foreign_key = token_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[derive(Serialize)]
pub struct Balance {
    pub account: String,
    pub guard: String,
    pub token_id: String,
    pub amount: BigDecimal,
    pub chain_id: i64,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq)]
#[diesel(table_name = crate::schema::marmalade_v2_activity)]
#[diesel(belongs_to(Token, foreign_key = token_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[derive(Serialize)]
pub struct ActivityEvent {
    pub token_id: String,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub creation_time: NaiveDateTime,
}
