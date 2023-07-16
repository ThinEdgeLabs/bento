use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use futures::stream;
use futures::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use std::time::Instant;
use std::vec;

use crate::db::DbError;

use super::chainweb_client;
use super::chainweb_client::{
    tx_result::PactTransactionResult, BlockHeader, BlockPayload, Bounds, ChainId, Command, Cut,
    Hash, Payload, SignedTransaction,
};
use super::models::*;
use super::repository::*;

pub struct Indexer {
    pub blocks: BlocksRepository,
    pub events: EventsRepository,
    pub transactions: TransactionsRepository,
}

impl Indexer {
    pub async fn backfill(&self) -> Result<(), Box<dyn Error>> {
        let cut = chainweb_client::get_cut().await.unwrap();
        let bounds: Vec<(ChainId, Bounds)> = self.get_all_bounds(&cut);
        stream::iter(bounds)
            .map(|(chain, bounds)| async move { self.index_chain(bounds, &chain).await })
            .buffer_unordered(4)
            .collect::<Vec<Result<(), Box<dyn Error>>>>()
            .await;
        Ok(())
    }

    pub async fn index_chain(&self, bounds: Bounds, chain: &ChainId) -> Result<(), Box<dyn Error>> {
        log::info!("Indexing chain: {}, bounds: {:?}", chain.0, bounds);
        let mut next_bounds = bounds;
        loop {
            let before = Instant::now();
            let response = chainweb_client::get_block_headers_branches(chain, &next_bounds, &None)
                .await
                .unwrap();
            match response.items[..] {
                [] => return Ok(()),
                _ => {
                    log::info!(
                        "Chain {}: retrieved {} blocks, between heights {} and {}",
                        chain.0,
                        response.items.len(),
                        response.items.first().unwrap().height,
                        response.items.last().unwrap().height
                    );

                    let previous_bounds = next_bounds.clone();
                    next_bounds = Bounds {
                        upper: vec![Hash(response.items.last().unwrap().hash.to_string())],
                        ..next_bounds
                    };
                    if next_bounds == previous_bounds {
                        log::info!("Chain {}: fetched all blocks within given bounds.", chain.0);
                        return Ok(());
                    }
                }
            }
            self.process_headers(response.items, chain).await?;
            log::info!(
                "Chain {}, elapsed time per batch: {:.2?}",
                chain.0,
                before.elapsed()
            );
        }
    }
    fn get_all_bounds(&self, cut: &Cut) -> Vec<(ChainId, Bounds)> {
        let mut bounds: Vec<(ChainId, Bounds)> = vec![];
        cut.hashes.iter().for_each(|(chain, last_block_hash)| {
            log::info!(
                "Chain: {}, current height: {}, last block hash: {}",
                chain.0,
                last_block_hash.height,
                last_block_hash.hash
            );
            match self
                .blocks
                .find_min_max_height_blocks(chain.0 as i64)
                .unwrap()
            {
                (Some(min_block), Some(max_block)) => {
                    bounds.push((
                        chain.clone(),
                        Bounds {
                            lower: vec![Hash(max_block.hash)],
                            upper: vec![Hash(last_block_hash.hash.to_string())],
                        },
                    ));
                    if min_block.height > 0 {
                        bounds.push((
                            chain.clone(),
                            Bounds {
                                lower: vec![],
                                upper: vec![Hash(min_block.hash)],
                            },
                        ));
                    }
                }
                (None, None) => bounds.push((
                    chain.clone(),
                    Bounds {
                        lower: vec![],
                        upper: vec![Hash(last_block_hash.hash.to_string())],
                    },
                )),
                _ => {}
            }
        });
        bounds
    }

    async fn process_headers(
        &self,
        headers: Vec<BlockHeader>,
        chain_id: &ChainId,
    ) -> Result<(), Box<dyn Error>> {
        let payloads = chainweb_client::get_block_payload_batch(
            chain_id,
            headers
                .iter()
                .map(|e| e.payload_hash.as_str())
                .collect::<Vec<&str>>(),
        )
        .await
        .unwrap();

        match self.save_blocks(&headers, &payloads) {
            Ok(_) => {}
            Err(e) => panic!("Error inserting blocks: {:#?}", e),
        }

        let signed_txs_by_hash = get_signed_txs_from_payloads(&payloads);
        let request_keys: Vec<String> = signed_txs_by_hash.keys().map(|e| e.to_string()).collect();
        let tx_results = fetch_transactions_results(&request_keys[..], chain_id).await?;
        let txs = get_transactions_from_payload(&signed_txs_by_hash, &tx_results, chain_id);
        if txs.len() > 0 {
            match self.transactions.insert_batch(&txs) {
                Ok(inserted) => log::info!("Inserted {} transactions", inserted),
                Err(e) => panic!("Error inserting transactions: {:#?}", e),
            }
            let events = get_events_from_txs(&tx_results, &signed_txs_by_hash);
            if events.len() > 0 {
                match self.events.insert_batch(&events) {
                    Ok(inserted) => log::info!("Inserted {} events", inserted),
                    Err(e) => panic!("Error inserting events: {:#?}", e),
                }
            }
        }
        Ok(())
    }

