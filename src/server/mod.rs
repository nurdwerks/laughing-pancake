// src/server/mod.rs

use crate::event::{Event, SelectionAlgorithm, WebsocketState, WsMessage, EVENT_BROKER};
use crate::ga::{Generation, GenerationConfig, Match};
use crate::game::search::SearchConfig;
use crate::sts::{StsResult, StsRunner};
use actix::{Actor, AsyncContext, Handler, Message, StreamHandler};
use actix_files as fs;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::{fs as std_fs, io, time::Duration};

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
    selection_algorithm: SelectionAlgorithm,
}

#[derive(Serialize)]
struct ApiIndividual {
    id: usize,
    config: SearchConfig,
    elo: f64,
    config_hash: u64,
}

#[derive(Serialize)]
struct IndividualDetails {
    individual: ApiIndividual,
    matches: Vec<Match>,
}

#[derive(Serialize, Clone, Debug)]
pub struct StsRunResponse {
    pub config_hash: u64,
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
                        "/generation/{id}/config",
                        web::get().to(get_generation_config),
                    )
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

async fn get_generation_config(path: web::Path<u32>) -> impl Responder {
    let gen_id = path.into_inner();
    let file_path = Path::new("evolution").join(format!("generation_{gen_id}_config.json"));

    match std_fs::read_to_string(file_path) {
        Ok(json_content) => match serde_json::from_str::<GenerationConfig>(&json_content) {
            Ok(config) => HttpResponse::Ok().json(config),
            Err(e) => HttpResponse::InternalServerError().body(format!("Deserialization error: {e}")),
        },
        Err(e) => {
            HttpResponse::NotFound().body(format!("Could not read generation config file: {e}"))
        }
    }
}

async fn get_generation_details(path: web::Path<u32>) -> impl Responder {
    let gen_id = path.into_inner();
    let file_path = Path::new("evolution").join(format!("generation_{gen_id}.json"));

    match std_fs::read_to_string(file_path) {
        Ok(json_content) => match serde_json::from_str::<Generation>(&json_content) {
            Ok(mut gen) => {
                gen.population.individuals.sort_by(|a, b| {
                    b.elo.partial_cmp(&a.elo).unwrap_or(std::cmp::Ordering::Equal)
                });
                HttpResponse::Ok().json(gen)
            }
            Err(e) => HttpResponse::InternalServerError().body(format!("Deserialization error: {e}")),
        },
        Err(e) => HttpResponse::NotFound().body(format!("Could not read generation file: {e}")),
    }
}

async fn get_individual_details(path: web::Path<(u32, u32)>) -> impl Responder {
    let (gen_id, ind_id) = path.into_inner();
    let file_path = Path::new("evolution").join(format!("generation_{gen_id}.json"));

    match std_fs::read_to_string(file_path) {
        Ok(json_content) => match serde_json::from_str::<Generation>(&json_content) {
            Ok(gen) => {
                if let Some(individual) = gen
                    .population
                    .individuals
                    .iter()
                    .find(|i| i.id == ind_id as usize)
                {
                    let config_hash = StsRunner::new(individual.config.clone()).config_hash();

                    let api_individual = ApiIndividual {
                        id: individual.id,
                        config: individual.config.clone(),
                        elo: individual.elo,
                        config_hash,
                    };

                    let individual_name = format!("individual_{ind_id}.json");
                    let matches = gen
                        .matches
                        .into_iter()
                        .filter(|m| {
                            m.white_player_name == individual_name
                                || m.black_player_name == individual_name
                        })
                        .collect();

                    HttpResponse::Ok().json(IndividualDetails {
                        individual: api_individual,
                        matches,
                    })
                } else {
                    HttpResponse::NotFound().body(format!(
                        "Individual {ind_id} not found in generation {gen_id}"
                    ))
                }
            }
            Err(e) => HttpResponse::InternalServerError().body(format!("Deserialization error: {e}")),
        },
        Err(e) => HttpResponse::NotFound().body(format!("Could not read generation file: {e}")),
    }
}

