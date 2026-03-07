mod board;
mod eval;
mod flood;
mod logic;
mod search;
mod types;

use axum::{routing::{get, post}, Json, Router};
use std::net::SocketAddr;
use types::{GameState, InfoResponse, MoveResponse};
use tracing::info;

/// GET / — Snake customization & metadata.
async fn handle_index() -> Json<InfoResponse> {
    Json(InfoResponse {
        apiversion: "1",
        author: "ouroboros",
        color: "#e63946",
        head: "evil",
        tail: "bolt",
        version: "1.0.0",
    })
}

/// POST /start — Game started (no-op).
async fn handle_start(Json(state): Json<GameState>) -> &'static str {
    info!(game_id = %state.game.id, "Game started");
    "ok"
}

/// POST /move — Main move endpoint.
async fn handle_move(Json(state): Json<GameState>) -> Json<MoveResponse> {
    let dir = logic::get_move(&state);
    Json(MoveResponse {
        mv: dir,
        shout: None,
    })
}

/// POST /end — Game ended (no-op).
async fn handle_end(Json(state): Json<GameState>) -> &'static str {
    info!(game_id = %state.game.id, turn = state.turn, "Game ended");
    "ok"
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "battlesnake=info".into()),
        )
        .compact()
        .init();

    let app = Router::new()
        .route("/", get(handle_index))
        .route("/start", post(handle_start))
        .route("/move", post(handle_move))
        .route("/end", post(handle_end));

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Battlesnake server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
