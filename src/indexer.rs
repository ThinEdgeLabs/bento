use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use futures::stream;
use futures::StreamExt;
use serde_json::Value;
use std::error::Error;
use std::str::FromStr;

use crate::chainweb_client::tx_result::PactTransactionResult;
use crate::chainweb_client::{
    get_block_headers_branches, get_block_payload_batch, get_cut, poll, BlockHash, BlockHeader,
    BlockPayload, Bounds, ChainId, Command, Hash, Payload, SignedTransaction,
};
use crate::models::*;
use crate::repository::*;

pub struct Indexer<'a> {
    pub blocks: &'a BlocksRepository<'a>,
    pub events: &'a EventsRepository<'a>,
    pub transactions: &'a TransactionsRepository<'a>,
}

impl<'a> Indexer<'a> {
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let cut = get_cut().await.unwrap();
        stream::iter(cut.hashes)
            .map(|(chain, last_block_hash)| async move {
                self.index_chain(&last_block_hash, &chain).await
            })
            .buffer_unordered(2)
            .collect::<Vec<Result<(), Box<dyn Error>>>>()
            .await;
        Ok(())
    }

    async fn index_chain(
        &self,
        last_block_hash: &BlockHash,
        chain: &ChainId,
    ) -> Result<(), Box<dyn Error>> {
        log::info!(
            "Syncing chain: {}, current height: {}, last block hash: {}",
            chain.0,
            last_block_hash.height,
            last_block_hash.hash
        );
        let bounds = Bounds {
            lower: vec![],
            upper: vec![Hash(last_block_hash.hash.to_string())],
        };
        let mut next: Option<String> = None;
        use std::time::Instant;
        loop {
            let before = Instant::now();
            let headers_response = get_block_headers_branches(&chain, &bounds, &next)
                .await
                .unwrap();
            log::debug!("Received headers: {:#?}", headers_response);
            log::info!("Elapsed time to get headers: {:.2?}", before.elapsed());
            match headers_response.next {
                Some(next_cursor) => {
                    log::info!(
                        "Current height: {}",
                        headers_response.items.last().unwrap().height
                    );
                    next = Some(next_cursor);
                }
                None => return Ok(()),
            }

            let before_payloads = Instant::now();
            let payloads = get_block_payload_batch(
                &chain,
                headers_response
                    .items
                    .iter()
                    .map(|e| e.payload_hash.as_str())
                    .collect::<Vec<&str>>(),
            )
            .await
            .unwrap();
            log::info!(
                "Elapsed time to get payloads: {:.2?}",
                before_payloads.elapsed()
            );

            log::debug!("Received blocks payloads: {:#?}", payloads);
            let headers_by_payload_hash = headers_response
                .items
                .iter()
                .map(|e| (e.payload_hash.clone(), e))
                .collect::<std::collections::HashMap<String, &BlockHeader>>();
            let payloads_by_hash = payloads
                .iter()
                .map(|e| (e.payload_hash.clone(), e))
                .collect::<std::collections::HashMap<String, &BlockPayload>>();

            match self.blocks.insert_batch(
                &headers_by_payload_hash
                    .into_iter()
                    .map(|(payload_hash, header)| {
                        build_block(&header, &payloads_by_hash.get(&payload_hash).unwrap())
                    })
                    .collect::<Vec<Block>>(),
            ) {
                Ok(_) => log::info!("Inserted blocks"),
                Err(e) => log::error!("Error inserting blocks: {:#?}", e),
            }

            let signed_txs = get_signed_txs_from_payloads(&payloads);
            let signed_txs_by_hash = signed_txs
                .iter()
                .map(|e| (e.hash.to_string(), e))
                .collect::<std::collections::HashMap<String, &SignedTransaction>>(
            );
            log::info!("Fetching {} transactions", signed_txs.len());
            let before_transactions = Instant::now();
            let results = fetch_transactions_results(
                &signed_txs_by_hash.keys().map(|e| e.to_string()).collect(),
                chain,
            )
            .await;
            log::info!(
                "Elapsed time to fetch transactions: {:.2?}",
                before_transactions.elapsed()
            );

            match results {
                Ok(results) => {
                    log::info!("Received {} transactions", results.len());
                    let before_txs = Instant::now();
                    let txs: Vec<Transaction> = results
                        .iter()
                        .map(|pact_result| {
                            let signed_tx =
                                signed_txs_by_hash.get(&pact_result.request_key).unwrap();
                            build_transaction(&signed_tx, &pact_result)
                        })
                        .collect();
                    log::info!("{} transactions are continuations", txs.len());
                    match self.transactions.insert_batch(&txs) {
                        Ok(inserted) => log::info!("Inserted {} transactions", inserted.len()),
                        Err(e) => log::error!("Error inserting transactions: {:#?}", e),
                    }
                    log::info!(
                        "Elapsed time to insert transactions: {:.2?}",
                        before_txs.elapsed()
                    );
                    let before_events = Instant::now();
                    let events: Vec<Event> = results
                        .iter()
                        .map(|pact_result| {
                            let signed_tx =
                                signed_txs_by_hash.get(&pact_result.request_key).unwrap();
                            build_events(&signed_tx, &pact_result)
                        })
                        .flatten()
                        .collect();
                    match self.events.insert_batch(&events) {
                        Ok(inserted) => log::info!("Inserted {} events", inserted.len()),
                        Err(e) => log::error!("Error inserting events: {:#?}", e),
                    }
                    log::info!(
                        "Elapsed time to insert events: {:.2?}",
                        before_events.elapsed()
                    );
                }
                Err(e) => log::error!("Error fetching transactions: {:#?}", e),
            }
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
        .flatten()
        .collect::<Vec<SignedTransaction>>()
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
        weight: BigDecimal::from_str(&header.weight)
            .or::<bigdecimal::ParseBigDecimalError>(Ok(BigDecimal::from(0)))
            .unwrap(),
        creation_time: NaiveDateTime::from_timestamp_micros(header.creation_time).unwrap(),
        epoch: NaiveDateTime::from_timestamp_micros(header.epoch_start).unwrap(),
        flags: BigDecimal::from(header.feature_flags),
        miner: miner_data["account"].to_string(),
        nonce: BigDecimal::from_str(&header.nonce).unwrap(),
        payload: block_payload.payload_hash.clone(),
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
    let (code, data, proof) = match command.payload {
        Payload::Exec(value) => (Some(value.code), Some(value.data), None),
        Payload::Cont(value) => (None, Some(value.data), Some(value.proof)),
    };

    Transaction {
        bad_result: pact_result.result.error.clone(),
        block: pact_result.metadata.block_hash.clone(),
        chain_id: command.meta.chain_id.parse().unwrap(),
        creation_time: NaiveDateTime::from_timestamp_micros(pact_result.metadata.block_time)
            .unwrap(),
        code: code,
        data: data,
        continuation: pact_result.continuation.clone(),
        gas: pact_result.gas,
        gas_price: command.meta.gas_price as f64,
        gas_limit: command.meta.gas_limit as i64,
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
        proof: proof,
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
    use crate::chainweb_client::{BlockPayload, Sig};

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
        let signed_txs = vec![
            SignedTransaction {
                cmd: String::from("{\"networkId\":\"mainnet01\",\"payload\":{\"exec\":{\"data\":{\"keyset\":{\"pred\":\"keys-all\",\"keys\":[\"56df77b51a5b6100dd25eb7b9cb55f3d1994f21369cb565cf9d9f7c1d630d1ef\"]}},\"code\":\"(free.radio02.add-received \\\"30ae7bfffee347e6\\\" \\\"U2FsdGVkX1/96zcn8NhZ3ih4dRhy0Thvm72dnl7HKAI=;;;;;qTUcRG54XW+vRuO+ttj+iaxOwojNSIwCZCXtufJVFfPDbkVvLbY885sD0GY+7rlNjZyfprGhWfTthAOP9bq8Io/5yxu888zPFZdfQD1ngVrk0RzhZ3Ac2HtJXtGBJUKr21j/T5d//WBTgCmtXIi+GvqH2Nrhq6PuVZmyvlTYSP8=\\\" )\"}},\"signers\":[{\"pubKey\":\"56df77b51a5b6100dd25eb7b9cb55f3d1994f21369cb565cf9d9f7c1d630d1ef\"}],\"meta\":{\"creationTime\":1687691365,\"ttl\":28800,\"gasLimit\":1000,\"chainId\":\"0\",\"gasPrice\":0.000001,\"sender\":\"k:56df77b51a5b6100dd25eb7b9cb55f3d1994f21369cb565cf9d9f7c1d630d1ef\"},\"nonce\":\"\\\"2023-06-25T11:09:44.635Z\\\"\"}"),
                hash: String::from("gaD_OZdL3cJKGelC73laoBDJjWJTkstkkjIAIKOOq1U"),
                sigs: vec![Sig { sig: String::from("328aa76e9f04055e7a0a47318030721508f23ac9b14689a6ce0e60b63be4226cafcb3d421138349ae6adad4610f30460041da40df22d461549892560355df102")}]
            },
            SignedTransaction {
                cmd: String::from("{\"networkId\":\"mainnet01\",\"payload\":{\"exec\":{\"data\":{\"keyset\":{\"pred\":\"keys-all\",\"keys\":[\"7c51dd668165d5cd8b0a7a11141bc1ec981f3fed008f554c67174c04b87b7a9c\"]}},\"code\":\"(free.radio02.update-sent \\\"U2FsdGVkX19/DOLIhAyW0TzeK0f3H14qzkYV8q7BPHs=;;;;;FEhcJxTWnOHbi1MeDBuYiOfbhFbqzzUAsOZGsmUpt6kIlMCdGF8ory0xFgAfBhnISHL0DghsWVV46aEmY+c3X/zuSk/eNnWxECmRGW7/szC7VY+2x7FxOW+9cOyp0htVw6St7kwTAMMZFBuH0bCRllgefpgRWlS2X+DUDdmdxQo=\\\" )\"}},\"signers\":[{\"pubKey\":\"7c51dd668165d5cd8b0a7a11141bc1ec981f3fed008f554c67174c04b87b7a9c\"}],\"meta\":{\"creationTime\":1687691373,\"ttl\":28800,\"gasLimit\":7000,\"chainId\":\"0\",\"gasPrice\":0.000001,\"sender\":\"k:7c51dd668165d5cd8b0a7a11141bc1ec981f3fed008f554c67174c04b87b7a9c\"},\"nonce\":\"\\\"2023-06-25T11:09:47.940Z\\\"\"}"),
                hash: String::from("tdZsPK1KjFEwn3Fmm3tTb6DK5XulN1p_ZNzq24pvxfw"),
                sigs: vec![Sig { sig: String::from("43f1212465bdbc41bf0216c26ba332805fa2ad618a20fe65bd4efb559902af69b0c8bed440287c343ffe38ee66b3bf6a1bd376b5781055b92a71fc610304740a")}]
            },
        ];
        assert_eq!(get_signed_txs_from_payloads(&vec![payload]), signed_txs);
    }
}
