use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use serde_json::Value;
use std::error::Error;
use std::str::FromStr;

use crate::chainweb_client::tx_result::PactTransactionResult;
use crate::chainweb_client::{
    get_block_headers_branches, get_block_payload_batch, get_cut, poll, BlockHash, BlockHeader,
    BlockPayload, Bounds, ChainId, Command, Hash, SignedTransaction,
};
use crate::models::*;
use crate::repository::*;

pub struct Indexer<'a> {
    pub blocks: BlocksRepository<'a>,
    pub events: EventsRepository<'a>,
    pub transactions: TransactionsRepository<'a>,
}

impl<'a> Indexer<'a> {
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let cut = get_cut().await.unwrap();
        let chain = ChainId(0);
        let last_block_hash = cut.hashes.get(&chain).unwrap();
        //TODO: Index all chains
        self.index_chain(last_block_hash, &chain).await?;
        Ok(())
    }

    async fn index_chain(
        &mut self,
        last_block_hash: &BlockHash,
        chain: &ChainId,
    ) -> Result<(), Box<dyn Error>> {
        log::info!(
            "Syncing chain: {}, current height: {}, last block hash: {}",
            chain.0,
            last_block_hash.height,
            last_block_hash.hash
        );
        let mut bounds = Bounds {
            lower: vec![],
            upper: vec![Hash(last_block_hash.hash.to_string())],
        };
        loop {
            let headers = get_block_headers_branches(&chain, &bounds).await.unwrap();
            log::debug!("Received headers: {:#?}", headers);
            match headers[..] {
                [] => return Ok(()),
                _ => {
                    log::info!("Current height: {}", headers.last().unwrap().height);
                    bounds.upper = vec![Hash(headers.last().unwrap().hash.to_string())]
                }
            }
            let payloads = get_block_payload_batch(
                &chain,
                headers
                    .iter()
                    .map(|e| e.payload_hash.as_str())
                    .collect::<Vec<&str>>(),
            )
            .await
            .unwrap();
            //TODO: Ignore/drop the first element in the payloads vector as it was already processed
            //in the previous iteration
            log::debug!("Received blocks payloads: {:#?}", payloads);
            let headers_by_payload_hash = headers
                .iter()
                .map(|e| (e.payload_hash.clone(), e))
                .collect::<std::collections::HashMap<String, &BlockHeader>>(
            );
            let payloads_by_hash = payloads
                .iter()
                .map(|e| (e.payload_hash.clone(), e))
                .collect::<std::collections::HashMap<String, &BlockPayload>>();

            for (payload_hash, header) in headers_by_payload_hash {
                let block = build_block(&header, &payloads_by_hash.get(&payload_hash).unwrap());
                match self.blocks.insert(&block) {
                    Ok(_) => log::info!("Inserted block: {:#?}", block.hash),
                    Err(e) => log::error!("Error inserting block: {:#?}", e),
                }
            }

            let signed_txs = get_signed_txs_from_payloads(&payloads);
            let signed_txs_by_hash = signed_txs
                .iter()
                .map(|e| (e.hash.to_string(), e))
                .collect::<std::collections::HashMap<String, &SignedTransaction>>(
            );
            log::info!("Fetching {} transactions", signed_txs.len());
            let results = fetch_transactions_results(
                &signed_txs_by_hash.keys().map(|e| e.to_string()).collect(),
                chain,
            )
            .await;

            match results {
                Ok(results) => {
                    log::info!("Received {} transactions", results.len());
                    for pact_result in results {
                        let signed_tx = signed_txs_by_hash.get(&pact_result.request_key).unwrap();
                        //log::info!("Processing transaction: {:#?}", signed_tx);
                        let tx = build_transaction(&signed_tx, &pact_result);

                        match self.transactions.insert(&tx) {
                            Ok(_) => log::info!("Inserted transaction: {:#?}", tx.request_key),
                            Err(e) => log::error!("Error inserting transaction: {:#?}", e),
                        }

                        let events = build_events(&signed_tx, &pact_result);
                        for event in events {
                            match self.events.insert(&event) {
                                Ok(_) => log::info!("Inserted event: {:#?}", event.request_key),
                                Err(e) => log::error!("Error inserting event: {:#?}", e),
                            }
                        }
                    }
                }
                Err(e) => log::error!("Error fetching transactions: {:#?}", e),
            }

            //TODO: Save events to DB
        }
    }
}

fn get_signed_txs_from_payloads(payloads: &Vec<BlockPayload>) -> Vec<SignedTransaction> {
    payloads
        .iter()
        .map(|e| {
            e.transactions
                .iter()
                .map(|tx| {
                    serde_json::from_slice::<SignedTransaction>(&base64_url::decode(&tx).unwrap())
                        .unwrap()
                })
                .collect::<Vec<SignedTransaction>>()
        })
        .filter(|e| !e.is_empty())
        .collect::<Vec<Vec<SignedTransaction>>>()
        .into_iter()
        .flatten()
        .collect()
}

