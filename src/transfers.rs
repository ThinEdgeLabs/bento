use crate::chainweb_client;
use crate::db::DbError;
use crate::models::{Event, Transfer};
use crate::repository::{EventsRepository, TransfersRepository};
use bigdecimal::BigDecimal;
use std::str::FromStr;
use std::time::Instant;

pub async fn backfill(
    batch_size: i64,
    events_repository: &EventsRepository,
    transfers_repository: &TransfersRepository,
) -> Result<(), Box<dyn std::error::Error>> {
    let cut = chainweb_client::get_cut().await.unwrap();
    cut.hashes.iter().for_each(|e| {
        let chain_id = e.0 .0;
        log::info!("Backfilling transfers on chain {}...", chain_id);
        backfill_chain(
            chain_id as i64,
            batch_size,
            events_repository,
            transfers_repository,
        )
        .unwrap();
    });
    Ok(())
}
/// Loop through events
/// Parse event
/// Check if event is a balance transfer
/// If it is, insert into transfers table
fn backfill_chain(
    chain_id: i64,
    batch_size: i64,
    events_repository: &EventsRepository,
    transfers_repository: &TransfersRepository,
) -> Result<(), DbError> {
    let min_height = 0;
    let mut max_height = events_repository.find_max_height(chain_id)?;
    loop {
        if max_height <= min_height {
            break;
        }
        log::info!("Indexing transfers from height: {}", max_height);
        let before = Instant::now();
        let events =
            events_repository.find_by_range(max_height - batch_size, max_height, chain_id)?;
        log::info!(
            "Found {} events in {}ms",
            events.len(),
            before.elapsed().as_millis()
        );
        if events.is_empty() {
            max_height -= batch_size;
            continue;
        }
        let before = Instant::now();
        process_transfers(&events, transfers_repository)?;
        log::info!(
            "Processed {} events in {}ms",
            events.len(),
            before.elapsed().as_millis(),
        );
        max_height -= batch_size;
    }
    Ok(())
}

fn is_balance_transfer(event: &Event) -> bool {
    event.name == "TRANSFER"
}

pub fn process_transfers(
    events: &Vec<Event>,
    repository: &TransfersRepository,
) -> Result<(), DbError> {
    let transfers = events
        .iter()
        .filter(|event| is_balance_transfer(event))
        .map(|event| make_transfer(event))
        .collect::<Vec<Transfer>>();
    repository.insert_batch(&transfers)?;
    Ok(())
}

fn make_transfer(event: &Event) -> Transfer {
    let sender = event.params[0].as_str().unwrap().to_string();
    let receiver = event.params[1].as_str().unwrap().to_string();
    let amount = match event.params[2].is_number() {
        true => BigDecimal::from_str(&event.params[2].to_string()).unwrap(),
        false => match event.params[2].is_object() {
            true => BigDecimal::from_str(
                &event.params[2]
                    .as_object()
                    .unwrap()
                    .get("decimal")
                    .unwrap()
                    .as_str()
                    .unwrap(),
            )
            .unwrap_or(BigDecimal::from(0)),
            false => BigDecimal::from(0),
        },
    };
    Transfer {
        amount: amount,
        block: event.block.clone(),
        chain_id: event.chain_id,
        from_account: sender,
        height: event.height,
        idx: event.idx,
        module_hash: event.module_hash.clone(),
        module_name: event.module.clone(),
        request_key: event.request_key.clone(),
        to_account: receiver,
    }
}