    async fn process_header(
        &self,
        header: &BlockHeader,
        chain_id: &ChainId,
    ) -> Result<(), Box<dyn Error>> {
        let payloads =
            chainweb_client::get_block_payload_batch(chain_id, vec![header.payload_hash.as_str()])
                .await?;
        if payloads.is_empty() {
            log::error!(
                "No payload received from node, payload hash: {}, height: {}, chain: {}",
                header.payload_hash,
                header.height,
                chain_id.0
            );
            //TODO: Should we retry here?
            return Err("Unable to retrieve payload".into());
        }
        match self.save_block(&header, &payloads[0]) {
            Err(e) => {
                log::error!("Error saving block: {:#?}", e);
                return Err(e);
            }
            Ok(block) => block,
        };

        let signed_txs_by_hash = get_signed_txs_from_payload(&payloads[0]);
        let request_keys: Vec<String> = signed_txs_by_hash.keys().map(|e| e.to_string()).collect();
        let tx_results = fetch_transactions_results(&request_keys[..], chain_id).await?;
        let txs = get_transactions_from_payload(&signed_txs_by_hash, &tx_results, chain_id);
        match self.transactions.insert_batch(&txs) {
            Ok(inserted) => {
                if inserted > 0 {
                    log::info!("Inserted {} transactions", inserted)
                }
            }
            Err(e) => panic!("Error inserting transactions: {:#?}", e),
        }

        let events = get_events_from_txs(&tx_results, &signed_txs_by_hash);
        match self.events.insert_batch(&events) {
            Ok(inserted) => {
                if inserted > 0 {
                    log::info!("Inserted {} events", inserted)
                }
            }
            Err(e) => panic!("Error inserting events: {:#?}", e),
        }
        Ok(())
    }

    pub async fn listen_headers_stream(&self) -> Result<(), Box<dyn Error>> {
        use crate::chainweb_client::BlockHeaderEvent;
        use eventsource_client as es;
        use futures::stream::TryStreamExt;

        match chainweb_client::start_headers_stream() {
            Ok(stream) => {
                log::info!("Stream started");
                match stream
                    .try_for_each_concurrent(4, |event| async move {
                        if let es::SSE::Event(ev) = event {
                            if ev.event_type == "BlockHeader" {
                                let block_header_event: BlockHeaderEvent =
                                    serde_json::from_str(&ev.data).unwrap();
                                let chain_id = block_header_event.header.chain_id.clone();
                                log::info!(
                                    "Received chain {} header at height {}",
                                    chain_id,
                                    block_header_event.header.height
                                );
                                match self
                                    .process_header(&block_header_event.header, &chain_id)
                                    .await
                                {
                                    Ok(_) => {}
                                    Err(e) => log::error!("Error processing headers: {:#?}", e),
                                }
                            }
                        }
                        Ok(())
                    })
                    .await
                {
                    Ok(_) => {
                        log::info!("Headers stream ended");
                        Ok(())
                    }
                    Err(_) => Err("Stream error".into()),
                }
            }
            Err(e) => {
                log::error!("Stream error: {:?}", e);
                Err("Error".into())
            }
        }
    }

    /// Builds the list of blocks from the given headers and payloads
    /// and inserts them in the database in a single transaction.
    fn save_blocks(
        &self,
        headers: &Vec<BlockHeader>,
        payloads: &Vec<BlockPayload>,
    ) -> Result<Vec<Block>, DbError> {
        let headers_by_payload_hash = headers
            .iter()
            .map(|e| (e.payload_hash.clone(), e))
            .collect::<HashMap<String, &BlockHeader>>();
        let payloads_by_hash = payloads
            .iter()
            .map(|e| (e.payload_hash.clone(), e))
            .collect::<HashMap<String, &BlockPayload>>();
        let blocks = headers_by_payload_hash
            .into_iter()
            .map(|(payload_hash, header)| {
                build_block(header, payloads_by_hash.get(&payload_hash).unwrap())
            })
            .collect::<Vec<Block>>();
        self.blocks.insert_batch(&blocks)
    }

