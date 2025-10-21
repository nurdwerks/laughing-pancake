// src/server/mod.rs

use crate::event::{Event, WsMessage, EVENT_BROKER};
use crate::ga::{Generation, Individual, Match};
use crate::sts::{StsResult, StsRunner};
use actix::{Actor, AsyncContext, Handler, StreamHandler};
use actix_files as fs;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::{fs as std_fs, io};

#[derive(Serialize)]
struct GenerationSummary {
    generation_index: u32,
    num_individuals: usize,
    num_matches: usize,
    white_wins: usize,
    black_wins: usize,
    draws: usize,
    top_elo: f64,
    average_elo: f64,
    lowest_elo: f64,
}

#[derive(Serialize)]
struct IndividualDetails {
    individual: Individual,
    matches: Vec<Match>,
}

#[derive(Serialize)]
struct StsRunResponse {
    config_hash: u64,
}

/// The main entry point for the web server.
pub async fn start_server() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/ws", web::get().to(ws_index))
            .service(
                web::scope("/api")
                    .route("/generations", web::get().to(get_generations))
                    .route("/generation/{id}", web::get().to(get_generation_details))
                    .route(
                        "/individual/{gen_id}/{ind_id}",
                        web::get().to(get_individual_details),
                    )
                    .route("/sts/run/{gen_id}/{ind_id}", web::post().to(run_sts_test))
                    .route("/sts/result/{config_hash}", web::get().to(get_sts_result)),
            )
            .service(fs::Files::new("/", "./static").index_file("index.html"))
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
}

async fn get_generations() -> impl Responder {
    match read_generations_summary() {
        Ok(summaries) => HttpResponse::Ok().json(summaries),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn get_generation_details(path: web::Path<u32>) -> impl Responder {
    let gen_id = path.into_inner();
    let file_path = Path::new("evolution").join(format!("generation_{}.json", gen_id));

    match std_fs::read_to_string(file_path) {
        Ok(json_content) => match serde_json::from_str::<Generation>(&json_content) {
            Ok(mut gen) => {
                gen.population.individuals.sort_by(|a, b| {
                    b.elo.partial_cmp(&a.elo).unwrap_or(std::cmp::Ordering::Equal)
                });
                HttpResponse::Ok().json(gen)
            }
            Err(e) => HttpResponse::InternalServerError().body(format!("Deserialization error: {}", e)),
        },
        Err(e) => HttpResponse::NotFound().body(format!("Could not read generation file: {}", e)),
    }
}

async fn get_individual_details(path: web::Path<(u32, u32)>) -> impl Responder {
    let (gen_id, ind_id) = path.into_inner();
    let file_path = Path::new("evolution").join(format!("generation_{}.json", gen_id));

    match std_fs::read_to_string(file_path) {
        Ok(json_content) => match serde_json::from_str::<Generation>(&json_content) {
            Ok(gen) => {
                if let Some(individual) = gen
                    .population
                    .individuals
                    .iter()
                    .find(|i| i.id == ind_id as usize)
                    .cloned()
                {
                    let individual_name = format!("individual_{}.json", ind_id);
                    let matches = gen
                        .matches
                        .into_iter()
                        .filter(|m| {
                            m.white_player_name == individual_name
                                || m.black_player_name == individual_name
                        })
                        .collect();

                    HttpResponse::Ok().json(IndividualDetails { individual, matches })
                } else {
                    HttpResponse::NotFound().body(format!(
                        "Individual {} not found in generation {}",
                        ind_id, gen_id
                    ))
                }
            }
            Err(e) => HttpResponse::InternalServerError().body(format!("Deserialization error: {}", e)),
        },
        Err(e) => HttpResponse::NotFound().body(format!("Could not read generation file: {}", e)),
    }
}

fn read_generations_summary() -> io::Result<Vec<GenerationSummary>> {
    let mut summaries = Vec::new();
    let evolution_dir = Path::new("evolution");

    if !evolution_dir.exists() {
        return Ok(summaries);
    }

    for entry in std_fs::read_dir(evolution_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.to_string_lossy().contains("generation_") {
            if let Ok(json_content) = std_fs::read_to_string(&path) {
                if let Ok(gen) = serde_json::from_str::<Generation>(&json_content) {
                    let white_wins = gen.matches.iter().filter(|m| m.result == "1-0").count();
                    let black_wins = gen.matches.iter().filter(|m| m.result == "0-1").count();
                    let draws = gen.matches.iter().filter(|m| m.result == "1/2-1/2").count();
                    let elos: Vec<f64> = gen.population.individuals.iter().map(|i| i.elo).collect();
                    let top_elo = elos.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let lowest_elo = elos.iter().cloned().fold(f64::INFINITY, f64::min);
                    let average_elo = elos.iter().sum::<f64>() / elos.len() as f64;

                    summaries.push(GenerationSummary {
                        generation_index: gen.generation_index,
                        num_individuals: gen.population.individuals.len(),
                        num_matches: gen.matches.len(),
                        white_wins,
                        black_wins,
                        draws,
                        top_elo,
                        average_elo,
                        lowest_elo,
                    });
                }
            }
        }
    }
    summaries.sort_by_key(|s| s.generation_index);
    Ok(summaries)
}

async fn run_sts_test(path: web::Path<(u32, u32)>) -> impl Responder {
    let (gen_id, ind_id) = path.into_inner();
    let gen_file_path = Path::new("evolution").join(format!("generation_{}.json", gen_id));

    let json_content = match std_fs::read_to_string(gen_file_path) {
        Ok(content) => content,
        Err(e) => return HttpResponse::NotFound().body(format!("Could not read generation file: {}", e)),
    };

    let gen: Generation = match serde_json::from_str(&json_content) {
        Ok(g) => g,
        Err(e) => return HttpResponse::InternalServerError().body(format!("Deserialization error: {}", e)),
    };

    if let Some(individual) = gen
        .population
        .individuals
        .iter()
        .find(|i| i.id == ind_id as usize)
    {
        let mut runner = StsRunner::new(individual.config.clone());
        let config_hash = runner.config_hash();
        tokio::spawn(async move {
            runner.run().await;
        });

        HttpResponse::Ok().json(StsRunResponse { config_hash })
    } else {
        HttpResponse::NotFound().body(format!("Individual {} not found", ind_id))
    }
}

async fn get_sts_result(path: web::Path<u64>) -> impl Responder {
    let config_hash = path.into_inner();
    let result_path = Path::new("sts_results").join(format!("{}.json", config_hash));

    match std_fs::read_to_string(result_path) {
        Ok(json) => match serde_json::from_str::<StsResult>(&json) {
            Ok(result) => HttpResponse::Ok().json(result),
            Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
        },
        Err(_) => HttpResponse::NotFound().finish(),
    }
}


#[derive(Debug, Eq, PartialEq, Hash)]
enum Subscription {
    State,
    Log,
    Sts(u64),
}

#[derive(Deserialize)]
struct SubscriptionRequest {
    subscribe: String,
    config_hash: Option<u64>,
}

/// The WebSocket actor.
struct MyWs {
    subscriptions: HashSet<Subscription>,
}

impl MyWs {
    fn new() -> Self {
        Self {
            subscriptions: HashSet::new(),
        }
    }
}

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let mut rx = EVENT_BROKER.subscribe();
        let addr = ctx.address();

        actix_rt::spawn(async move {
            while let Ok(event) = rx.recv().await {
                match event {
                    Event::WebsocketStateUpdate(_) | Event::LogUpdate(_) | Event::StsUpdate(_) => {
                        addr.do_send(event);
                    }
                    _ => {} // Ignore other events
                }
            }
        });
    }
}

