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

#[derive(Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Sig {
    pub sig: String,
}
#[derive(Deserialize, Debug)]
pub struct SignedTransaction {
    pub cmd: String,
    pub hash: String,
    pub sigs: Vec<Sig>,
}

#[derive(Deserialize, Debug)]
pub enum Network {
    #[serde(rename = "mainnet01")]
    Mainnet,
    #[serde(rename = "testnet04")]
    Testnet,
    #[serde(rename = "development")]
    Devnet,
}

#[derive(Deserialize, Debug)]
pub struct Meta {
    #[serde(rename(deserialize = "chainId"))]
    pub chain_id: String,
    #[serde(rename(deserialize = "creationTime"))]
    pub creation_time: u64,
    #[serde(rename(deserialize = "gasLimit"))]
    pub gas_limit: f32,
    #[serde(rename(deserialize = "gasPrice"))]
    pub gas_price: f32,
    pub sender: String,
    pub ttl: u32,
}

#[derive(Deserialize, Debug)]
pub struct Signer {
    #[serde(rename(deserialize = "pubKey"))]
    public_key: String,
}

#[derive(Deserialize, Debug)]
pub struct Command {
    #[serde(rename(deserialize = "networkId"))]
    pub network_id: Network,
    pub nonce: String,
    pub payload: Value,
    pub signers: Vec<Signer>,
    pub meta: Meta,
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
