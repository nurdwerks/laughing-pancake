// src/server/mod.rs

use crate::event::{Event, EVENT_BROKER};
use actix::{Actor, AsyncContext, Handler, StreamHandler};
use actix_files as fs;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;

/// The main entry point for the web server.
pub async fn start_server() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/ws", web::get().to(ws_index))
            .service(fs::Files::new("/", "./static").index_file("index.html"))
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
}

/// The WebSocket actor.
struct MyWs;

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let mut rx = EVENT_BROKER.subscribe();
        let addr = ctx.address();

        actix_rt::spawn(async move {
            while let Ok(event) = rx.recv().await {
                if let Event::WebsocketStateUpdate(_) = event {
                    addr.do_send(event);
                }
            }
        });
    }
}

/// Handler for `Event` messages.
impl Handler<Event> for MyWs {
    type Result = ();

    fn handle(&mut self, msg: Event, ctx: &mut Self::Context) {
        if let Event::WebsocketStateUpdate(state) = msg {
            // Serialize the state to JSON and send it to the client.
            if let Ok(json) = serde_json::to_string(&state) {
                ctx.text(json);
            }
        }
    }
}

/// Handler for WebSocket messages.
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                let text = text.to_string();
                if text == "request_quit" {
                    EVENT_BROKER.publish(crate::event::Event::RequestQuit);
                } else if text == "force_quit" {
                    EVENT_BROKER.publish(crate::event::Event::ForceQuit);
                }
            }
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