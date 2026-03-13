use std::collections::HashMap;
use std::time::Duration;

use log::error;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::auth::AuthState;
use crate::messages::{fetch_room_messages_from_client, MatrixChatMessage};
use crate::rooms::{collect_chat_summaries, store_cached_chats, sync_client_rooms_once, MatrixChatSummary};

#[derive(Copy, Clone)]
pub enum RoomUpdateEvent {
    RoomAdded,
    RoomUpdated,
    RoomRemoved,
    SelectedRoomMessages,
}

impl RoomUpdateEvent {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RoomAdded => "matrix://rooms/added",
            Self::RoomUpdated => "matrix://rooms/updated",
            Self::RoomRemoved => "matrix://rooms/removed",
            Self::SelectedRoomMessages => "matrix://rooms/selected/messages",
        }
    }
}

#[derive(Clone)]
struct RoomRefreshTrigger {
    selected_room_id: Option<String>,
}

pub struct RoomUpdateTriggerState {
    sender: UnboundedSender<RoomRefreshTrigger>,
}

impl RoomUpdateTriggerState {
    fn new(sender: UnboundedSender<RoomRefreshTrigger>) -> Self {
        Self { sender }
    }

    fn enqueue(&self, trigger: RoomRefreshTrigger) -> Result<(), String> {
        self.sender
            .send(trigger)
            .map_err(|_| String::from("Room update worker is not available"))
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixTriggerRoomUpdateRequest {
    pub selected_room_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixTriggerRoomUpdateResponse {
    pub queued: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatrixRoomRemovedEvent {
    room_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatrixSelectedRoomMessagesEvent {
    room_id: String,
    next_from: Option<String>,
    messages: Vec<MatrixChatMessage>,
}

pub fn start_room_update_worker(app: AppHandle) -> RoomUpdateTriggerState {
    let (sender, mut receiver) = mpsc::unbounded_channel::<RoomRefreshTrigger>();
    let task_app = app.clone();

    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        let mut previous_snapshot: HashMap<String, MatrixChatSummary> = HashMap::new();

        loop {
            let mut trigger_room_id = None;

            tokio::select! {
                _ = interval.tick() => {}
                maybe_trigger = receiver.recv() => {
                    let Some(trigger) = maybe_trigger else {
                        break;
                    };

                    trigger_room_id = trigger.selected_room_id;

                    while let Ok(next_trigger) = receiver.try_recv() {
                        if next_trigger.selected_room_id.is_some() {
                            trigger_room_id = next_trigger.selected_room_id;
                        }
                    }
                }
            }

            if let Err(error) = run_refresh_pass(&task_app, &mut previous_snapshot, trigger_room_id).await {
                error!("Room update pass failed: {error}");
            }
        }
    });

    RoomUpdateTriggerState::new(sender)
}

#[tauri::command]
pub async fn matrix_trigger_room_update(
    request: Option<MatrixTriggerRoomUpdateRequest>,
    trigger_state: State<'_, RoomUpdateTriggerState>,
) -> Result<MatrixTriggerRoomUpdateResponse, String> {
    let payload = request.unwrap_or_default();

    trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: payload.selected_room_id,
    })?;

    Ok(MatrixTriggerRoomUpdateResponse { queued: true })
}

async fn run_refresh_pass(
    app: &AppHandle,
    previous_snapshot: &mut HashMap<String, MatrixChatSummary>,
    selected_room_id: Option<String>,
) -> Result<(), String> {
    let auth_state = app.state::<AuthState>();
    auth_state.restore_client_from_disk_if_needed(app).await?;

    let client = match auth_state.client() {
        Ok(client) => client,
        Err(_) => return Ok(()),
    };

    sync_client_rooms_once(&client).await?;

    let chats = collect_chat_summaries(&client).await;
    let _ = store_cached_chats(app, &chats);

    let mut current_snapshot: HashMap<String, MatrixChatSummary> = HashMap::new();
    for chat in chats {
        current_snapshot.insert(chat.room_id.clone(), chat);
    }

    for (room_id, chat) in &current_snapshot {
        match previous_snapshot.get(room_id) {
            None => {
                let _ = app.emit(RoomUpdateEvent::RoomAdded.as_str(), chat.clone());
            }
            Some(previous) if previous != chat => {
                let _ = app.emit(RoomUpdateEvent::RoomUpdated.as_str(), chat.clone());
            }
            Some(_) => {}
        }
    }

    for room_id in previous_snapshot.keys() {
        if !current_snapshot.contains_key(room_id) {
            let _ = app.emit(
                RoomUpdateEvent::RoomRemoved.as_str(),
                MatrixRoomRemovedEvent {
                    room_id: room_id.clone(),
                },
            );
        }
    }

    if let Some(room_id) = selected_room_id {
        if current_snapshot.contains_key(&room_id) {
            if let Ok(response) = fetch_room_messages_from_client(&client, &room_id, None, Some(50)).await {
                let _ = app.emit(
                    RoomUpdateEvent::SelectedRoomMessages.as_str(),
                    MatrixSelectedRoomMessagesEvent {
                        room_id: response.room_id,
                        next_from: response.next_from,
                        messages: response.messages,
                    },
                );
            }
        }
    }

    *previous_snapshot = current_snapshot;
    Ok(())
}