fn read_generations_summary() -> io::Result<Vec<GenerationSummary>> {
    let mut summaries = Vec::new();
    let evolution_dir = Path::new("evolution");

    if !evolution_dir.exists() {
        return Ok(summaries);
    }

    let paths: Vec<_> = std_fs::read_dir(evolution_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.to_string_lossy().contains("generation_") && p.extension().is_some_and(|e| e == "json"))
        .collect();

    for path in paths {
        if let Ok(json_content) = std_fs::read_to_string(&path) {
            if let Ok(gen) = serde_json::from_str::<Generation>(&json_content) {
                let config_path = evolution_dir.join(format!("generation_{}_config.json", gen.generation_index));
                let selection_algorithm = std_fs::read_to_string(config_path)
                    .ok()
                    .and_then(|json| serde_json::from_str::<GenerationConfig>(&json).ok())
                    .map(|config| config.selection_algorithm)
                    .unwrap_or(SelectionAlgorithm::SwissTournament); // Default for older generations

                let white_wins = gen.matches.iter().filter(|m| m.result == "1-0").count();
                let black_wins = gen.matches.iter().filter(|m| m.result == "0-1").count();
                let draws = gen.matches.iter().filter(|m| m.result == "1/2-1/2").count();
                let elos: Vec<f64> = gen.population.individuals.iter().map(|i| i.elo).collect();
                let top_elo = elos.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let lowest_elo = elos.iter().cloned().fold(f64::INFINITY, f64::min);
                let average_elo = if elos.is_empty() { 0.0 } else { elos.iter().sum::<f64>() / elos.len() as f64 };

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
                    selection_algorithm,
                });
            }
        }
    }

    summaries.sort_by_key(|s| s.generation_index);
    Ok(summaries)
}

#[derive(Message)]
#[rtype(result = "()")]
struct SubscribeToSts {
    hash: u64,
}

#[derive(Message)]
#[rtype(result = "()")]
struct SendToClient {
    json_payload: String,
}

async fn run_sts_test(path: web::Path<(u32, u32)>) -> impl Responder {
    let (gen_id, ind_id) = path.into_inner();
    match run_sts_test_logic(gen_id, ind_id).await {
        Ok(config_hash) => HttpResponse::Ok().json(StsRunResponse { config_hash }),
        Err(response) => response,
    }
}

async fn run_sts_test_logic(gen_id: u32, ind_id: u32) -> Result<u64, HttpResponse> {
    let gen_file_path = Path::new("evolution").join(format!("generation_{gen_id}.json"));

    let json_content = match std_fs::read_to_string(gen_file_path) {
        Ok(content) => content,
        Err(e) => {
            return Err(HttpResponse::NotFound().body(format!("Could not read generation file: {e}")))
        }
    };

    let gen: Generation = match serde_json::from_str(&json_content) {
        Ok(g) => g,
        Err(e) => {
            return Err(HttpResponse::InternalServerError()
                .body(format!("Deserialization error: {e}")))
        }
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

        Ok(config_hash)
    } else {
        Err(HttpResponse::NotFound().body(format!("Individual {ind_id} not found")))
    }
}

async fn get_sts_result(path: web::Path<u64>) -> impl Responder {
    let config_hash = path.into_inner();
    let result_path = Path::new("sts_results").join(format!("{config_hash}.json"));

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
    StsGlobal,
}

#[derive(Deserialize)]
struct WebSocketRequest {
    subscribe: Option<String>,
    action: Option<String>,
    config_hash: Option<u64>,
    gen_id: Option<u32>,
    ind_id: Option<u32>,
}

/// The WebSocket actor.
struct MyWs {
    subscriptions: HashSet<Subscription>,
    state_collector: Option<WebsocketState>,
}

impl MyWs {
    fn new() -> Self {
        Self {
            subscriptions: HashSet::new(),
            state_collector: None,
        }
    }
}

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let mut rx = EVENT_BROKER.subscribe();
        let addr = ctx.address();

        // Spawn a task to handle incoming events
        actix_rt::spawn(async move {
            while let Ok(event) = rx.recv().await {
                addr.do_send(event);
            }
        });

        // Spawn a task for periodic state updates
        let addr = ctx.address();
        actix_rt::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                addr.do_send(SendStateUpdate);
            }
        });
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct SendStateUpdate;

impl Handler<SendStateUpdate> for MyWs {
    type Result = ();

    fn handle(&mut self, _: SendStateUpdate, ctx: &mut Self::Context) {
        if let Some(state) = self.state_collector.take() {
            let ws_msg = WsMessage::State(state);
            if let Ok(json) = serde_json::to_string(&ws_msg) {
                ctx.text(json);
            }
        }
    }
}