fn build_block(header: &BlockHeader, payload: &BlockPayload) -> Block {
    let miner_data =
        serde_json::from_slice::<Value>(&base64_url::decode(&payload.miner_data).unwrap()).unwrap();
    Block {
        chain_id: header.chain_id.0 as i64,
        hash: header.hash.clone(),
        height: header.height as i64,
        parent: header.parent.clone(),
        weight: BigDecimal::from_str(&header.weight)
            .or::<bigdecimal::ParseBigDecimalError>(Ok(BigDecimal::from(0)))
            .unwrap(),
        creation_time: NaiveDateTime::from_timestamp_micros(header.creation_time).unwrap(),
        epoch: NaiveDateTime::from_timestamp_micros(header.epoch_start).unwrap(),
        flags: BigDecimal::from(header.feature_flags),
        miner: miner_data["account"].to_string(),
        nonce: BigDecimal::from_str(&header.nonce).unwrap(),
        payload: payload.payload_hash.clone(),
        pow_hash: "".to_string(),
        predicate: miner_data["predicate"].to_string(),
        target: bigdecimal::BigDecimal::from(1),
    }
}

fn build_transaction(
    signed_tx: &SignedTransaction,
    pact_result: &PactTransactionResult,
) -> Transaction {
    let continuation = pact_result.continuation.clone();
    let command = serde_json::from_str::<Command>(&signed_tx.cmd).unwrap();
    Transaction {
        bad_result: pact_result.result.error.clone(),
        block: pact_result.metadata.block_hash.clone(),
        chain_id: command.meta.chain_id.parse().unwrap(),
        creation_time: NaiveDateTime::from_timestamp_micros(pact_result.metadata.block_time)
            .unwrap(),
        code: Some("code".to_string()),
        continuation: pact_result.continuation.clone(),
        data: pact_result.result.data.clone(),
        gas: pact_result.gas,
        gas_price: command.meta.gas_price as f64,
        gas_limit: 800,
        good_result: pact_result.result.data.clone(),
        height: pact_result.metadata.block_height,
        logs: if pact_result.logs.is_empty() {
            None
        } else {
            Some(pact_result.logs.to_string())
        },
        metadata: Some(serde_json::to_value(&pact_result.metadata).unwrap()),
        nonce: command.nonce,
        num_events: Some(pact_result.events.len() as i64),
        pact_id: continuation.clone().map(|e| e["pactId"].to_string()),
        proof: None,
        request_key: pact_result.request_key.to_string(),
        rollback: continuation
            .clone()
            .map(|e| e["stepHasRollback"].as_bool().unwrap()),
        sender: command.meta.sender,
        step: continuation.map(|e| e["step"].as_i64().unwrap()),
        ttl: command.meta.ttl as i64,
        tx_id: pact_result.tx_id,
    }
}

fn build_events(
    signed_tx: &SignedTransaction,
    pact_result: &PactTransactionResult,
) -> Vec<crate::models::Event> {
    let command = serde_json::from_str::<Command>(&signed_tx.cmd).unwrap();
    let mut events = vec![];
    for (i, event) in pact_result.events.iter().enumerate() {
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
    events
}

pub async fn fetch_transactions_results(
    request_keys: &Vec<String>,
    chain: &ChainId,
) -> Result<Vec<PactTransactionResult>, Box<dyn Error>> {
    use futures::StreamExt;

    let transactions_per_request = 10;
    let concurrent_requests = 40;
    let mut results: Vec<PactTransactionResult> = vec![];

    //TODO: Try to use tokio::StreamExt instead so we can return a result
    futures::stream::iter(request_keys.chunks(transactions_per_request))
        .map(|chunk| async move {
            // match poll(&chunk.to_vec(), &chain).await {
            //     Ok(resp) => {
            //         //Ok(resp.into_values().collect::<Vec<PactTransactionResult>>())
            //     }
            //     Err(e) => {
            //         log::error!("{:#?}", e);
            //         //Err("error")
            //     }
            // }
            poll(&chunk.to_vec(), &chain).await
        })
        .buffer_unordered(concurrent_requests)
        .for_each(|result| {
            match result {
                Ok(result) => results
                    .append(&mut result.into_values().collect::<Vec<PactTransactionResult>>()),
                Err(e) => log::info!("Error: {}", e),
            }
            async { () }
        })
        .await;
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chainweb_client::BlockPayload;

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
        assert_eq!(
            get_signed_txs_from_payloads(&vec![payload]),
            vec![
                "gaD_OZdL3cJKGelC73laoBDJjWJTkstkkjIAIKOOq1U",
                "tdZsPK1KjFEwn3Fmm3tTb6DK5XulN1p_ZNzq24pvxfw"
            ]
        );
    }
}
