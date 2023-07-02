use futures::{Stream, TryStreamExt};
use reqwest::Url;
use serde::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Display;
use std::{collections::HashMap, error::Error};

use self::tx_result::PactTransactionResult;

const HOST: &str = "http://147.182.182.28/chainweb/0.0/mainnet01";

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct BlockHeaderBranchResponse {
    items: Vec<String>,
    next: Option<String>,
    limit: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Bounds {
    pub lower: Vec<Hash>,
    pub upper: Vec<Hash>,
}

#[derive(Deserialize, Debug)]
pub struct BlockHash {
    pub height: u32,
    pub hash: String,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub struct ChainId(pub u16);

impl Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Hash(pub String);

#[derive(Deserialize, Debug)]
pub struct Cut {
    pub height: u32,
    pub weight: String,
    pub hashes: HashMap<ChainId, BlockHash>,
    pub instance: String,
    pub id: String,
}

#[derive(Deserialize, Debug)]
pub struct BlockHeader {
    #[serde(rename(deserialize = "creationTime"))]
    pub creation_time: i64,
    pub parent: String,
    pub height: u64,
    pub hash: String,
    #[serde(rename(deserialize = "chainId"))]
    pub chain_id: ChainId,
    #[serde(rename(deserialize = "payloadHash"))]
    pub payload_hash: String,
    pub weight: String,
    #[serde(rename(deserialize = "featureFlags"))]
    pub feature_flags: i32,
    #[serde(rename(deserialize = "epochStart"))]
    pub epoch_start: i64,
    pub adjacents: HashMap<ChainId, String>,
    #[serde(rename(deserialize = "chainwebVersion"))]
    pub chainweb_version: String,
    pub target: String,
    pub nonce: String,
}

#[derive(Deserialize, Debug)]
pub struct BlockHeaderResponse {
    pub items: Vec<BlockHeader>,
    pub limit: u32,
    pub next: Option<String>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct BlockPayload {
    #[serde(rename(deserialize = "minerData"))]
    pub miner_data: String,
    #[serde(rename(deserialize = "outputsHash"))]
    pub outputs_hash: String,
    #[serde(rename(deserialize = "payloadHash"))]
    pub payload_hash: String,
    pub transactions: Vec<String>,
    #[serde(rename(deserialize = "transactionsHash"))]
    pub transactions_hash: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Sig {
    pub sig: String,
}
#[derive(Deserialize, Debug, PartialEq)]
pub struct SignedTransaction {
    pub cmd: String,
    pub hash: String,
    pub sigs: Vec<Sig>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub enum Network {
    #[serde(rename = "mainnet01")]
    Mainnet,
    #[serde(rename = "testnet04")]
    Testnet,
    #[serde(rename = "development")]
    Devnet,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Meta {
    #[serde(rename(deserialize = "chainId"))]
    pub chain_id: String,
    #[serde(
        rename(deserialize = "creationTime",),
        deserialize_with = "de_f64_or_u64_as_f64"
    )]
    pub creation_time: f64,
    #[serde(
        rename(deserialize = "gasLimit"),
        deserialize_with = "de_i64_or_string_as_i64"
    )]
    pub gas_limit: i64,
    #[serde(
        rename(deserialize = "gasPrice"),
        deserialize_with = "de_f64_or_string_as_f64"
    )]
    pub gas_price: f64,
    pub sender: String,
    #[serde(deserialize_with = "de_f64_or_u64_or_string_as_u64")]
    pub ttl: u64,
}

fn de_f64_or_u64_as_f64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(num) => num.as_f64().unwrap(),
        _ => return Err(serde::de::Error::custom("expected a number")),
    })
}

fn de_f64_or_string_as_f64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(num) => num.as_f64().unwrap(),
        Value::String(s) => s.parse().unwrap(),
        _ => return Err(serde::de::Error::custom("expected a number or a string")),
    })
}

fn de_i64_or_string_as_i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(num) => num.as_i64().unwrap(),
        Value::String(s) => s.parse().unwrap(),
        _ => return Err(serde::de::Error::custom("expected a number or a string")),
    })
}

