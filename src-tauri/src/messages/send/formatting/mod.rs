mod emoji_html;

use super::super::types::MatrixPickerCustomEmoji;

pub(super) fn build_formatted_body_from_custom_emoji(
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Option<String> {
    emoji_html::build_formatted_body_from_custom_emoji(body, picker_custom_emoji)
}
