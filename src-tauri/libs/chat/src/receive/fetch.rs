use log::{debug, warn};

use crate::helpers::build_messages_options;
use crate::media::MediaResolver;
use types::chat::MatrixGetChatMessagesResponse;

use super::parsing::{parse_message_chunk, ReactionAccumulator};
use protocol::parse_room_id;

/// One page of messages as returned by the server, plus the size of the RAW
/// event chunk it was parsed from.
///
/// The raw count matters because `messages` only contains events that survived
/// parsing: reaction events, state events and undecryptable chunks are all
/// dropped. Paging has to stop on "the server had nothing left", not on "we
/// could not use anything in this page", otherwise a page of pure reactions
/// silently truncates the timeline.
pub(super) struct FetchedPage {
    pub response: MatrixGetChatMessagesResponse,
    pub raw_event_count: usize,
    /// The reaction accumulator, handed back so the caller can carry it into the
    /// next page. The target of a reaction is normally still waiting in a page
    /// that has not been fetched yet.
    pub reactions_by_target: ReactionAccumulator,
}

pub(super) async fn fetch_room_messages_impl<M: MediaResolver>(
    media_resolver: &M,
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    from: Option<String>,
    limit: Option<u32>,
    reactions_by_target: &mut ReactionAccumulator,
) -> Result<FetchedPage, String> {
    let room_id = parse_room_id(room_id_raw)?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from(crate::helpers::ROOM_NOT_AVAILABLE))?;

    let response = room
        .messages(build_messages_options(from.clone(), limit))
        .await
        .map_err(|error| format!("Failed to read room messages: {error}"))?;

    let raw_event_count = response.chunk.len();
    // One accumulator per pagination run, not per page: it has to outlive the
    // page a reaction was parsed in, because its target is in an older page.
    let (mut messages, mut had_utd) =
        parse_message_chunk(media_resolver, client, response.chunk, reactions_by_target).await;
    let mut next_from = response.end;

    if had_utd && client.encryption().backups().are_enabled().await {
        if let Err(error) = client
            .encryption()
            .backups()
            .download_room_keys_for_room(&room_id)
            .await
        {
            warn!(
                "Failed to download backup keys for room {}: {}",
                room_id, error
            );
        } else {
            // The retry is best-effort: if it fails too, the partly
            // undecryptable first page is still the best we have, but the
            // placeholders must not be cached as if they were final.
            match room.messages(build_messages_options(from, limit)).await {
                Ok(retry_response) => {
                    let (retry_messages, retry_had_utd) = parse_message_chunk(
                        media_resolver,
                        client,
                        retry_response.chunk,
                        reactions_by_target,
                    )
                    .await;
                    messages = retry_messages;
                    had_utd = retry_had_utd;
                    next_from = retry_response.end;
                }
                Err(error) => {
                    warn!(
                        "Retrying room {} messages after downloading backup keys failed: {}",
                        room_id, error
                    );
                }
            }
        }
    }

    debug!(
        "Fetched {} chat messages from {} raw events (utd_present={})",
        messages.len(),
        raw_event_count,
        had_utd
    );

    Ok(FetchedPage {
        response: MatrixGetChatMessagesResponse {
            room_id: room_id.to_string(),
            next_from,
            messages,
        },
        raw_event_count,
        reactions_by_target: std::mem::take(reactions_by_target),
    })
}