fn de_f64_or_u64_or_string_as_u64<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<u64, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(num) => num.as_f64().unwrap() as u64,
        Value::String(s) => s.parse().unwrap(),
        _ => return Err(serde::de::Error::custom("expected a number or a string")),
    })
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Signer {
    #[serde(rename(deserialize = "pubKey"))]
    public_key: String,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Command {
    #[serde(rename(deserialize = "networkId"))]
    pub network_id: Option<Network>,
    pub nonce: String,
    pub payload: Payload,
    pub signers: Vec<Signer>,
    pub meta: Meta,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Payload {
    pub exec: Option<ExecPayload>,
    pub cont: Option<ContPayload>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ExecPayload {
    pub code: String,
    pub data: Value,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ContPayload {
    pub data: Value,
    #[serde(rename(deserialize = "pactId"))]
    pub pact_id: String,
    pub proof: Option<String>,
    pub rollback: bool,
    pub step: u32,
}

pub mod tx_result {
    use super::*;

    #[derive(Deserialize, Debug)]
    pub struct Module {
        pub name: String,
        pub namespace: Option<String>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Event {
        pub module: Module,
        #[serde(rename(deserialize = "moduleHash"))]
        pub module_hash: String,
        pub name: String,
        pub params: Value,
    }

    #[derive(Deserialize, Serialize, Debug)]
    pub struct Metadata {
        #[serde(rename(deserialize = "blockHash"))]
        pub block_hash: String,
        #[serde(rename(deserialize = "blockHeight"))]
        pub block_height: i64,
        #[serde(rename(deserialize = "blockTime"))]
        pub block_time: i64,
        #[serde(rename(deserialize = "prevBlockHash"))]
        pub prev_block_hash: String,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "lowercase")]
    pub enum Status {
        Success,
        Failure,
    }

    #[derive(Deserialize, Debug)]
    pub struct Result {
        pub error: Option<Value>,
        pub data: Option<Value>,
        pub status: Status,
    }

    #[derive(Deserialize, Debug)]
    pub struct PactTransactionResult {
        pub continuation: Option<Value>,
        pub events: Option<Vec<Event>>,
        pub gas: i64,
        pub logs: String,
        #[serde(rename(deserialize = "metaData"))]
        pub metadata: Metadata,
        #[serde(rename(deserialize = "reqKey"))]
        pub request_key: String,
        pub result: Result,
        #[serde(rename(deserialize = "txId"))]
        pub tx_id: Option<i64>,
    }
}

pub async fn get_cut() -> Result<Cut, Box<dyn Error>> {
    let endpoint = "/cut";
    let url = Url::parse(&format!("{HOST}{endpoint}")).unwrap();
    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await?
        .json::<Cut>()
        .await?;
    Ok(response)
}

#[allow(dead_code)]
async fn get_block_hashes_branches(
    chain: &ChainId,
    bounds: &Bounds,
) -> Result<BlockHeaderBranchResponse, Box<dyn Error>> {
    let endpoint = format!("/chain/{chain}/hash/branch");
    let mut url = Url::parse(&format!("{HOST}{endpoint}")).unwrap();
    url.query_pairs_mut().append_pair("limit", "50");
    let response = reqwest::Client::new()
        .post(url)
        .json(bounds)
        .send()
        .await?
        .json::<BlockHeaderBranchResponse>()
        .await?;
    Ok(response)
}

pub async fn get_block_headers_branches(
    chain: &ChainId,
    bounds: &Bounds,
    next: &Option<String>,
) -> Result<BlockHeaderResponse, Box<dyn Error>> {
    let endpoint = format!("/chain/{chain}/header/branch");
    let mut url = Url::parse(&format!("{HOST}{endpoint}")).unwrap();
    url.query_pairs_mut().append_pair("limit", "50");
    if let Some(next) = next {
        url.query_pairs_mut().append_pair("next", &next);
    }
    let mut headers = reqwest::header::HeaderMap::new();
    headers.append(
        "accept",
        "application/json;blockheader-encoding=object"
            .parse()
            .unwrap(),
    );

    let response: BlockHeaderResponse = reqwest::Client::new()
        .post(url)
        .json(bounds)
        .headers(headers)
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

pub async fn get_block_payload_batch(
    chain: &ChainId,
    block_payload_hash: Vec<&str>,
) -> Result<Vec<BlockPayload>, Box<dyn Error>> {
    let endpoint = format!("/chain/{chain}/payload/batch");
    let url = Url::parse(&format!("{HOST}{endpoint}")).unwrap();
    let response: Vec<BlockPayload> = reqwest::Client::new()
        .post(url)
        .json(&block_payload_hash)
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

pub async fn poll(
    request_keys: &Vec<String>,
    chain: &ChainId,
) -> Result<HashMap<String, PactTransactionResult>, Box<dyn Error>> {
    let endpoint = format!("/chain/{chain}/pact/api/v1/poll");
    let url = Url::parse(&format!("{HOST}{endpoint}")).unwrap();
    let response = reqwest::Client::new()
        .post(url)
        .json(&serde_json::json!({ "requestKeys": request_keys }))
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

#[allow(dead_code)]
pub fn headers_stream() -> Result<impl Stream<Item = Result<(), ()>>, eventsource_client::Error> {
    use eventsource_client as es;
    use eventsource_client::Client;
    use std::time::Duration;

    let endpoint = format!("/header/updates");
    let url = Url::parse(&format!("{HOST}{endpoint}")).unwrap();
    log::info!("connecting to {}", url.as_str());
    let client = es::ClientBuilder::for_url(url.as_str())?
        .reconnect(
            es::ReconnectOptions::reconnect(true)
                .retry_initial(false)
                .delay(Duration::from_secs(1))
                .backoff_factor(2)
                .delay_max(Duration::from_secs(60))
                .build(),
        )
        .build();
    let result = client
        .stream()
        .map_ok(|event| match event {
            es::SSE::Event(ev) => {
                println!("got an event: {}\n{}", ev.event_type, ev.data)
            }
            es::SSE::Comment(comment) => {
                println!("got a comment: \n{}", comment)
            }
        })
        .map_err(|err| eprintln!("error streaming events: {:?}", err));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing_cont_command_json() {
        let json = "{\"networkId\":\"mainnet01\",\"payload\":{\"cont\":{\"proof\":\"proof\",\"pactId\":\"AoKZVe35EWK-2a-kj_tD6vC8Ifdt1mdQyK0_2Rm_Jto\",\"rollback\":false,\"step\":1,\"data\":{}}},\"signers\":[],\"meta\":{\"creationTime\":1687931936,\"ttl\":3600,\"gasLimit\":850,\"chainId\":\"0\",\"gasPrice\":1e-8,\"sender\":\"xwallet-xchain-gas\"},\"nonce\":\"\\\"2023-06-28T05:59:55.767Z\\\"\"}";

        let cont = ContPayload {
            data: serde_json::json!({}),
            pact_id: "AoKZVe35EWK-2a-kj_tD6vC8Ifdt1mdQyK0_2Rm_Jto".to_string(),
            proof: Some("proof".to_string()),
            rollback: false,
            step: 1,
        };
        let cmd = Command {
            network_id: Some(Network::Mainnet),
            payload: Payload {
                exec: None,
                cont: Some(cont),
            },
            signers: vec![],
            nonce: String::from("\"2023-06-28T05:59:55.767Z\""),
            meta: Meta {
                chain_id: String::from("0"),
                creation_time: 1687931936.0,
                gas_limit: 850,
                gas_price: 0.00000001,
                sender: String::from("xwallet-xchain-gas"),
                ttl: 3600,
            },
        };
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), cmd);
    }

    #[test]
    fn test_parsing_exec_command_json() {
        let json = "{\"networkId\":\"mainnet01\",\"payload\":{\"exec\":{\"data\":{\"keyset\":{\"pred\":\"keys-all\",\"keys\":[\"48484c674e734ba4deef7289b47c14d0743e914e2fc0863b9859ac0ec2715173\"]}},\"code\":\"(free.radio02.direct-to-send \\\"k:1625709839e6c607385cc6b71191ae033217da29fe4bcaf8131575ba31f6d58e\\\" )\"}},\"signers\":[{\"pubKey\":\"48484c674e734ba4deef7289b47c14d0743e914e2fc0863b9859ac0ec2715173\"}],\"meta\":{\"creationTime\":1687938640,\"ttl\":28800.0,\"gasLimit\":1000,\"chainId\":\"0\",\"gasPrice\":0.000001,\"sender\":\"k:48484c674e734ba4deef7289b47c14d0743e914e2fc0863b9859ac0ec2715173\"},\"nonce\":\"\\\"2023-06-28T07:50:55.438Z\\\"\"}";
        let exec = ExecPayload {
            code: String::from("(free.radio02.direct-to-send \"k:1625709839e6c607385cc6b71191ae033217da29fe4bcaf8131575ba31f6d58e\" )"),
            data: serde_json::json!({
                "keyset": {
                    "pred": "keys-all",
                    "keys": [
                        "48484c674e734ba4deef7289b47c14d0743e914e2fc0863b9859ac0ec2715173"
                    ]
                }
            }),
        };
        let cmd = Command {
            network_id: Some(Network::Mainnet),
            nonce: String::from("\"2023-06-28T07:50:55.438Z\""),
            payload: Payload {
                exec: Some(exec),
                cont: None,
            },
            signers: vec![Signer {
                public_key: String::from(
                    "48484c674e734ba4deef7289b47c14d0743e914e2fc0863b9859ac0ec2715173",
                ),
            }],
            meta: Meta {
                chain_id: String::from("0"),
                creation_time: 1687938640.0,
                gas_limit: 1000,
                gas_price: 0.000001,
                sender: String::from(
                    "k:48484c674e734ba4deef7289b47c14d0743e914e2fc0863b9859ac0ec2715173",
                ),
                ttl: 28800,
            },
        };
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), cmd);
    }

    #[test]
    fn test_parsing_command() {
        let json = "{\"networkId\":\"mainnet01\",\"payload\":{\"exec\":null,\"cont\":{\"proof\":\"proof\",\"pactId\":\"oPTGT8K99LDbqdwrPR-HYhM2aDGVpcokB-wZ_UQKiF0\",\"rollback\":false,\"step\":1,\"data\":{}}},\"signers\":[],\"meta\":{\"creationTime\":1688098548,\"ttl\":28800,\"gasLimit\":850,\"chainId\":\"1\",\"gasPrice\":1.0E-8,\"sender\":\"kadena-xchain-gas\"},\"nonce\":\"2023-06-30T04:15:48.169508637Z[UTC]\"}";
        let command = serde_json::from_str::<Command>(json).unwrap();
        assert!(command.payload.exec.is_none());
        assert!(command.payload.cont.is_some());

        let json = "{\"networkId\":\"mainnet01\",\"payload\":{\"exec\":{\"code\":\"(namespace \\\"user\\\") (define-keyset \\\"user.z-ks\\\" (read-keyset \\\"ks\\\"))\",\"data\":{\"ks\":{\"keys\":[\"9eb1f99fdc35413c05d58f182d761c38d2b8620b04a5438053ab737099a7f305\"],\"pred\":\"keys-all\"}}}},\"meta\":{\"sender\":\"k:9eb1f99fdc35413c05d58f182d761c38d2b8620b04a5438053ab737099a7f305\",\"chainId\":\"1\",\"gasPrice\":\"0.00000001\",\"gasLimit\":\"100000\",\"ttl\":\"600\",\"creationTime\":1683614361},\"signers\":[{\"pubKey\":\"9eb1f99fdc35413c05d58f182d761c38d2b8620b04a5438053ab737099a7f305\",\"clist\":[]},{\"pubKey\":\"9eb1f99fdc35413c05d58f182d761c38d2b8620b04a5438053ab737099a7f305\",\"clist\":[{\"name\":\"coin.GAS\",\"args\":[]}]}],\"nonce\":\"1683614361\"}";
        let command = serde_json::from_str::<Command>(json).unwrap();
        assert!(command.payload.exec.is_some());
        assert!(command.payload.cont.is_none());
    }

    #[test]
    fn test_parsing_command_with_gas_price_as_string() {
        let json = "{\"meta\":{\"chainId\":\"0\",\"creationTime\":1688039944,\"gasLimit\":8000,\"gasPrice\":\"0.00000001\",\"sender\":\"k:0b259904ba912dcfe7af4c70016e1a93982610c740b27c766ad329772ad44bd3\",\"ttl\":28860},\"networkId\":\"mainnet01\",\"nonce\":\"2023-06-29T19:59:04Z.189Z\",\"payload\":{\"exec\":{\"code\":\"(coin.transfer-create \\\"k:0b259904ba912dcfe7af4c70016e1a93982610c740b27c766ad329772ad44bd3\\\" \\\"k:5c01f1f5d0aa2fe56ad69b50025d56a1e2043cd76f743e792da7adf04d7abd06\\\" (read-keyset \\\"receiver-guard\\\") 230.9)\",\"data\":{\"receiver-guard\":{\"keys\":[\"5c01f1f5d0aa2fe56ad69b50025d56a1e2043cd76f743e792da7adf04d7abd06\"],\"pred\":\"keys-all\"}}}},\"signers\":[{\"clist\":[{\"args\":[\"k:0b259904ba912dcfe7af4c70016e1a93982610c740b27c766ad329772ad44bd3\",\"k:5c01f1f5d0aa2fe56ad69b50025d56a1e2043cd76f743e792da7adf04d7abd06\",230.9],\"name\":\"coin.TRANSFER\"},{\"args\":[],\"name\":\"coin.GAS\"}],\"pubKey\":\"0b259904ba912dcfe7af4c70016e1a93982610c740b27c766ad329772ad44bd3\"}]}";
        let command = serde_json::from_str::<Command>(json).unwrap();
        assert!(command.meta.gas_price == 0.00000001);
    }
}

//"{\"networkId\":\"mainnet01\",\"payload\":{\"exec\":{\"data\":{\"user-ks\":{\"pred\":\"keys-all\",\"keys\":[\"4923fc6713ec16d3d21b08d44e236a3663a0442797ed46c5c7f759a8519bd1d1\"]},\"account\":\"k:4923fc6713ec16d3d21b08d44e236a3663a0442797ed46c5c7f759a8519bd1d1\"},\"code\":\"(coin.transfer-crosschain \\\"k:4923fc6713ec16d3d21b08d44e236a3663a0442797ed46c5c7f759a8519bd1d1\\\" \\\"k:4923fc6713ec16d3d21b08d44e236a3663a0442797ed46c5c7f759a8519bd1d1\\\" (read-keyset \\\"user-ks\\\") \\\"8\\\" 0.000355000000)\"}},\"signers\":[{\"clist\":[{\"name\":\"coin.TRANSFER_XCHAIN\",\"args\":[\"k:4923fc6713ec16d3d21b08d44e236a3663a0442797ed46c5c7f759a8519bd1d1\",\"k:4923fc6713ec16d3d21b08d44e236a3663a0442797ed46c5c7f759a8519bd1d1\",0.000355,\"8\"]}],\"pubKey\":\"4923fc6713ec16d3d21b08d44e236a3663a0442797ed46c5c7f759a8519bd1d1\"}],\"meta\":{\"creationTime\":1688045415.29,\"ttl\":1200,\"gasLimit\":1100,\"chainId\":\"3\",\"gasPrice\":2e-8,\"sender\":\"746d0601603d1cc907ae82fed1c4bdf3\"},\"nonce\":\"\\\"1688045415.292\\\"\"}"
