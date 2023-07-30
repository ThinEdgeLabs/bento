use crate::db::DbError;
use crate::models::{Balance, Event};
use crate::repository::{BalancesRepository, EventsRepository};
use bigdecimal::BigDecimal;
use std::str::FromStr;
use std::time::Instant;

/// Loop through events
/// Parse event
/// Check if event is a balance transfer
/// If it is, update balance for sender and receiver
pub fn calculate_balances(
    chain_id: i64,
    batch_size: i64,
    starting_height: Option<i64>,
    repository: &EventsRepository,
    balances_repository: &BalancesRepository,
) -> Result<(), DbError> {
    let mut min_height = match starting_height {
        Some(height) => height,
        None => 0,
    };
    let max_height = repository.find_max_height(chain_id)?;
    loop {
        log::info!("Calculating balances from height: {}", min_height);
        if min_height > max_height {
            break;
        }
        let before = Instant::now();
        let events = repository.find_by_range(min_height, min_height + batch_size, chain_id)?;
        log::info!(
            "Found {} events in {}ms",
            events.len(),
            before.elapsed().as_millis()
        );
        if events.is_empty() {
            min_height += batch_size;
            continue;
        }
        let before = Instant::now();
        update_balances(&events, balances_repository)?;
        log::info!(
            "Processed {} events in {}ms",
            events.len(),
            before.elapsed().as_millis(),
        );
        min_height += batch_size + 1;
    }
    Ok(())
}

fn is_balance_transfer(event: &Event) -> bool {
    event.name == "TRANSFER"
}

fn update_balances(events: &Vec<Event>, repository: &BalancesRepository) -> Result<(), DbError> {
    for event in events {
        if is_balance_transfer(&event) {
            let (sender, receiver, amount) = parse_transfer_event(&event);
            if let Some(sender) = sender {
                update_account_balance(
                    &sender,
                    event.chain_id,
                    &event.qual_name,
                    &event.module,
                    event.height,
                    amount.clone() * BigDecimal::from(-1),
                    repository,
                )?;
            }
            if let Some(receiver) = receiver {
                update_account_balance(
                    &receiver,
                    event.chain_id,
                    &event.qual_name,
                    &event.module,
                    event.height,
                    amount,
                    repository,
                )?;
            }
        }
    }
    Ok(())
}

fn parse_transfer_event(event: &Event) -> (Option<String>, Option<String>, BigDecimal) {
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
    (
        (!sender.is_empty()).then(|| sender),
        (!receiver.is_empty()).then(|| receiver),
        BigDecimal::from(amount),
    )
}

pub fn update_account_balance(
    account: &str,
    chain_id: i64,
    qual_name: &str,
    module: &str,
    height: i64,
    change: BigDecimal,
    balances_repository: &BalancesRepository,
) -> Result<Balance, DbError> {
    match balances_repository.find_by_account_chain_and_module(account, chain_id, module) {
        Ok(Some(balance)) => {
            let new_balance = Balance {
                amount: balance.amount + change,
                height,
                ..balance
            };
            balances_repository.update(&new_balance)
        }
        Ok(None) => {
            let balance = Balance {
                account: account.to_string(),
                chain_id,
                qual_name: qual_name.to_string(),
                module: module.to_string(),
                amount: change,
                height,
            };
            balances_repository.insert(&balance)
        }
        Err(e) => {
            log::error!("Error updating balance: {}", e);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::Block;
    use crate::repository::BlocksRepository;
    use bigdecimal::BigDecimal;
    use chrono::Utc;

    #[test]
    fn test_calculate_balances() {
        dotenvy::from_filename(".env.test").ok();
        let pool = db::initialize_db_pool();
        let blocks_repository = BlocksRepository { pool: pool.clone() };
        let events_repository = EventsRepository { pool: pool.clone() };
        let balances_repository = BalancesRepository { pool: pool.clone() };
        events_repository.delete_all().unwrap();
        blocks_repository.delete_all().unwrap();
        balances_repository.delete_all().unwrap();
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
        calculate_balances(
            chain_id,
            batch_size,
            None,
            &events_repository,
            &balances_repository,
        )
        .unwrap();
        let bob_balance = balances_repository
            .find_by_account_chain_and_module("bob", 0, "coin")
            .unwrap()
            .unwrap();
        assert!(bob_balance.amount == BigDecimal::from_str("25.6").unwrap());

        let alice_balance = balances_repository
            .find_by_account_chain_and_module("alice", 0, "coin")
            .unwrap()
            .unwrap();
        assert!(alice_balance.amount == BigDecimal::from_str("74.5").unwrap());
    }

    #[test]
    fn test_parse_transfer_event() {
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
        let (sender, receiver, amount) = parse_transfer_event(&event);
        assert_eq!(sender.unwrap(), "bob");
        assert_eq!(receiver.unwrap(), "alice");
        assert_eq!(amount, BigDecimal::from_str("100.12324354665567").unwrap());
        let no_sender_event = Event {
            params: serde_json::json!(["", "alice", 10]),
            ..event.clone()
        };
        let (sender, _, _) = parse_transfer_event(&no_sender_event);
        assert!(sender.is_none());
        let no_receiver_event = Event {
            params: serde_json::json!(["bob", "", 10]),
            ..event
        };
        let (_, receiver, _) = parse_transfer_event(&no_receiver_event);
        assert!(receiver.is_none());
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
        let (_, _, amount) = parse_transfer_event(&event);
        assert!(amount == BigDecimal::from_str("22.230409400000000000000000").unwrap());
    }

    #[test]
    fn test_parse_transfer_event_wrong_amount() {
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
        let (_, _, amount) = parse_transfer_event(&event);
        assert!(amount == BigDecimal::from(0));
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
