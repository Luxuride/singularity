use super::types::MatrixChatMessage;

pub(crate) fn sort_messages_by_timestamp(messages: &mut [MatrixChatMessage]) {
    messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
}