    fn save_block(&self, header: &BlockHeader, payload: &BlockPayload) -> Result<Block, DbError> {
        use diesel::result::DatabaseErrorKind;
        use diesel::result::Error::DatabaseError;
        let block = build_block(header, payload);
        match self.blocks.insert(&block) {
            Ok(_) => Ok(block),
            Err(e) => match e.downcast_ref() {
                Some(DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
                    log::info!("Block already exists");
                    let orphan = self
                        .blocks
                        .find_by_height(block.height, block.chain_id)
                        .unwrap()
                        .unwrap();
                    self.events.delete_all_by_block(&orphan.hash).unwrap();
                    self.transactions.delete_all_by_block(&orphan.hash).unwrap();
                    self.blocks
                        .delete_one(block.height, block.chain_id)
                        .unwrap();
                    self.blocks.insert(&block)
                }
                _ => Err(e),
            },
        }
    }
}

fn get_signed_txs_from_payload(payload: &BlockPayload) -> HashMap<String, SignedTransaction> {
    payload
        .transactions
        .iter()
        .map(|tx| {
            serde_json::from_slice::<SignedTransaction>(&base64_url::decode(&tx).unwrap()).unwrap()
        })
        .map(|tx| (tx.hash.clone(), tx))
        .collect::<HashMap<String, SignedTransaction>>()
}

fn get_signed_txs_from_payloads(payloads: &[BlockPayload]) -> HashMap<String, SignedTransaction> {
    payloads
        .iter()
        .map(get_signed_txs_from_payload)
        .filter(|e| !e.is_empty())
        .flatten()
        .collect::<HashMap<String, SignedTransaction>>()
}

fn build_block(header: &BlockHeader, block_payload: &BlockPayload) -> Block {
    let miner_data =
        serde_json::from_slice::<Value>(&base64_url::decode(&block_payload.miner_data).unwrap())
            .unwrap();
    Block {
        chain_id: header.chain_id.0 as i64,
        hash: header.hash.clone(),
        height: header.height as i64,
        parent: header.parent.clone(),
        weight: BigDecimal::from_str(&header.weight).unwrap_or(BigDecimal::from(0)),
        creation_time: NaiveDateTime::from_timestamp_micros(header.creation_time).unwrap(),
        epoch: NaiveDateTime::from_timestamp_micros(header.epoch_start).unwrap(),
        flags: header.feature_flags.clone(),
        miner: miner_data["account"].to_string(),
        nonce: BigDecimal::from_str(&header.nonce).unwrap(),
        payload: block_payload.payload_hash.clone(),
        pow_hash: "".to_string(),
        predicate: miner_data["predicate"].to_string(),
        target: bigdecimal::BigDecimal::from(1),
    }
}

fn get_transactions_from_payload(
    signed_txs: &HashMap<String, SignedTransaction>,
    tx_results: &Vec<PactTransactionResult>,
    chain_id: &ChainId,
) -> Vec<Transaction> {
    tx_results
        .iter()
        .map(|pact_result| {
            let signed_tx = signed_txs.get(&pact_result.request_key).unwrap();
            build_transaction(signed_tx, pact_result, chain_id)
        })
        .collect()
}

fn build_transaction(
    signed_tx: &SignedTransaction,
    pact_result: &PactTransactionResult,
    chain: &ChainId,
) -> Transaction {
    let continuation = pact_result.continuation.clone();
    let command = serde_json::from_str::<Command>(&signed_tx.cmd);
    match &command {
        Ok(_) => (),
        Err(e) => {
            log::info!("Error parsing command: {:#?}", signed_tx);
            panic!("{:#?}", e);
        }
    }
    let command = command.unwrap();
    let (code, data, proof) = match command.payload {
        Payload {
            exec: Some(value),
            cont: None,
        } => (Some(value.code), Some(value.data), None),
        Payload {
            exec: None,
            cont: Some(value),
        } => (None, Some(value.data), Some(value.proof)),
        _ => (None, None, None),
    };

    return Transaction {
        bad_result: pact_result.result.error.clone(),
        block: pact_result.metadata.block_hash.clone(),
        chain_id: chain.0 as i64,
        creation_time: NaiveDateTime::from_timestamp_micros(pact_result.metadata.block_time)
            .unwrap(),
        code,
        data,
        continuation: pact_result.continuation.clone(),
        gas: pact_result.gas,
        gas_price: command.meta.gas_price,
        gas_limit: command.meta.gas_limit,
        good_result: pact_result.result.data.clone(),
        height: pact_result.metadata.block_height,
        logs: if pact_result.logs.is_empty() {
            None
        } else {
            Some(pact_result.logs.to_string())
        },
        metadata: Some(serde_json::to_value(&pact_result.metadata).unwrap()),
        nonce: command.nonce,
        num_events: pact_result.events.as_ref().map(|e| e.len() as i64),
        pact_id: continuation.clone().map(|e| e["pactId"].to_string()),
        proof: proof.flatten(),
        request_key: pact_result.request_key.to_string(),
        rollback: continuation
            .clone()
            .map(|e| e["stepHasRollback"].as_bool().unwrap()),
        sender: command.meta.sender,
        step: continuation.map(|e| e["step"].as_i64().unwrap()),
        ttl: command.meta.ttl as i64,
        tx_id: pact_result.tx_id,
    };
}

