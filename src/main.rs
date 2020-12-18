use std::fs::File;
use std::io::Read;
use std::time::{Duration, Instant};
use std::sync::{Arc,Mutex};

use actix::{Actor, StreamHandler, Addr};
use actix_web::{get, web, App, HttpRequest, HttpResponse, HttpServer, Responder, Error, middleware, dev, http};
use actix_web::middleware::errhandlers::{ErrorHandlerResponse, ErrorHandlers};
use actix_files as fs;
use actix_web_actors::ws;
use futures::future::{ready, Ready};
use serde::{Serialize,Deserialize};
use log::{info, debug};
use env_logger::Env;

mod socket;

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);


#[get("/hello")]
async fn hello() -> impl Responder {
  HttpResponse::Ok().body("Hello world")
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

  let f = File::open("./static/404.html");
  match f {
      Ok(mut file) => {
          let mut content = String::new();
          file.read_to_string(&mut content);

          let new_res = res.map_body(| _head, _body | {
            dev::ResponseBody::Other(dev::Body::Message(Box::new(content)))
          });

          Ok(ErrorHandlerResponse::Response(new_res))
      }
      Err(err) => {
          Err(Error::from(err))
      }
  }
}

struct AppState {
    pub counter: Mutex<u32>,
    pub clients: Mutex<Vec<Addr<WebSocketHandler>>>,
}

#[derive(Serialize, Deserialize)]
struct WebSocketRequest {
    #[serde(default)]
    t: String,
}

#[derive(Clone)]
struct WebSocketHandler {
    state: web::Data<AppState>,
}

impl Actor for WebSocketHandler {
    type Context = ws::WebsocketContext<Self>;
}

/// Handler for ws::Message message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketHandler {
    fn started(&mut self, ctx: &mut Self::Context) {
        let mut clients = self.state.clients.lock().unwrap();

        clients.push(ctx.address());
    }

    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                let req: WebSocketRequest = serde_json::from_str(text.as_str()).unwrap();
                match req.t.as_str() {
                    "pressed" => {
                        let mut counter = self.state.counter.lock().unwrap();
                        *counter += 1;
                        let r = String::from(format!("{{\"t\":\"count\",\"c\":{}}}",counter));

                        for client in ctx.state().clients.lock().unwrap().iter() {
                            client.text(r);
                        };
                    }
                    _ => {}
                }
            },
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            _ => (),
        }
    }
}

async fn websocket(
    req: HttpRequest,
    stream: web::Payload,
    data: web::Data<AppState>
) -> Result<HttpResponse, Error> {
    let handler = WebSocketHandler { state: data };
    let resp = ws::start(handler, &req, stream);
    println!("{:?}", resp);
    resp
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  std::env::set_var("RUST_LOG", "debug,actix_web=info");
  std::env::set_var("RUST_BACKTRACE", "1");
  env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

  let data = web::Data::new(AppState {
    counter: Mutex::new(0),
    clients:  Mutex::new(Vec::<Addr<WebSocketHandler>>::new()),
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
        .header("Content-Security-Policy", "default-src 'self';style-src 'unsafe-inline' 'self';")
      )
      .wrap(middleware::Compress::default())
      .service(web::scope("/api")
        .service(hello)
        .route("/ip", web::get().to(index))
        .route("/socket", web::get().to(websocket))
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
