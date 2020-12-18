use actix_web::{get, web, App, HttpRequest, HttpResponse, HttpServer, Responder, Error, middleware, dev, http};
use actix_web::middleware::errhandlers::{ErrorHandlerResponse, ErrorHandlers};
use actix_files as fs;
use std::sync::Mutex;
use futures::future::{ready, Ready};
use serde::Serialize;
use log::{info, debug};
use env_logger::Env;

#[get("/hello")]
async fn hello() -> impl Responder {
  HttpResponse::Ok().body("Hello world")
}

struct AppState {
  counter: Mutex<u32>,
}

async fn counter(data: web::Data<AppState>) -> String {
  let mut counter = data.counter.lock().unwrap();
  *counter += 1;

  format!("Request {}", counter)
}

fn counter_config(cfg: &mut web::ServiceConfig) {
 cfg.service(
      web::resource("/counter")
        .route(web::get().to(counter))
        .route(web::head().to(|| HttpResponse::MethodNotAllowed()))
        .route(web::post().to(|| HttpResponse::MethodNotAllowed()))
        .route(web::put().to(|| HttpResponse::MethodNotAllowed()))
    );
}

#[derive(Serialize)]
struct IndexRes {
  ip: String,
}

impl Responder for IndexRes {
  type Error = Error;
  type Future = Ready<Result<HttpResponse, Error>>;

  fn respond_to(self, _req: &HttpRequest) -> Self::Future {
    let body = serde_json::to_string(&self).unwrap();
    
    ready(Ok(HttpResponse::Ok()
      .content_type("application/json")
      .body(body)
    ))
  }
}

async fn index(req: HttpRequest) -> impl Responder {
  let info = req.connection_info();
  let remote = info.remote_addr().unwrap();
  IndexRes { ip: String::from(remote) }
}

fn render_404<B>(mut res: dev::ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>,Error> {
  res.headers_mut().insert(
    http::header::CONTENT_TYPE,
    http::HeaderValue::from_static("text/html"),
  );

  let new_res = res.map_body(| _head, _body | {
    dev::ResponseBody::Other(dev::Body::Message(Box::new("<h1>404</h1>")))
  });

  Ok(ErrorHandlerResponse::Response(new_res))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  std::env::set_var("RUST_LOG", "debug,actix_web=info");
  std::env::set_var("RUST_BACKTRACE", "1");
  env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
  
  let data = web::Data::new(AppState {
    counter: Mutex::new(0),
  });

  HttpServer::new(move || {
    App::new()
      .app_data(data.clone())
      .wrap(middleware::Logger::default())
      .wrap(
        ErrorHandlers::new()
          .handler(http::StatusCode::NOT_FOUND, render_404)
      )
      .wrap(middleware::DefaultHeaders::new()
        .header("Server", "Rusty")
        .header("Access-Control-Allow-Origin", "localhost")
        .header("Content-Security-Policy", "default-src;")
      )
      .wrap(middleware::Compress::default())
      .service(web::scope("/api")
        .service(hello)
        .configure(counter_config)
        .route("/ip", web::get().to(index))
      )
      .service(fs::Files::new("/", "./static")
        .use_last_modified(true)
        .use_etag(true)
        .index_file("index.html")
      )
  })
  .bind("127.0.0.1:8000")?
  .run()
  .await
}
