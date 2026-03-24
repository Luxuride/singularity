pub(crate) mod commands;
mod persistence;
mod types;
mod workers;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::messages::MatrixChatMessage;
use crate::protocol::event_paths;

pub use types::MatrixChatSummary;
pub use workers::start_room_update_worker;

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
            Self::RoomAdded => event_paths::ROOM_ADDED,
            Self::RoomUpdated => event_paths::ROOM_UPDATED,
            Self::RoomRemoved => event_paths::ROOM_REMOVED,
            Self::SelectedRoomMessages => event_paths::SELECTED_ROOM_MESSAGES,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RoomRefreshTrigger {
    pub(crate) selected_room_id: Option<String>,
    pub(crate) include_selected_messages: bool,
}

pub struct RoomUpdateTriggerState {
    sender: UnboundedSender<RoomRefreshTrigger>,
}

impl RoomUpdateTriggerState {
    pub(crate) fn new(sender: UnboundedSender<RoomRefreshTrigger>) -> Self {
        Self { sender }
    }

    pub(crate) fn enqueue(&self, trigger: RoomRefreshTrigger) -> Result<(), String> {
        self.sender
            .send(trigger)
            .map_err(|_| String::from("Room update worker is not available"))
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixTriggerRoomUpdateRequest {
    pub selected_room_id: Option<String>,
    #[serde(default)]
    pub include_selected_messages: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixTriggerRoomUpdateResponse {
    pub queued: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MatrixRoomRemovedEvent {
    pub(crate) room_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MatrixSelectedRoomMessagesEvent {
    pub(crate) room_id: String,
    pub(crate) next_from: Option<String>,
    pub(crate) messages: Vec<MatrixChatMessage>,
}

pub(crate) type RoomSnapshot = HashMap<String, MatrixChatSummary>;
