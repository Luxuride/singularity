use matrix_sdk::ruma::events::relation::InReplyTo;
use matrix_sdk::ruma::events::room::message::{Relation, RoomMessageEventContent};

use super::types::MatrixPickerCustomEmoji;
use crate::protocol::{parse_event_id, parse_room_id};
mod formatting;
use formatting::build_formatted_body_from_custom_emoji;

pub(crate) trait MessageSender {
    async fn send_room_message(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        body: &str,
        picker_custom_emoji: &[MatrixPickerCustomEmoji],
        in_reply_to_event_id_raw: Option<&str>,
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
        in_reply_to_event_id_raw: Option<&str>,
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

        let mut content = if let Some(formatted_body) = formatted_body.as_deref() {
            RoomMessageEventContent::text_html(trimmed_body, formatted_body)
        } else {
            RoomMessageEventContent::text_plain(trimmed_body)
        };

        if let Some(in_reply_to_event_id_raw) = in_reply_to_event_id_raw {
            let in_reply_to_event_id = parse_event_id(in_reply_to_event_id_raw)?;
            content.relates_to = Some(Relation::Reply {
                in_reply_to: InReplyTo::new(in_reply_to_event_id),
            });
        }

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
    in_reply_to_event_id_raw: Option<&str>,
) -> Result<String, String> {
    MatrixMessageSender
        .send_room_message(
            client,
            room_id_raw,
            body,
            picker_custom_emoji,
            in_reply_to_event_id_raw,
        )
        .await
}

pub(crate) fn build_formatted_body_from_custom_emoji_for_send(
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Option<String> {
    build_formatted_body_from_custom_emoji(body, picker_custom_emoji)
}
