// src/server/mod.rs

use crate::event::{Event, SelectionAlgorithm, WebsocketState, WsMessage, EVENT_BROKER};
use crate::ga::{Generation, GenerationConfig, Match, SelectionModeConfig};
use crate::game::search::SearchConfig;
use crate::sts::{StsResult, StsRunner};
use actix::{Actor, AsyncContext, Handler, Message, StreamHandler};
use actix_files as fs;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::{fs as std_fs, io, time::Duration};

#[derive(Serialize, Clone)]
pub struct GenerationSummary {
    pub generation_index: u32,
    pub num_individuals: usize,
    pub num_matches: usize,
    pub white_wins: usize,
    pub black_wins: usize,
    pub draws: usize,
    pub top_elo: f64,
    pub average_elo: f64,
    pub lowest_elo: f64,
    pub selection_algorithm: SelectionAlgorithm,
}

#[derive(Serialize, Clone)]
pub struct ApiIndividual {
    pub id: usize,
    pub config: SearchConfig,
    pub elo: f64,
    pub config_hash: u64,
}

#[derive(Serialize)]
pub struct IndividualDetails {
    pub individual: ApiIndividual,
    pub matches: Vec<Match>,
}

#[derive(Serialize, Clone)]
pub struct ApiGenerationDetails {
    pub generation_index: u32,
    pub round: u32,
    pub population: Vec<ApiIndividual>,
    pub matches: Vec<Match>,
    pub sts_results: Option<Vec<StsResult>>,
}

#[derive(Serialize, Clone, Debug)]
pub struct StsRunResponse {
    pub config_hash: u64,
}