// pub fn update_account_balance(
//     account: &str,
//     chain_id: i64,
//     qual_name: &str,
//     module: &str,
//     height: i64,
//     change: BigDecimal,
//     balances_repository: &BalancesRepository,
// ) -> Result<Balance, DbError> {
//     match balances_repository.find_by_account_chain_and_module(account, chain_id, module) {
//         Ok(Some(balance)) => {
//             let new_balance = Balance {
//                 amount: balance.amount + change,
//                 height,
//                 ..balance
//             };
//             balances_repository.update(&new_balance)
//         }
//         Ok(None) => {
//             let balance = Balance {
//                 account: account.to_string(),
//                 chain_id,
//                 qual_name: qual_name.to_string(),
//                 module: module.to_string(),
//                 amount: change,
//                 height,
//             };
//             balances_repository.insert(&balance)
//         }
//         Err(e) => {
//             log::error!("Error updating balance: {}", e);
//             Err(e)
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::Block;
    use crate::repository::BlocksRepository;
    use bigdecimal::BigDecimal;
    use chrono::Utc;

    #[test]
    fn test_backfill() {
        dotenvy::from_filename(".env.test").ok();
        let pool = db::initialize_db_pool();
        let blocks_repository = BlocksRepository { pool: pool.clone() };
        let events_repository = EventsRepository { pool: pool.clone() };
        let transfers_repository = TransfersRepository { pool: pool.clone() };
        events_repository.delete_all().unwrap();
        transfers_repository.delete_all().unwrap();
        blocks_repository.delete_all().unwrap();
        let block = Block {
            chain_id: 0,
            hash: "block-0".to_string(),
            height: 0,
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
        };
        blocks_repository.insert(&block).unwrap();
        let event_1 = Event {
            block: "block-0".to_string(),
            chain_id: 0,
            height: 0,
            idx: 0,
            module: "coin".to_string(),
            module_hash: "module-hash".to_string(),
            name: "TRANSFER".to_string(),
            params: serde_json::json!(["", "alice", 100.1]),
            param_text: "param-text".to_string(),
            qual_name: "coin.TRANSFER".to_string(),
            request_key: "request-key-a".to_string(),
        };
        let event_2 = Event {
            block: "block-0".to_string(),
            chain_id: 0,
            height: 0,
            idx: 1,
            module: "coin".to_string(),
            module_hash: "module-hash".to_string(),
            name: "TRANSFER".to_string(),
            params: serde_json::json!(["alice", "bob", 10.0]),
            param_text: "param-text".to_string(),
            qual_name: "coin.TRANSFER".to_string(),
            request_key: "request-key-b".to_string(),
        };
        let block = Block {
            chain_id: 0,
            hash: "block-2".to_string(),
            height: 2,
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
        };
        blocks_repository.insert(&block).unwrap();
        let event_3 = Event {
            block: "block-2".to_string(),
            chain_id: 0,
            height: 2,
            idx: 0,
            module: "coin".to_string(),
            module_hash: "module-hash".to_string(),
            name: "TRANSFER".to_string(),
            params: serde_json::json!(["alice", "bob", 10.1]),
            param_text: "param-text".to_string(),
            qual_name: "coin.TRANSFER".to_string(),
            request_key: "request-key-b".to_string(),
        };
        let event_4 = Event {
            block: "block-2".to_string(),
            chain_id: 0,
            height: 2,
            idx: 1,
            module: "coin".to_string(),
            module_hash: "module-hash".to_string(),
            name: "TRANSFER".to_string(),
            params: serde_json::json!(["alice", "bob", 5.5]),
            param_text: "param-text".to_string(),
            qual_name: "coin.TRANSFER".to_string(),
            request_key: "request-key-b".to_string(),
        };
        events_repository
            .insert_batch(&[event_1, event_2, event_3, event_4])
            .unwrap();
        let chain_id = 0;
        let batch_size = 1;
        backfill_chain(
            chain_id,
            batch_size,
            &events_repository,
            &transfers_repository,
        )
        .unwrap();
        let bob_incoming_transfers = transfers_repository
            .find_incoming("bob", 0, "coin")
            .unwrap();
        assert!(bob_incoming_transfers.len() == 3);
        let alice_incoming_transfers = transfers_repository
            .find_incoming("alice", 0, "coin")
            .unwrap();
        assert!(alice_incoming_transfers.len() == 1);
    }

    #[test]
    fn test_make_transfer() {
        let event = Event {
            block: "block-hash".to_string(),
            chain_id: 0,
            height: 0,
            idx: 0,
            module: "coin".to_string(),
            module_hash: "module-hash".to_string(),
            name: "TRANSFER".to_string(),
            params: serde_json::json!(["bob", "alice", 100.12324354665567]),
            param_text: "param-text".to_string(),
            qual_name: "coin.TRANSFER".to_string(),
            request_key: "request-key".to_string(),
        };
        let transfer = make_transfer(&event);
        assert_eq!(
            transfer,
            Transfer {
                amount: BigDecimal::from_str("100.12324354665567").unwrap(),
                block: "block-hash".to_string(),
                chain_id: 0,
                from_account: "bob".to_string(),
                height: 0,
                idx: 0,
                module_hash: "module-hash".to_string(),
                module_name: "coin".to_string(),
                request_key: "request-key".to_string(),
                to_account: "alice".to_string(),
            }
        );

        let no_sender_event = Event {
            params: serde_json::json!(["", "alice", 10]),
            ..event.clone()
        };
        let transfer = make_transfer(&no_sender_event);
        assert_eq!(
            transfer,
            Transfer {
                amount: BigDecimal::from_str("10").unwrap(),
                block: "block-hash".to_string(),
                chain_id: 0,
                from_account: "".to_string(),
                height: 0,
                idx: 0,
                module_hash: "module-hash".to_string(),
                module_name: "coin".to_string(),
                request_key: "request-key".to_string(),
                to_account: "alice".to_string(),
            }
        );
        let no_receiver_event = Event {
            params: serde_json::json!(["bob", "", 10]),
            ..event
        };
        let transfer = make_transfer(&no_receiver_event);
        assert_eq!(
            transfer,
            Transfer {
                amount: BigDecimal::from_str("10").unwrap(),
                block: "block-hash".to_string(),
                chain_id: 0,
                from_account: "bob".to_string(),
                height: 0,
                idx: 0,
                module_hash: "module-hash".to_string(),
                module_name: "coin".to_string(),
                request_key: "request-key".to_string(),
                to_account: "".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_transfer_event_decimal() {
        let event = Event {
            block: "block-hash".to_string(),
            chain_id: 0,
            height: 0,
            idx: 0,
            module: "coin".to_string(),
            module_hash: "module-hash".to_string(),
            name: "TRANSFER".to_string(),
            params: serde_json::json!(["bob", "alice", {"decimal": "22.230409400000000000000000"}]),
            param_text: "param-text".to_string(),
            qual_name: "coin.TRANSFER".to_string(),
            request_key: "request-key".to_string(),
        };
        let transfer = make_transfer(&event);
        assert!(transfer.amount == BigDecimal::from_str("22.230409400000000000000000").unwrap());
    }

    #[test]
    /// This test is to make sure that if the amount is not a number, we default to 0
    fn test_make_transfer_when_event_has_string_as_amount() {
        let event = Event {
            block: "block-hash".to_string(),
            chain_id: 0,
            height: 0,
            idx: 0,
            module: "coin".to_string(),
            module_hash: "module-hash".to_string(),
            name: "TRANSFER".to_string(),
            params: serde_json::json!(["bob", "alice", "wrong-amount"]),
            param_text: "param-text".to_string(),
            qual_name: "coin.TRANSFER".to_string(),
            request_key: "request-key".to_string(),
        };
        let transfer = make_transfer(&event);
        assert!(transfer.amount == BigDecimal::from(0));
    }

    #[test]
    fn test_is_balance_transfer() {
        let event = Event {
            block: "block-hash".to_string(),
            chain_id: 0,
            height: 0,
            idx: 0,
            module: "coin".to_string(),
            module_hash: "module-hash".to_string(),
            name: "TRANSFER".to_string(),
            params: serde_json::json!(["bob", "alice", 100.1]),
            param_text: "param-text".to_string(),
            qual_name: "coin.TRANSFER".to_string(),
            request_key: "request-key".to_string(),
        };
        assert!(is_balance_transfer(&event));
    }
}
