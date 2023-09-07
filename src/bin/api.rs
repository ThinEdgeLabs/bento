use actix_web::{error, get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use bento::db;
use bento::models::*;
use bento::repository::*;
use bigdecimal::BigDecimal;
use dotenvy::dotenv;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::time::Instant;

#[derive(Deserialize)]
struct RequestKeys {
    request_keys: Vec<String>,
}

#[get("/tx/{request_key}")]
async fn tx(
    path: web::Path<String>,
    transactions: web::Data<TransactionsRepository>,
) -> actix_web::Result<impl Responder> {
    let request_key = path.into_inner();
    let req_key = request_key.clone();
    let tx: HashMap<String, Vec<Transaction>> =
        web::block(move || transactions.find_all_related(&vec![request_key]))
            .await?
            .map_err(error::ErrorInternalServerError)?;
    Ok(match tx.contains_key(&req_key) {
        false => HttpResponse::NotFound().body("Tx not found"),
        true => HttpResponse::Ok().json(tx.get(&req_key).unwrap()),
    })
}

#[post("/txs")]
async fn txs(
    body: web::Json<RequestKeys>,
    transactions: web::Data<TransactionsRepository>,
) -> actix_web::Result<impl Responder> {
    let result: HashMap<String, Vec<Transaction>> =
        web::block(move || transactions.find_all_related(&body.request_keys))
            .await?
            .map_err(error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(result))
}

#[get("/balance/{account}")]
async fn all_balances(
    path: web::Path<String>,
    transfers: web::Data<TransfersRepository>,
) -> actix_web::Result<impl Responder> {
    let account = path.into_inner();
    let all: HashMap<String, HashMap<i64, BigDecimal>> =
        web::block(move || transfers.calculate_all_balances(&account))
            .await?
            .map_err(error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(all))
}

#[get("/balance/{account}/{module}")]
async fn balance(
    path: web::Path<(String, String)>,
    transfers: web::Data<TransfersRepository>,
) -> actix_web::Result<impl Responder> {
    let (account, module) = path.into_inner();
    let balance: HashMap<i64, BigDecimal> =
        web::block(move || transfers.calculate_balance(&account, &module))
            .await?
            .map_err(error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(balance))
}

#[deprecated(note = "Use /transfers instead, this endpoint will be removed in the near future")]
#[get("/transfers/{account}/received")]
async fn received_transfers(
    path: web::Path<String>,
    request: HttpRequest,
    transfers: web::Data<TransfersRepository>,
) -> actix_web::Result<impl Responder> {
    let account = path.into_inner();
    let params = web::Query::<HashMap<String, i64>>::from_query(request.query_string()).unwrap();
    let min_height = params.get("min_height").copied();
    let before = Instant::now();
    let transfers: HashMap<String, Vec<Transfer>> =
        web::block(move || transfers.find_received(&account, min_height))
            .await?
            .map_err(error::ErrorInternalServerError)?;
    log::info!("Received transfers took {:?}", before.elapsed().as_millis());
    Ok(HttpResponse::Ok().json(transfers))
}

#[get("/transfers")]
async fn get_transfers(
    request: HttpRequest,
    transfers: web::Data<TransfersRepository>,
) -> actix_web::Result<impl Responder> {
    let params = web::Query::<HashMap<String, String>>::from_query(request.query_string()).unwrap();
    let from = params.get("from").map(|e| e.to_string());
    let to = params.get("to").map(|e| e.to_string());
    let min_height = match params.get("min_height").map(|h| h.parse::<i64>()) {
        Some(Ok(height)) => Some(height),
        Some(Err(_)) => return Ok(HttpResponse::BadRequest().body("Invalid min_height")),
        None => None,
    };
    let transfers = web::block(move || transfers.find(from, to, min_height))
        .await?
        .map_err(error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(transfers))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    dotenv().ok();
    let port = env::var("API_PORT")
        .expect("Missing API_PORT")
        .parse::<u16>()
        .expect("Invalid API_PORT");

    let pool = db::initialize_db_pool();
    let transactions = TransactionsRepository { pool: pool.clone() };
    let transfers = TransfersRepository { pool: pool.clone() };

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(transactions.clone()))
            .app_data(web::Data::new(transfers.clone()))
            .service(tx)
            .service(txs)
            .service(balance)
            .service(all_balances)
            .service(received_transfers)
            .service(get_transfers)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