/// Handler for `Event` messages.
impl Handler<Event> for MyWs {
    type Result = ();

    fn handle(&mut self, msg: Event, ctx: &mut Self::Context) {
        let ws_msg = match msg {
            Event::WebsocketStateUpdate(state) => {
                if self.subscriptions.contains(&Subscription::State) {
                    WsMessage::State(state)
                } else {
                    return;
                }
            }
            Event::LogUpdate(log) => {
                if self.subscriptions.contains(&Subscription::Log) {
                    WsMessage::Log(log)
                } else {
                    return;
                }
            }
            Event::StsUpdate(update) => {
                if self
                    .subscriptions
                    .contains(&Subscription::Sts(update.config_hash))
                {
                    WsMessage::Sts(update)
                } else {
                    return;
                }
            }
            _ => return,
        };

        if let Ok(json) = serde_json::to_string(&ws_msg) {
            ctx.text(json);
        }
    }
}

/// Handler for WebSocket messages.
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                let text_str = text.to_string();
                if let Ok(req) = serde_json::from_str::<SubscriptionRequest>(&text_str) {
                    match req.subscribe.as_str() {
                        "State" => {
                            self.subscriptions.insert(Subscription::State);
                        }
                        "Log" => {
                            self.subscriptions.insert(Subscription::Log);
                        }
                        "Sts" => {
                            if let Some(hash) = req.config_hash {
                                self.subscriptions.insert(Subscription::Sts(hash));
                            }
                        }
                        _ => (),
                    }
                } else if text_str == "request_quit" {
                    EVENT_BROKER.publish(crate::event::Event::RequestQuit);
                } else if text_str == "force_quit" {
                    EVENT_BROKER.publish(crate::event::Event::ForceQuit);
                } else if text_str == "reset_simulation" {
                    EVENT_BROKER.publish(crate::event::Event::ResetSimulation);
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
    ws::start(MyWs::new(), &r, stream)
}