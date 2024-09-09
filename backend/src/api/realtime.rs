use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::Extension,
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::task;

type Clients = Arc<Mutex<HashMap<String, HashSet<String>>>>;

pub async fn realtime_handler(
    ws: WebSocketUpgrade,
    Extension(clients): Extension<Clients>,
    Extension(tx): Extension<broadcast::Sender<Update>>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, clients, tx))
}

#[derive(Deserialize)]
pub struct FollowingData {
    route_ids: Vec<String>,
}

#[derive(Clone)]
pub struct Update {
    pub route_id: String,
    pub data: serde_json::Value,
}

async fn handle_socket(mut socket: WebSocket, clients: Clients, tx: broadcast::Sender<Update>) {
    let mut followed_routes = HashSet::new();

    while let Some(Ok(message)) = socket.next().await {
        if let Message::Text(text) = message {
            if let Ok(follow_routes) = serde_json::from_str::<FollowingData>(&text) {
                followed_routes = follow_routes.route_ids.into_iter().collect();
                let client_id = uuid::Uuid::now_v7().to_string();
                clients
                    .lock()
                    .unwrap()
                    .insert(client_id.clone(), followed_routes.clone());

                let clients = clients.clone();
                let mut socket = socket;
                task::spawn(async move {
                    let mut rx = tx.clone().subscribe();

                    while let Ok(update) = rx.recv().await {
                        if followed_routes.contains(&update.route_id) {
                            let msg = serde_json::to_string(&update.data).unwrap();
                            if socket.send(Message::Text(msg)).await.is_err() {
                                break;
                            }
                        }
                    }
                    clients.lock().unwrap().remove(&client_id);
                });
            }
        }
    }
}
