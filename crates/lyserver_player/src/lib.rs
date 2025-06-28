use actix_web::{web, HttpResponse, Scope};

async fn get_player() -> HttpResponse {
    HttpResponse::Ok().body("Get player")
}

async fn create_player() -> HttpResponse {
    HttpResponse::Ok().body("Create player")
}

async fn update_player() -> HttpResponse {
    HttpResponse::Ok().body("Update player")
}

async fn delete_player() -> HttpResponse {
    HttpResponse::Ok().body("Delete player")
}

pub fn router() -> Scope {
    web::scope("/player")
        .service(
            web::resource("")
                .route(web::get().to(get_player))
                .route(web::put().to(create_player))
                .route(web::post().to(update_player))
                .route(web::delete().to(delete_player))
        )
}
