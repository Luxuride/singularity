use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;

pub(crate) trait MessageSender {
    async fn send_room_message(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        body: &str,
        formatted_body: Option<&str>,
    ) -> Result<String, String>;
}

#[derive(Default, Clone, Copy)]
pub(crate) struct MatrixMessageSender;

impl MessageSender for MatrixMessageSender {
    async fn send_room_message(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        body: &str,
        formatted_body: Option<&str>,
    ) -> Result<String, String> {
        let trimmed_body = body.trim();
        if trimmed_body.is_empty() {
            return Err(String::from("Message cannot be empty"));
        }

        let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_raw)
            .map_err(|_| String::from("roomId is invalid"))?;

        let room = client
            .get_room(&room_id)
            .ok_or_else(|| String::from("Room is not available in current session"))?;

        let content = if let Some(formatted_body) = formatted_body
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            RoomMessageEventContent::text_html(trimmed_body, formatted_body)
        } else {
            RoomMessageEventContent::text_plain(trimmed_body)
        };

        let response = room
            .send(content)
            .await
            .map_err(|error| format!("Failed to send room message: {error}"))?;

        Ok(response.event_id.to_string())
    }
}

pub(crate) async fn send_room_message_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    body: &str,
    formatted_body: Option<&str>,
) -> Result<String, String> {
    MatrixMessageSender
        .send_room_message(client, room_id_raw, body, formatted_body)
        .await
}
