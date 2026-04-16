use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;

use super::types::MatrixPickerCustomEmoji;
use crate::protocol::parse_room_id;
mod formatting;
use formatting::build_formatted_body_from_custom_emoji;

pub(crate) trait MessageSender {
    async fn send_room_message(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        body: &str,
        picker_custom_emoji: &[MatrixPickerCustomEmoji],
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
        picker_custom_emoji: &[MatrixPickerCustomEmoji],
    ) -> Result<String, String> {
        let trimmed_body = body.trim();
        if trimmed_body.is_empty() {
            return Err(String::from("Message cannot be empty"));
        }

        let room_id = parse_room_id(room_id_raw)?;

        let room = client
            .get_room(&room_id)
            .ok_or_else(|| String::from("Room is not available in current session"))?;

        let formatted_body =
            build_formatted_body_from_custom_emoji(trimmed_body, picker_custom_emoji);

        let content = if let Some(formatted_body) = formatted_body.as_deref() {
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
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Result<String, String> {
    MatrixMessageSender
        .send_room_message(client, room_id_raw, body, picker_custom_emoji)
        .await
}

pub(crate) fn build_formatted_body_from_custom_emoji_for_send(
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Option<String> {
    build_formatted_body_from_custom_emoji(body, picker_custom_emoji)
}
