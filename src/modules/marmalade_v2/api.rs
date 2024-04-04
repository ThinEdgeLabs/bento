use actix_web::{error, get, web, HttpResponse, Responder};

#[get("/collection/{id}")]
async fn get_collection(
    path: web::Path<String>,
    //transfers: web::Data<TransfersRepository>,
) -> actix_web::Result<impl Responder> {
    let id = path.into_inner();
    let response = format!("get_collection, collection id: {}", id);
    // let balance: HashMap<i64, BigDecimal> =
    //     web::block(move || transfers.calculate_balance(&account, &module))
    //         .await?
    //         .map_err(error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(response))
}

pub fn get_routes() -> actix_web::Scope {
    actix_web::web::scope("/marmalade-v2").service(get_collection)
}