fn get_events_from_txs(
    tx_results: &Vec<PactTransactionResult>,
    signed_txs_by_hash: &HashMap<String, SignedTransaction>,
) -> Vec<Event> {
    tx_results
        .iter()
        .flat_map(|pact_result| {
            let signed_tx = signed_txs_by_hash.get(&pact_result.request_key).unwrap();
            build_events(signed_tx, pact_result)
        })
        .collect()
}

fn build_events(
    signed_tx: &SignedTransaction,
    pact_result: &PactTransactionResult,
) -> Vec<crate::models::Event> {
    let command = serde_json::from_str::<Command>(&signed_tx.cmd).unwrap();
    let mut events = vec![];
    if pact_result.events.is_some() {
        for (i, event) in pact_result.events.as_ref().unwrap().iter().enumerate() {
            let event = crate::models::Event {
                block: pact_result.metadata.block_hash.clone(),
                chain_id: command.meta.chain_id.parse().unwrap(),
                height: pact_result.metadata.block_height,
                idx: i as i64,
                module: event.module.name.clone(),
                module_hash: "".to_string(), // TODO: Get module hash
                name: event.name.clone(),
                params: event.params.clone(),
                param_text: event.params.to_string(),
                qual_name: format!("{}.{}", event.module.name, event.name),
                request_key: pact_result.request_key.to_string(),
            };
            events.push(event);
        }
    }
    events
}

