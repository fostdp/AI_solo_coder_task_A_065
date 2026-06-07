use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tokio::time::{self, Duration};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use tracing::{info, error, debug};
use serde::Serialize;

use crate::config::Config;
use crate::models::{AppState, MachineStatus};

#[derive(Clone)]
struct WsState {
    app_state: Arc<RwLock<AppState>>,
    broadcast_tx: broadcast::Sender<WsMessage>,
}

#[derive(Clone, Serialize)]
struct WsMessage {
    type_: String,
    data: serde_json::Value,
}

pub async fn start_websocket_server(
    config: Config,
    app_state: Arc<RwLock<AppState>>,
) -> anyhow::Result<()> {
    let (tx, _rx) = broadcast::channel(100);
    let ws_state = WsState {
        app_state: app_state.clone(),
        broadcast_tx: tx.clone(),
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(ws_state.clone());

    let broadcast_tx = tx.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let state = app_state.read().await;
            let statuses: Vec<MachineStatus> = state.machine_statuses
                .values()
                .cloned()
                .collect();
            
            if !statuses.is_empty() {
                let msg = WsMessage {
                    type_: "status_update".to_string(),
                    data: serde_json::to_value(&statuses).unwrap_or_default(),
                };
                let _ = broadcast_tx.send(msg);
            }
        }
    });

    let addr = format!("0.0.0.0:{}", config.server.websocket_port);
    info!("WebSocket server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<WsState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: WsState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.broadcast_tx.subscribe();

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    debug!("Received WS message: {}", text);
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    debug!("WebSocket connection closed");
}
