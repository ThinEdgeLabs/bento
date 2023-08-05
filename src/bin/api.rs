use std::collections::HashMap;

use actix_web::cookie::time::Instant;
use bento::db;
use bento::models::*;
use bento::repository::*;
use bigdecimal::BigDecimal;
use dotenvy::dotenv;
use serde::Deserialize;

use actix_web::{error, get, post, web, App, HttpResponse, HttpServer, Responder};

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

#[derive(Deserialize)]
struct RequestKeys {
    request_keys: Vec<String>,
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

#[get("/transfers/{account}/received")]
async fn received_transfers(
    path: web::Path<String>,
    transfers: web::Data<TransfersRepository>,
) -> actix_web::Result<impl Responder> {
    let account = path.into_inner();
    let before = Instant::now();
    let transfers: Vec<Transfer> = web::block(move || transfers.find_received(&account))
        .await?
        .map_err(error::ErrorInternalServerError)?;
    log::info!("Received transfers took {:?}", before.elapsed());
    Ok(HttpResponse::Ok().json(transfers))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    dotenv().ok();

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
    })
    .bind(("0.0.0.0", 8181))?
    .run()
    .await
}
