use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::api::Direction;
use matrix_sdk::ruma::uint;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tauri::{AppHandle, State};

use crate::auth::AuthState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatMessagesRequest {
    pub room_id: String,
    pub from: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChatMessage {
    pub event_id: Option<String>,
    pub sender: String,
    pub timestamp: Option<u64>,
    pub body: String,
    pub encrypted: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatMessagesResponse {
    pub room_id: String,
    pub next_from: Option<String>,
    pub messages: Vec<MatrixChatMessage>,
}

#[tauri::command]
pub async fn matrix_get_chat_messages(
    request: MatrixGetChatMessagesRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatMessagesResponse, String> {
    auth_state.restore_client_from_disk_if_needed(&app_handle).await?;
    let client = auth_state.client()?;
    client
        .sync_once(matrix_sdk::config::SyncSettings::default().timeout(Duration::from_secs(5)))
        .await
        .map_err(|error| format!("Failed to sync Matrix room messages: {error}"))?;

    fetch_room_messages_from_client(&client, request.room_id.as_str(), request.from, request.limit).await
}

pub(crate) async fn fetch_room_messages_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    from: Option<String>,
    limit: Option<u32>,
) -> Result<MatrixGetChatMessagesResponse, String> {
    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_raw)
        .map_err(|_| String::from("roomId is invalid"))?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    let mut options = MessagesOptions::new(Direction::Backward);
    if let Some(from) = from {
        options.from = Some(from);
    }

    options.limit = uint!(50);
    if let Some(limit) = limit {
        options.limit = limit.min(100).into();
    }

    let response = room
        .messages(options)
        .await
        .map_err(|error| format!("Failed to read room messages: {error}"))?;

    let mut messages = Vec::new();
    for timeline in response.chunk {
        let is_encrypted_event = timeline.encryption_info().is_some() || timeline.kind.is_utd();
        let Ok(event) = timeline.raw().deserialize_as::<Value>() else {
            continue;
        };

        let event_type = event.get("type").and_then(Value::as_str).unwrap_or_default();
        let sender = event
            .get("sender")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        let event_id = event
            .get("event_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let timestamp = event.get("origin_server_ts").and_then(Value::as_u64);

        if event_type == "m.room.message" {
            let msgtype = event
                .get("content")
                .and_then(|content| content.get("msgtype"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let body = event
                .get("content")
                .and_then(|content| content.get("body"))
                .and_then(Value::as_str)
                .unwrap_or("Unsupported message")
                .to_owned();

            let text_body = if msgtype == "m.text" || msgtype == "m.notice" || msgtype == "m.emote" {
                body
            } else {
                format!("Unsupported message type: {msgtype}")
            };

            messages.push(MatrixChatMessage {
                event_id,
                sender,
                timestamp,
                body: text_body,
                encrypted: is_encrypted_event,
            });
        } else if event_type == "m.room.encrypted" {
            messages.push(MatrixChatMessage {
                event_id,
                sender,
                timestamp,
                body: String::from("Encrypted message"),
                encrypted: true,
            });
        }
    }

    messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    Ok(MatrixGetChatMessagesResponse {
        room_id: room_id.to_string(),
        next_from: response.end,
        messages,
    })
}
