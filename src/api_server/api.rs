use actix_web::{App, HttpResponse, HttpServer, web};
use serde::Deserialize;

pub struct ApiServer {
    pub port: u16,
}

impl ApiServer {
    pub fn new(port: u16) -> Self {
        ApiServer { port }
    }

    async fn get_idx() -> HttpResponse {
        HttpResponse::Ok().body("Hello world")
    }

    pub async fn run_server(&self) {
        let server = HttpServer::new(|| App::new().route("/", web::get().to(Self::get_idx)));

        println!("server run on port: {}", self.port);
        server
            .bind(("0.0.0.0", self.port))
            .unwrap()
            .run()
            .await
            .expect("error running the server");
    }
}
