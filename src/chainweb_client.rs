use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Display;
use std::{collections::HashMap, error::Error};

use self::tx_result::PactTransactionResult;

const HOST: &str = "http://147.182.182.28/chainweb/0.0/mainnet01";

#[derive(Deserialize, Debug)]
struct BlockHeaderBranchResponse {
    items: Vec<String>,
    next: Option<String>,
    limit: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Bounds {
    pub lower: Vec<Hash>,
    pub upper: Vec<Hash>,
}

#[derive(Deserialize, Debug)]
pub struct BlockHash {
    pub height: u32,
    pub hash: String,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct ChainId(pub u16);

impl Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Debug)]
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
    #[serde(rename(deserialize = "creationTime"))]
    pub creation_time: u64,
    #[serde(rename(deserialize = "gasLimit"))]
    pub gas_limit: i32,
    #[serde(rename(deserialize = "gasPrice"))]
    pub gas_price: f32,
    pub sender: String,
    pub ttl: u32,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Signer {
    #[serde(rename(deserialize = "pubKey"))]
    public_key: String,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Command {
    #[serde(rename(deserialize = "networkId"))]
    pub network_id: Network,
    pub nonce: String,
    pub payload: Payload,
    pub signers: Vec<Signer>,
    pub meta: Meta,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Payload {
    #[serde(rename(deserialize = "cont", serialize = "cont"))]
    Cont(ContPayload),
    #[serde(rename(deserialize = "exec", serialize = "exec"))]
    Exec(ExecPayload),
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
    pub proof: String,
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
        pub events: Vec<Event>,
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
    url.query_pairs_mut().append_pair("limit", "10");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing_cont_command_json() {
        let json = "{\"networkId\":\"mainnet01\",\"payload\":{\"cont\":{\"proof\":\"proof\",\"pactId\":\"AoKZVe35EWK-2a-kj_tD6vC8Ifdt1mdQyK0_2Rm_Jto\",\"rollback\":false,\"step\":1,\"data\":{}}},\"signers\":[],\"meta\":{\"creationTime\":1687931936,\"ttl\":3600,\"gasLimit\":850,\"chainId\":\"0\",\"gasPrice\":1e-8,\"sender\":\"xwallet-xchain-gas\"},\"nonce\":\"\\\"2023-06-28T05:59:55.767Z\\\"\"}";

        let cont = ContPayload {
            data: serde_json::json!({}),
            pact_id: "AoKZVe35EWK-2a-kj_tD6vC8Ifdt1mdQyK0_2Rm_Jto".to_string(),
            proof: "proof".to_string(),
            rollback: false,
            step: 1,
        };
        let cmd = Command {
            network_id: Network::Mainnet,
            payload: Payload::Cont(cont),
            signers: vec![],
            nonce: String::from("\"2023-06-28T05:59:55.767Z\""),
            meta: Meta {
                chain_id: String::from("0"),
                creation_time: 1687931936,
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
        let json = "{\"networkId\":\"mainnet01\",\"payload\":{\"exec\":{\"data\":{\"keyset\":{\"pred\":\"keys-all\",\"keys\":[\"48484c674e734ba4deef7289b47c14d0743e914e2fc0863b9859ac0ec2715173\"]}},\"code\":\"(free.radio02.direct-to-send \\\"k:1625709839e6c607385cc6b71191ae033217da29fe4bcaf8131575ba31f6d58e\\\" )\"}},\"signers\":[{\"pubKey\":\"48484c674e734ba4deef7289b47c14d0743e914e2fc0863b9859ac0ec2715173\"}],\"meta\":{\"creationTime\":1687938640,\"ttl\":28800,\"gasLimit\":1000,\"chainId\":\"0\",\"gasPrice\":0.000001,\"sender\":\"k:48484c674e734ba4deef7289b47c14d0743e914e2fc0863b9859ac0ec2715173\"},\"nonce\":\"\\\"2023-06-28T07:50:55.438Z\\\"\"}";
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
            network_id: Network::Mainnet,
            nonce: String::from("\"2023-06-28T07:50:55.438Z\""),
            payload: Payload::Exec(exec),
            signers: vec![Signer {
                public_key: String::from(
                    "48484c674e734ba4deef7289b47c14d0743e914e2fc0863b9859ac0ec2715173",
                ),
            }],
            meta: Meta {
                chain_id: String::from("0"),
                creation_time: 1687938640,
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
}
