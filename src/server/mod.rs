// src/server/mod.rs

use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Error};
use actix_web_actors::ws;
use crate::event::{EVENT_BROKER};

/// The main entry point for the web server.
/// This function will be called from `main.rs` to start the server.
pub async fn start_server() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/ws", web::get().to(ws_index))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

/// The WebSocket actor.
/// This struct will be responsible for handling the WebSocket connection.
struct MyWs;

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let mut rx = EVENT_BROKER.subscribe();
        let addr = ctx.address();

        actix_rt::spawn(async move {
            while let Ok(event) = rx.recv().await {
                addr.do_send(event);
            }
        });
    }
}

impl actix::Handler<crate::event::Event> for MyWs {
    type Result = ();

    fn handle(&mut self, msg: crate::event::Event, ctx: &mut Self::Context) {
        ctx.text(format!("{:?}", msg));
    }
}

/// This is the handler for the WebSocket stream.
/// It will be called whenever a message is received from the client.
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(_)) => (),
            Ok(ws::Message::Binary(_)) => (),
            _ => (),
        }
    }
}

/// This is the handler for the WebSocket connection.
/// It will be called whenever a new WebSocket connection is established.
async fn ws_index(r: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    ws::start(MyWs, &r, stream)
}