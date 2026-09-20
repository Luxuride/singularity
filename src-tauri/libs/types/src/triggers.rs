use tokio::sync::mpsc::UnboundedSender;

/// A request to refresh the room snapshot, optionally including the selected
/// room's messages.
#[derive(Clone)]
pub struct RoomRefreshTrigger {
    pub selected_room_id: Option<String>,
    pub include_selected_messages: bool,
}

/// Managed state that lets commands enqueue room-update work onto the
/// background room-update worker.
#[derive(Clone)]
pub struct RoomUpdateTriggerState {
    sender: UnboundedSender<RoomRefreshTrigger>,
}

impl RoomUpdateTriggerState {
    pub fn new(sender: UnboundedSender<RoomRefreshTrigger>) -> Self {
        Self { sender }
    }

    pub fn enqueue(&self, trigger: RoomRefreshTrigger) -> Result<(), String> {
        self.sender
            .send(trigger)
            .map_err(|_| String::from("Room update worker is not available"))
    }

    /// Enqueue a refresh for an optional selected room, optionally including
    /// that room's messages.
    pub fn enqueue_refresh(
        &self,
        selected_room_id: Option<String>,
        include_selected_messages: bool,
    ) -> Result<(), String> {
        self.enqueue(RoomRefreshTrigger {
            selected_room_id,
            include_selected_messages,
        })
    }
}
