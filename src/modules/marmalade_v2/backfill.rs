use std::collections::HashMap;

use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;

use crate::{
    chainweb_client::ChainwebClient,
    db::{DbError, DbPool},
    models::{Block, Event},
    modules::marmalade_v2::repository::{CollectionsRepository, TokensRepository},
    repository::{BlocksRepository, EventsRepository},
};

use super::models::{Collection, Token};

pub async fn run(pool: DbPool) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Starting MarmaladeV2 backfill...");

    let batch_size = 1000;
    let chainweb_client = ChainwebClient::new();
    let blocks_repository = BlocksRepository { pool: pool.clone() };
    let events_repository = EventsRepository { pool: pool.clone() };
    let collections_repository = CollectionsRepository { pool: pool.clone() };
    let tokens_repository = TokensRepository { pool: pool.clone() };

    let cut = chainweb_client.get_cut().await.unwrap();
    cut.hashes.iter().for_each(|e| {
        let chain_id = e.0 .0;
        backfill(
            chain_id as i64,
            batch_size,
            &events_repository,
            &blocks_repository,
            &collections_repository,
            &tokens_repository,
            None,
        )
        .unwrap();
    });
    Ok(())
}

fn backfill(
    chain_id: i64,
    batch_size: i64,
    events_repository: &EventsRepository,
    blocks_repository: &BlocksRepository,
    collections_repository: &CollectionsRepository,
    tokens_repository: &TokensRepository,
    starting_max_height: Option<i64>,
) -> Result<(), DbError> {
    log::info!("Backfilling chain {}", chain_id);
    let result = blocks_repository.find_min_max_height_blocks(chain_id)?;
    if result.is_none() {
        log::info!("No blocks found for chain {}", chain_id);
        return Ok(());
    }
    let (min_height_block, max_height_block) = result.unwrap();
    let mut current_height = min_height_block.height;
    loop {
        if current_height > max_height_block.height {
            break;
        }
        let mut blocks = blocks_repository.find_by_range(
            current_height,
            current_height + batch_size,
            chain_id,
        )?;
        // find_by_range returns blocks in descending order but we want to process them in ascending order
        blocks.reverse();
        if blocks.is_empty() {
            current_height += batch_size + 1;
            continue;
        }
        let events = events_repository.find_by_blocks(&blocks)?;
        process_events(
            &events,
            &blocks,
            &collections_repository,
            &tokens_repository,
        )?;
        current_height = blocks.last().unwrap().height + 1;
    }
    Ok(())
}

fn process_events(
    events: &[Event],
    blocks: &[Block],
    collections_repository: &CollectionsRepository,
    tokens_repository: &TokensRepository,
) -> Result<(), DbError> {
    let blocks_by_hash = blocks
        .iter()
        .map(|block| (block.hash.to_string(), block))
        .collect::<HashMap<String, &Block>>();

    let collections = events
        .iter()
        .filter(|event| is_collection_event(event))
        .map(|event| make_collection(event, blocks_by_hash[&event.block]))
        .collect::<Vec<Collection>>();

    collections.chunks(1000).for_each(|chunk| {
        let inserted = collections_repository.insert_many(&chunk.to_vec()).unwrap();
        log::info!("Inserted {} new collections", inserted.len());
    });

    let tokens = events
        .iter()
        .filter(|event| is_token_event(event))
        .map(|event| make_token(event, blocks_by_hash[&event.block]))
        .collect::<Vec<Token>>();
    tokens.chunks(1000).for_each(|chunk| {
        let inserted = tokens_repository.insert_many(&chunk.to_vec()).unwrap();
        log::info!("Inserted {} new tokens", inserted.len());
    });

    events
        .iter()
        .filter(|event| is_token_collection_event(event))
        .map(|event| {
            let collection_id = event.params[0].as_str().unwrap().to_string();
            let token_id = event.params[1].as_str().unwrap().to_string();
            (collection_id, token_id)
        })
        .for_each(|(collection_id, token_id)| {
            tokens_repository
                .update_collection_id(&token_id, &collection_id)
                .unwrap();
        });
    Ok(())
}

fn is_collection_event(event: &Event) -> bool {
    event.name == "COLLECTION" && event.module == "marmalade-v2.collection-policy-v1"
}

fn is_token_collection_event(event: &Event) -> bool {
    event.name == "TOKEN-COLLECTION" && event.module == "marmalade-v2.collection-policy-v1"
}

fn is_token_event(event: &Event) -> bool {
    event.name == "TOKEN" && event.module == "marmalade-v2.ledger"
}

fn make_collection(event: &Event, block: &Block) -> Collection {
    let size = match event.params[2].is_i64() {
        true => event.params[2].as_i64().unwrap(),
        false => match event.params[2].is_object() {
            true => match &event.params[2].as_object().unwrap().get("int") {
                Some(number) => number.as_i64().unwrap_or(0),
                _ => 0,
            },
            _ => 0,
        },
    };

    Collection {
        id: event.params[0].as_str().unwrap().to_string(),
        name: event.params[1].as_str().unwrap().to_string(),
        size: size as i64,
        operator_guard: event.params[3].clone(),
        chain_id: event.chain_id,
        block: block.hash.to_string(),
        request_key: event.request_key.to_string(),
        creation_time: NaiveDateTime::from_timestamp_millis(block.creation_time.timestamp_millis())
            .unwrap(),
    }
}

fn parse_pact_integer(value: &serde_json::Value) -> i64 {
    match value.is_i64() {
        true => value.as_i64().unwrap(),
        false => match value.is_object() {
            true => match &value.as_object().unwrap().get("int") {
                Some(number) => number.as_i64().unwrap_or(0),
                _ => 0,
            },
            _ => 0,
        },
    }
}

fn make_token(event: &Event, block: &Block) -> Token {
    Token {
        id: event.params[0].as_str().unwrap().to_string(),
        collection_id: None,
        chain_id: event.chain_id,
        precision: parse_pact_integer(&event.params[1]) as i32,
        uri: event.params[3].as_str().unwrap().to_string(),
        supply: BigDecimal::from(0),
        policies: event.params[2].clone(),
        block: block.hash.to_string(),
        request_key: event.request_key.to_string(),
        creation_time: NaiveDateTime::from_timestamp_millis(block.creation_time.timestamp_millis())
            .unwrap(),
    }
}
