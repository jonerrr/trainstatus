use crate::AppState;
use axum::extract::{
    ws::{Message, WebSocket, WebSocketUpgrade},
    Extension, State,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub type Clients = Arc<Mutex<HashMap<String, HashSet<String>>>>;

pub async fn realtime_handler(
    ws: WebSocketUpgrade,
    // Extension(clients): Extension<Clients>,
    // Extension(tx): Extension<crossbeam::channel::Sender<serde_json::Value>>,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.clients, state.rx, state.initial_data))
}

#[derive(Deserialize)]
pub struct FollowingData {
    route_ids: Vec<String>,
}

#[derive(Clone)]
pub struct Update {
    pub route_id: String,
    pub data_type: String,
    pub data: serde_json::Value,
}

async fn handle_socket(
    socket: WebSocket,
    clients: Clients,
    // tx: crossbeam::channel::Sender<serde_json::Value>,
    rx: crossbeam::channel::Receiver<Vec<Update>>,
    initial_data: Arc<RwLock<serde_json::Value>>,
) {
    // TODO: maybe switch to message::binary
    let (mut sender, mut receiver) = socket.split();

    sender
        .send(Message::Text(initial_data.read().await.to_string()))
        .await
        .unwrap();

    let client_id = uuid::Uuid::now_v7().to_string();
    clients
        .lock()
        .await
        .insert(client_id.clone(), HashSet::new());

    let recv_clients = clients.clone();
    let recv_client_id = client_id.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(follow_routes) = serde_json::from_str::<FollowingData>(&text) {
                        // dbg!(&follow_routes);

                        let mut clients = recv_clients.lock().await;

                        clients.insert(
                            recv_client_id.clone(),
                            follow_routes.route_ids.into_iter().collect(),
                        );
                    }
                }
                _ => {}
            }
        }
    });

    // let mut rx = rx.clone();
    let send_clients = clients.clone();
    let mut send_task = tokio::spawn(async move {
        while let Ok(update) = rx.recv() {
            let clients = send_clients.lock().await;

            for (_client_id, followed_routes) in clients.iter() {
                let updates = update
                    .iter()
                    .filter_map(|u| {
                        // we only send specific stop_times for buses. For train we send all
                        if u.data_type == "train" || followed_routes.contains(&u.route_id) {
                            Some(&u.data)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                if updates.is_empty() {
                    continue;
                }

                let msg = serde_json::to_string(&updates).unwrap();
                if sender.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }

            // for (client_id, followed_routes) in clients.iter() {
            //     if followed_routes.contains(&update.route_id) {
            //         let msg = serde_json::to_string(&update.data).unwrap();
            //         if sender.send(Message::Text(msg)).await.is_err() {
            //             break;
            //         }
            //     }
            // }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };

    // remove client from clients when recv_task is done
    clients.lock().await.remove(&client_id);

    // let mut followed_routes = HashSet::new();

    // let mut recv_task = tokio::spawn(async move {
    //     // let mut cnt = 0;
    //     while let Some(Ok(msg)) = receiver.next().await {
    //         dbg!(&msg);

    //         match msg {
    //             Message::Text(text) => {
    //                 if let Ok(follow_routes) = serde_json::from_str::<FollowingData>(&text) {
    //                     let mut clients = clients.lock().await;
    //                     let client_id = uuid::Uuid::now_v7().to_string();
    //                     clients.insert(client_id, follow_routes.route_ids.into_iter().collect());

    //                     // let clients = clients.clone();

    //                     // let tx = tx.clone();
    //                     // tokio::spawn(async move {
    //                     //     let mut rx = tx.subscribe();

    //                     //     while let Ok(update) = rx.recv().await {
    //                     //         if clients.get(&client_id).unwrap().contains(&update.route_id) {
    //                     //             let msg = serde_json::to_string(&update.data).unwrap();
    //                     //             if sender.send(Message::Text(msg)).await.is_err() {
    //                     //                 break;
    //                     //             }
    //                     //         }
    //                     //     }
    //                     //     clients.remove(&client_id);
    //                     // });
    //                 }
    //             }
    //             // Message::Close(_) => break,
    //             _ => {}
    //         }

    //         // cnt += 1;
    //         // // print message and break if instructed to do so
    //         // if process_message(msg, who).is_break() {
    //         //     break;
    //         // }
    //     }
    //     // cnt
    // })
    // .await;

    // tokio::select! {
    //     _ = recv_task => {
    //         // println!("{} received {} messages", who, cnt);
    //     }
    //     _ = async {
    //         let mut rx = tx.subscribe();
    //         while let Ok(update) = rx.recv().await {
    //             if sender.send(Message::Text(update.data.to_string())).await.is_err() {
    //                 break;
    //             }
    //         }
    //     } => {
    //         // println!("{} sent {} messages", who, cnt);
    //     }
    // }

    // while let Some(Ok(message)) = socket.next().await {
    //     if let Message::Text(text) = message {
    //         if let Ok(follow_routes) = serde_json::from_str::<FollowingData>(&text) {
    //             followed_routes = follow_routes.route_ids.into_iter().collect();
    //             let client_id = uuid::Uuid::now_v7().to_string();
    //             clients
    //                 .lock()
    //                 .unwrap()
    //                 .insert(client_id.clone(), followed_routes.clone());

    //             let clients = clients.clone();
    //             let tx = tx.clone();
    //             task::spawn(async move {
    //                 let mut rx = tx.subscribe();

    //                 while let Ok(update) = rx.recv().await {
    //                     if followed_routes.contains(&update.route_id) {
    //                         let msg = serde_json::to_string(&update.data).unwrap();
    //                         if socket.send(Message::Text(msg)).await.is_err() {
    //                             break;
    //                         }
    //                     }
    //                 }
    //                 clients.lock().unwrap().remove(&client_id);
    //             });
    //         }
    //     }
    // }
}