/// The main entry point for the web server.
pub async fn start_server(mock_scenario: Option<String>) -> std::io::Result<()> {
    HttpServer::new(move || {
        let app_data = web::Data::new(mock_scenario.clone());
        App::new()
            .app_data(app_data)
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
                    .route("/sts/result/{config_hash}", web::get().to(get_sts_result))
                    .route("/selection_mode", web::get().to(get_selection_mode))
                    .route("/selection_mode", web::post().to(set_selection_mode)),
            )
            .service(fs::Files::new("/", "./static").index_file("index.html"))
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
}

async fn get_generations(mock_scenario: web::Data<Option<String>>) -> impl Responder {
    if let Some(scenario) = mock_scenario.get_ref() {
        let mock_data = match scenario.as_str() {
            "A" => crate::mock_api::MOCK_GENERATIONS_A.to_vec(),
            "B" => crate::mock_api::MOCK_GENERATIONS_B.to_vec(),
            "C" => crate::mock_api::MOCK_GENERATIONS_C.to_vec(),
            _ => vec![],
        };
        return HttpResponse::Ok().json(mock_data);
    }

    match read_generations_summary() {
        Ok(summaries) => HttpResponse::Ok().json(summaries),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn get_generation_config(
    path: web::Path<u32>,
    mock_scenario: web::Data<Option<String>>,
) -> impl Responder {
    if let Some(scenario) = mock_scenario.get_ref() {
        let gen_id = path.into_inner();
        if scenario == "A" || scenario == "B" {
            if gen_id == 0 {
                return HttpResponse::Ok().json(&*crate::mock_api::MOCK_CONFIG_B0);
            } else if gen_id == 1 {
                return HttpResponse::Ok().json(&*crate::mock_api::MOCK_CONFIG_B1);
            }
        }
        return HttpResponse::NotFound().finish();
    }

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

async fn get_generation_details(
    path: web::Path<u32>,
    mock_scenario: web::Data<Option<String>>,
) -> impl Responder {
    if let Some(scenario) = mock_scenario.get_ref() {
        let gen_id = path.into_inner();
        if scenario == "A" {
            if gen_id == 0 {
                return HttpResponse::Ok().json(&*crate::mock_api::MOCK_GENERATION_DETAILS_A0);
            } else if gen_id == 1 {
                return HttpResponse::Ok().json(&*crate::mock_api::MOCK_GENERATION_DETAILS_A1);
            }
        } else if scenario == "B" {
            if gen_id == 0 {
                return HttpResponse::Ok().json(&*crate::mock_api::MOCK_GENERATION_DETAILS_B0);
            } else if gen_id == 1 {
                return HttpResponse::Ok().json(&*crate::mock_api::MOCK_GENERATION_DETAILS_B1);
            }
        }
        return HttpResponse::NotFound().finish();
    }

    let gen_id = path.into_inner();
    let file_path = Path::new("evolution").join(format!("generation_{gen_id}.json"));

    match std_fs::read_to_string(file_path) {
        Ok(json_content) => match serde_json::from_str::<Generation>(&json_content) {
            Ok(mut gen) => {
                gen.population.individuals.sort_by(|a, b| {
                    b.elo.partial_cmp(&a.elo).unwrap_or(std::cmp::Ordering::Equal)
                });

                let api_population = gen
                    .population
                    .individuals
                    .into_iter()
                    .map(|ind| {
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        ind.config.hash(&mut hasher);
                        ApiIndividual {
                            id: ind.id,
                            config: ind.config,
                            elo: ind.elo,
                            config_hash: hasher.finish(),
                        }
                    })
                    .collect();

                let response = ApiGenerationDetails {
                    generation_index: gen.generation_index,
                    round: gen.round,
                    population: api_population,
                    matches: gen.matches,
                    sts_results: gen.sts_results,
                };
                HttpResponse::Ok().json(response)
            }
            Err(e) => HttpResponse::InternalServerError().body(format!("Deserialization error: {e}")),
        },
        Err(e) => HttpResponse::NotFound().body(format!("Could not read generation file: {e}")),
    }
}

async fn get_individual_details(
    path: web::Path<(u32, u32)>,
    mock_scenario: web::Data<Option<String>>,
) -> impl Responder {
    if let Some(scenario) = mock_scenario.get_ref() {
        let (gen_id, ind_id) = path.into_inner();
        if (scenario == "A" || scenario == "B") && gen_id == 0 && ind_id == 0 {
            let individual = crate::mock_api::MOCK_INDIVIDUAL_B0_0.clone();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            individual.config.hash(&mut hasher);
            let api_individual = ApiIndividual {
                id: individual.id,
                config: individual.config,
                elo: individual.elo,
                config_hash: hasher.finish(),
            };
            let details = IndividualDetails {
                individual: api_individual,
                matches: vec![],
            };
            return HttpResponse::Ok().json(details);
        }
        return HttpResponse::NotFound().finish();
    }

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
    mock_scenario: Option<String>,
}

impl MyWs {
    fn new(mock_scenario: Option<String>) -> Self {
        Self {
            subscriptions: HashSet::new(),
            state_collector: None,
            mock_scenario,
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
        if let Some(scenario) = &self.mock_scenario {
            let mock_state = match scenario.as_str() {
                "A" => crate::mock_api::MOCK_WEBSOCKET_STATE_A.clone(),
                "B" => crate::mock_api::MOCK_WEBSOCKET_STATE_B.clone(),
                "C" => crate::mock_api::MOCK_WEBSOCKET_STATE_C.clone(),
                _ => return,
            };
            let ws_msg = WsMessage::State(mock_state);
            if let Ok(json) = serde_json::to_string(&ws_msg) {
                ctx.text(json);
            }
            return;
        }

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
async fn ws_index(
    r: HttpRequest,
    stream: web::Payload,
    mock_scenario: web::Data<Option<String>>,
) -> Result<HttpResponse, Error> {
    ws::start(MyWs::new(mock_scenario.get_ref().clone()), &r, stream)
}

async fn get_selection_mode() -> impl Responder {
    let config = SelectionModeConfig::load();
    HttpResponse::Ok().json(config)
}

async fn set_selection_mode(new_config: web::Json<SelectionModeConfig>) -> impl Responder {
    match new_config.save() {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}