pub async fn fetch_transactions_results(
    request_keys: &[String],
    chain: &ChainId,
) -> Result<Vec<PactTransactionResult>, Box<dyn Error>> {
    let transactions_per_request = 10;
    let concurrent_requests = 40;
    let mut results: Vec<PactTransactionResult> = vec![];

    //TODO: Try to use tokio::StreamExt instead or figure out a way to return a Result
    // so we can handle errors if any of the requests fail
    futures::stream::iter(request_keys.chunks(transactions_per_request))
        .map(|chunk| async move { chainweb_client::poll(&chunk.to_vec(), chain).await })
        .buffer_unordered(concurrent_requests)
        .for_each(|result| {
            match result {
                Ok(result) => results
                    .append(&mut result.into_values().collect::<Vec<PactTransactionResult>>()),
                Err(e) => log::info!("Error: {}", e),
            }
            async {}
        })
        .await;
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        chainweb_client::{BlockPayload, Sig},
        db,
    };

    // #[test]
    // fn test_save_blocks() {
    //     dotenvy::from_filename(".env.test").ok();
    //     let pool = db::initialize_db_pool();

    //     let blocks = BlocksRepository { pool: pool.clone() };
    //     let events = EventsRepository { pool: pool.clone() };
    //     let mut transactions = TransactionsRepository { pool: pool.clone() };
    //     transactions.delete_all().unwrap();
    //     events.delete_all().unwrap();
    //     blocks.delete_all().unwrap();

    //     let indexer = Indexer {
    //         blocks: blocks,
    //         events: events,
    //         transactions: transactions,
    //     };
    //     let header = BlockHeader {
    //         creation_time: 1688902875826238,
    //         parent: "mZ3SiegRI9qBY43T3B7VQ82jY40tSgU2E9A7ZGPvXhI".to_string(),
    //         height: 3882292,
    //         hash: "_6S6n6dhjGw-vVHwIyq8Ulk8VNSlADLchRJCJg4vclM".to_string(),
    //         chain_id: ChainId(14),
    //         payload_hash: "yRHdjMjoqIeqm8K7WW1c4A77jxi8qP__4x_BjgZoFgE".to_string(),
    //         weight: "2CiW41EoGzYIeAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
    //         epoch_start: 1688901280684376,
    //         feature_flags: BigDecimal::from(0),
    //         adjacents: HashMap::from([(
    //             ChainId(15),
    //             "Z_lSTY7KrOVMHPqKhMTUCy3v3YPnljKAg16N3CX5dP8".to_string(),
    //         )]),
    //         chainweb_version: "mainnet01".to_string(),
    //         target: "hvD3dR8UooHyvbpvuIKyu0eALPNztocLHAAAAAAAAAA".to_string(),
    //         nonce: "11077503293030185962".to_string(),
    //     };
    //     let payload = BlockPayload {
    //         miner_data: "eyJhY2NvdW50IjoiazplN2Y3MTMwZjM1OWZiMWY4Yzg3ODczYmY4NThhMGU5Y2JjM2MxMDU5ZjYyYWU3MTVlYzcyZTc2MGIwNTVlOWYzIiwicHJlZGljYXRlIjoia2V5cy1hbGwiLCJwdWJsaWMta2V5cyI6WyJlN2Y3MTMwZjM1OWZiMWY4Yzg3ODczYmY4NThhMGU5Y2JjM2MxMDU5ZjYyYWU3MTVlYzcyZTc2MGIwNTVlOWYzIl19".to_string(),
    //         outputs_hash: "WrjWEw4Gj-60kcBPY3HZKTT9Gyoh0ZnAjFrL65Fc3GU".to_string(),
    //         payload_hash: "yRHdjMjoqIeqm8K7WW1c4A77jxi8qP__4x_BjgZoFgE".to_string(),
    //         transactions: vec![],
    //         transactions_hash: "9yNSeh7rTW_j1ziKYyubdYUCefnO5K63d5RfPkHQXiM".to_string()
    //     };
    //     let chain_id = header.chain_id.0 as i64;
    //     let hash = header.hash.clone();
    //     indexer.save_blocks(&vec![header], &vec![payload]).unwrap();
    //     let block = indexer
    //         .blocks
    //         .find_by_hash(&hash, chain_id)
    //         .unwrap()
    //         .unwrap();
    //     assert_eq!(block.hash, hash);
    // }

    #[test]
    fn test_save_block() {
        dotenvy::from_filename(".env.test").ok();
        let pool = db::initialize_db_pool();

        let blocks = BlocksRepository { pool: pool.clone() };
        let events = EventsRepository { pool: pool.clone() };
        let transactions = TransactionsRepository { pool: pool.clone() };
        transactions.delete_all().unwrap();
        events.delete_all().unwrap();
        blocks.delete_all().unwrap();

        let indexer = Indexer {
            blocks: blocks,
            events: events,
            transactions: transactions,
        };
        let orphan_header = BlockHeader {
            creation_time: 1688902875826238,
            parent: "mZ3SiegRI9qBY43T3B7VQ82jY40tSgU2E9A7ZGPvXhI".to_string(),
            height: 3882292,
            hash: "_6S6n6dhjGw-vVHwIyq8Ulk8VNSlADLchRJCJg4vclM".to_string(),
            chain_id: ChainId(14),
            payload_hash: "yRHdjMjoqIeqm8K7WW1c4A77jxi8qP__4x_BjgZoFgE".to_string(),
            weight: "2CiW41EoGzYIeAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            epoch_start: 1688901280684376,
            feature_flags: BigDecimal::from(0),
            adjacents: HashMap::from([(
                ChainId(15),
                "Z_lSTY7KrOVMHPqKhMTUCy3v3YPnljKAg16N3CX5dP8".to_string(),
            )]),
            chainweb_version: "mainnet01".to_string(),
            target: "hvD3dR8UooHyvbpvuIKyu0eALPNztocLHAAAAAAAAAA".to_string(),
            nonce: "11077503293030185962".to_string(),
        };
        let payload = BlockPayload {
            miner_data: "eyJhY2NvdW50IjoiazplN2Y3MTMwZjM1OWZiMWY4Yzg3ODczYmY4NThhMGU5Y2JjM2MxMDU5ZjYyYWU3MTVlYzcyZTc2MGIwNTVlOWYzIiwicHJlZGljYXRlIjoia2V5cy1hbGwiLCJwdWJsaWMta2V5cyI6WyJlN2Y3MTMwZjM1OWZiMWY4Yzg3ODczYmY4NThhMGU5Y2JjM2MxMDU5ZjYyYWU3MTVlYzcyZTc2MGIwNTVlOWYzIl19".to_string(),
            outputs_hash: "WrjWEw4Gj-60kcBPY3HZKTT9Gyoh0ZnAjFrL65Fc3GU".to_string(),
            payload_hash: "yRHdjMjoqIeqm8K7WW1c4A77jxi8qP__4x_BjgZoFgE".to_string(),
            transactions: vec![],
            transactions_hash: "9yNSeh7rTW_j1ziKYyubdYUCefnO5K63d5RfPkHQXiM".to_string()
        };
        let chain_id = orphan_header.chain_id.0 as i64;
        let hash = orphan_header.hash.clone();
        indexer.save_block(&orphan_header, &payload).unwrap();
        assert!(indexer
            .blocks
            .find_by_hash(&orphan_header.hash, chain_id)
            .unwrap()
            .is_some());
        let header = BlockHeader {
            hash: "new_hash".to_string(),
            ..orphan_header
        };
        indexer.save_block(&header, &payload).unwrap();
        let block = indexer.blocks.find_by_hash(&"new_hash", chain_id).unwrap();
        println!("block: {:#?}", block);
        assert!(block.is_some());
        let orphan_block = indexer.blocks.find_by_hash(&hash, chain_id).unwrap();
        println!("orphan_block: {:#?}", orphan_block);
        assert!(orphan_block.is_none());
        // Dealing with duplicate blocks (this only happens through the headers stream):
        // - try to insert the block
        // - if it fails, check if the block is already in the db
        // - if it is, delete the block, transactions and events
        // - insert the block again
    }

    #[test]
    fn test_get_signed_txs_from_payloads() {
        let payload = BlockPayload {
            payload_hash: String::from("OGY90QgfrgHz33lhr3szDK-MtrZTcWKtDuqMjssIyHU"),
            transactions: vec![
                String::from("eyJoYXNoIjoiZ2FEX09aZEwzY0pLR2VsQzczbGFvQkRKaldKVGtzdGtraklBSUtPT3ExVSIsInNpZ3MiOlt7InNpZyI6IjMyOGFhNzZlOWYwNDA1NWU3YTBhNDczMTgwMzA3MjE1MDhmMjNhYzliMTQ2ODlhNmNlMGU2MGI2M2JlNDIyNmNhZmNiM2Q0MjExMzgzNDlhZTZhZGFkNDYxMGYzMDQ2MDA0MWRhNDBkZjIyZDQ2MTU0OTg5MjU2MDM1NWRmMTAyIn1dLCJjbWQiOiJ7XCJuZXR3b3JrSWRcIjpcIm1haW5uZXQwMVwiLFwicGF5bG9hZFwiOntcImV4ZWNcIjp7XCJkYXRhXCI6e1wia2V5c2V0XCI6e1wicHJlZFwiOlwia2V5cy1hbGxcIixcImtleXNcIjpbXCI1NmRmNzdiNTFhNWI2MTAwZGQyNWViN2I5Y2I1NWYzZDE5OTRmMjEzNjljYjU2NWNmOWQ5ZjdjMWQ2MzBkMWVmXCJdfX0sXCJjb2RlXCI6XCIoZnJlZS5yYWRpbzAyLmFkZC1yZWNlaXZlZCBcXFwiMzBhZTdiZmZmZWUzNDdlNlxcXCIgXFxcIlUyRnNkR1ZrWDEvOTZ6Y244TmhaM2loNGRSaHkwVGh2bTcyZG5sN0hLQUk9Ozs7OztxVFVjUkc1NFhXK3ZSdU8rdHRqK2lheE93b2pOU0l3Q1pDWHR1ZkpWRmZQRGJrVnZMYlk4ODVzRDBHWSs3cmxOalp5ZnByR2hXZlR0aEFPUDlicThJby81eXh1ODg4elBGWmRmUUQxbmdWcmswUnpoWjNBYzJIdEpYdEdCSlVLcjIxai9UNWQvL1dCVGdDbXRYSWkrR3ZxSDJOcmhxNlB1VlpteXZsVFlTUDg9XFxcIiApXCJ9fSxcInNpZ25lcnNcIjpbe1wicHViS2V5XCI6XCI1NmRmNzdiNTFhNWI2MTAwZGQyNWViN2I5Y2I1NWYzZDE5OTRmMjEzNjljYjU2NWNmOWQ5ZjdjMWQ2MzBkMWVmXCJ9XSxcIm1ldGFcIjp7XCJjcmVhdGlvblRpbWVcIjoxNjg3NjkxMzY1LFwidHRsXCI6Mjg4MDAsXCJnYXNMaW1pdFwiOjEwMDAsXCJjaGFpbklkXCI6XCIwXCIsXCJnYXNQcmljZVwiOjAuMDAwMDAxLFwic2VuZGVyXCI6XCJrOjU2ZGY3N2I1MWE1YjYxMDBkZDI1ZWI3YjljYjU1ZjNkMTk5NGYyMTM2OWNiNTY1Y2Y5ZDlmN2MxZDYzMGQxZWZcIn0sXCJub25jZVwiOlwiXFxcIjIwMjMtMDYtMjVUMTE6MDk6NDQuNjM1WlxcXCJcIn0ifQ"),
                String::from("eyJoYXNoIjoidGRac1BLMUtqRkV3bjNGbW0zdFRiNkRLNVh1bE4xcF9aTnpxMjRwdnhmdyIsInNpZ3MiOlt7InNpZyI6IjQzZjEyMTI0NjViZGJjNDFiZjAyMTZjMjZiYTMzMjgwNWZhMmFkNjE4YTIwZmU2NWJkNGVmYjU1OTkwMmFmNjliMGM4YmVkNDQwMjg3YzM0M2ZmZTM4ZWU2NmIzYmY2YTFiZDM3NmI1NzgxMDU1YjkyYTcxZmM2MTAzMDQ3NDBhIn1dLCJjbWQiOiJ7XCJuZXR3b3JrSWRcIjpcIm1haW5uZXQwMVwiLFwicGF5bG9hZFwiOntcImV4ZWNcIjp7XCJkYXRhXCI6e1wia2V5c2V0XCI6e1wicHJlZFwiOlwia2V5cy1hbGxcIixcImtleXNcIjpbXCI3YzUxZGQ2NjgxNjVkNWNkOGIwYTdhMTExNDFiYzFlYzk4MWYzZmVkMDA4ZjU1NGM2NzE3NGMwNGI4N2I3YTljXCJdfX0sXCJjb2RlXCI6XCIoZnJlZS5yYWRpbzAyLnVwZGF0ZS1zZW50IFxcXCJVMkZzZEdWa1gxOS9ET0xJaEF5VzBUemVLMGYzSDE0cXprWVY4cTdCUEhzPTs7Ozs7RkVoY0p4VFduT0hiaTFNZURCdVlpT2ZiaEZicXp6VUFzT1pHc21VcHQ2a0lsTUNkR0Y4b3J5MHhGZ0FmQmhuSVNITDBEZ2hzV1ZWNDZhRW1ZK2MzWC96dVNrL2VObld4RUNtUkdXNy9zekM3VlkrMng3RnhPVys5Y095cDBodFZ3NlN0N2t3VEFNTVpGQnVIMGJDUmxsZ2VmcGdSV2xTMlgrRFVEZG1keFFvPVxcXCIgKVwifX0sXCJzaWduZXJzXCI6W3tcInB1YktleVwiOlwiN2M1MWRkNjY4MTY1ZDVjZDhiMGE3YTExMTQxYmMxZWM5ODFmM2ZlZDAwOGY1NTRjNjcxNzRjMDRiODdiN2E5Y1wifV0sXCJtZXRhXCI6e1wiY3JlYXRpb25UaW1lXCI6MTY4NzY5MTM3MyxcInR0bFwiOjI4ODAwLFwiZ2FzTGltaXRcIjo3MDAwLFwiY2hhaW5JZFwiOlwiMFwiLFwiZ2FzUHJpY2VcIjowLjAwMDAwMSxcInNlbmRlclwiOlwiazo3YzUxZGQ2NjgxNjVkNWNkOGIwYTdhMTExNDFiYzFlYzk4MWYzZmVkMDA4ZjU1NGM2NzE3NGMwNGI4N2I3YTljXCJ9LFwibm9uY2VcIjpcIlxcXCIyMDIzLTA2LTI1VDExOjA5OjQ3Ljk0MFpcXFwiXCJ9In0"),
            ],
            transactions_hash: String::from("hKek4su-RzH18nLq9EuZjGa6k7cq-p-o4-pnyd2S85U"),
            outputs_hash: String::from("7aK26TiKVzvnsjXcL0h4iWg3r6_HBmPoqNpO-o5mYcQ"),
            miner_data: String::from("eyJhY2NvdW50IjoiYzUwYjlhY2I0OWNhMjVmNTkxOTNiOTViNGUwOGU1MmUyZWM4OWZhMWJmMzA4ZTY0MzZmMzlhNDBhYzJkYzRmMyIsInByZWRpY2F0ZSI6ImtleXMtYWxsIiwicHVibGljLWtleXMiOlsiYzUwYjlhY2I0OWNhMjVmNTkxOTNiOTViNGUwOGU1MmUyZWM4OWZhMWJmMzA4ZTY0MzZmMzlhNDBhYzJkYzRmMyJdfQ"),
        };
        let signed_txs = HashMap::from([
            (String::from("gaD_OZdL3cJKGelC73laoBDJjWJTkstkkjIAIKOOq1U"), SignedTransaction {
                cmd: String::from("{\"networkId\":\"mainnet01\",\"payload\":{\"exec\":{\"data\":{\"keyset\":{\"pred\":\"keys-all\",\"keys\":[\"56df77b51a5b6100dd25eb7b9cb55f3d1994f21369cb565cf9d9f7c1d630d1ef\"]}},\"code\":\"(free.radio02.add-received \\\"30ae7bfffee347e6\\\" \\\"U2FsdGVkX1/96zcn8NhZ3ih4dRhy0Thvm72dnl7HKAI=;;;;;qTUcRG54XW+vRuO+ttj+iaxOwojNSIwCZCXtufJVFfPDbkVvLbY885sD0GY+7rlNjZyfprGhWfTthAOP9bq8Io/5yxu888zPFZdfQD1ngVrk0RzhZ3Ac2HtJXtGBJUKr21j/T5d//WBTgCmtXIi+GvqH2Nrhq6PuVZmyvlTYSP8=\\\" )\"}},\"signers\":[{\"pubKey\":\"56df77b51a5b6100dd25eb7b9cb55f3d1994f21369cb565cf9d9f7c1d630d1ef\"}],\"meta\":{\"creationTime\":1687691365,\"ttl\":28800,\"gasLimit\":1000,\"chainId\":\"0\",\"gasPrice\":0.000001,\"sender\":\"k:56df77b51a5b6100dd25eb7b9cb55f3d1994f21369cb565cf9d9f7c1d630d1ef\"},\"nonce\":\"\\\"2023-06-25T11:09:44.635Z\\\"\"}"),
                hash: String::from("gaD_OZdL3cJKGelC73laoBDJjWJTkstkkjIAIKOOq1U"),
                sigs: vec![Sig { sig: String::from("328aa76e9f04055e7a0a47318030721508f23ac9b14689a6ce0e60b63be4226cafcb3d421138349ae6adad4610f30460041da40df22d461549892560355df102")}]
            }),
            (String::from("tdZsPK1KjFEwn3Fmm3tTb6DK5XulN1p_ZNzq24pvxfw"), SignedTransaction {
                cmd: String::from("{\"networkId\":\"mainnet01\",\"payload\":{\"exec\":{\"data\":{\"keyset\":{\"pred\":\"keys-all\",\"keys\":[\"7c51dd668165d5cd8b0a7a11141bc1ec981f3fed008f554c67174c04b87b7a9c\"]}},\"code\":\"(free.radio02.update-sent \\\"U2FsdGVkX19/DOLIhAyW0TzeK0f3H14qzkYV8q7BPHs=;;;;;FEhcJxTWnOHbi1MeDBuYiOfbhFbqzzUAsOZGsmUpt6kIlMCdGF8ory0xFgAfBhnISHL0DghsWVV46aEmY+c3X/zuSk/eNnWxECmRGW7/szC7VY+2x7FxOW+9cOyp0htVw6St7kwTAMMZFBuH0bCRllgefpgRWlS2X+DUDdmdxQo=\\\" )\"}},\"signers\":[{\"pubKey\":\"7c51dd668165d5cd8b0a7a11141bc1ec981f3fed008f554c67174c04b87b7a9c\"}],\"meta\":{\"creationTime\":1687691373,\"ttl\":28800,\"gasLimit\":7000,\"chainId\":\"0\",\"gasPrice\":0.000001,\"sender\":\"k:7c51dd668165d5cd8b0a7a11141bc1ec981f3fed008f554c67174c04b87b7a9c\"},\"nonce\":\"\\\"2023-06-25T11:09:47.940Z\\\"\"}"),
                hash: String::from("tdZsPK1KjFEwn3Fmm3tTb6DK5XulN1p_ZNzq24pvxfw"),
                sigs: vec![Sig { sig: String::from("43f1212465bdbc41bf0216c26ba332805fa2ad618a20fe65bd4efb559902af69b0c8bed440287c343ffe38ee66b3bf6a1bd376b5781055b92a71fc610304740a")}]
            }),
        ]);
        assert_eq!(get_signed_txs_from_payloads(&vec![payload]), signed_txs);
    }
}