impl Handler<Event> for MyWs {
    type Result = ();

    fn handle(&mut self, msg: Event, ctx: &mut Self::Context) {
        let ws_msg = match msg {
            Event::WebsocketStateUpdate(state) => {
                if self.subscriptions.contains(&Subscription::State) {
                    self.state_collector = Some(state);
                }
                None
            }
            Event::LogUpdate(log) => {
                if self.subscriptions.contains(&Subscription::Log) {
                    Some(WsMessage::Log(log))
                } else {
                    None
                }
            }
            Event::StsUpdate(update) => {
                let is_subscribed_specific = self
                    .subscriptions
                    .contains(&Subscription::Sts(update.config_hash));
                let is_subscribed_global = self.subscriptions.contains(&Subscription::StsGlobal);

                if is_subscribed_specific || is_subscribed_global {
                    Some(WsMessage::Sts(update))
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(ws_msg) = ws_msg {
            if let Ok(json) = serde_json::to_string(&ws_msg) {
                ctx.text(json);
            }
        }
    }
}

impl Handler<SubscribeToSts> for MyWs {
    type Result = ();

    fn handle(&mut self, msg: SubscribeToSts, _: &mut Self::Context) {
        self.subscriptions.insert(Subscription::Sts(msg.hash));
    }
}

impl Handler<SendToClient> for MyWs {
    type Result = ();

    fn handle(&mut self, msg: SendToClient, ctx: &mut Self::Context) {
        ctx.text(msg.json_payload);
    }
}

/// Handler for WebSocket messages.
async fn get_config_hash_for_individual(gen_id: u32, ind_id: u32) -> Option<u64> {
    let gen_file_path = Path::new("evolution").join(format!("generation_{gen_id}.json"));
    let json_content = std_fs::read_to_string(gen_file_path).ok()?;
    let gen: Generation = serde_json::from_str(&json_content).ok()?;
    gen.population
        .individuals
        .iter()
        .find(|i| i.id == ind_id as usize)
        .map(|individual| StsRunner::new(individual.config.clone()).config_hash())
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                let text_str = text.to_string();
                if let Ok(req) = serde_json::from_str::<WebSocketRequest>(&text_str) {
                    if let Some(sub_type) = req.subscribe {
                        match sub_type.as_str() {
                            "State" => {
                                self.subscriptions.insert(Subscription::State);
                            }
                            "Log" => {
                                self.subscriptions.insert(Subscription::Log);
                            }
                            "Sts" => {
                                if let Some(hash) = req.config_hash {
                                    self.subscriptions.insert(Subscription::Sts(hash));
                                } else {
                                    self.subscriptions.insert(Subscription::StsGlobal);
                                }
                            }
                            "StsIndividual" => {
                                if let (Some(gen_id), Some(ind_id)) = (req.gen_id, req.ind_id) {
                                    let addr = ctx.address();
                                    tokio::spawn(async move {
                                        if let Some(hash) =
                                            get_config_hash_for_individual(gen_id, ind_id).await
                                        {
                                            // Subscribe the actor to STS updates for this hash
                                            addr.do_send(SubscribeToSts { hash });

                                            // Also, send a message back to *this specific client*
                                            // to let it know which hash to listen for.
                                            let response = WsMessage::StsStarted(StsRunResponse {
                                                config_hash: hash,
                                            });
                                            if let Ok(json_payload) =
                                                serde_json::to_string(&response)
                                            {
                                                addr.do_send(SendToClient { json_payload });
                                            }
                                        }
                                    });
                                }
                            }
                            _ => (),
                        }
                    }

                    if let Some(action) = req.action {
                        if action.as_str() == "run_sts" {
                            if let (Some(gen_id), Some(ind_id)) = (req.gen_id, req.ind_id) {
                                let addr = ctx.address();
                                tokio::spawn(async move {
                                    if let Ok(config_hash) = run_sts_test_logic(gen_id, ind_id).await {
                                        let response = WsMessage::StsStarted(StsRunResponse {
                                            config_hash,
                                        });
                                        if let Ok(json_payload) =
                                            serde_json::to_string(&response)
                                        {
                                            addr.do_send(SendToClient { json_payload });
                                        }
                                    }
                                });
                            }
                        }
                    }
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