mod emoji_html;

use types::chat::MatrixPickerCustomEmoji;

pub fn build_formatted_body_from_custom_emoji(
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Option<String> {
    emoji_html::build_formatted_body_from_custom_emoji(body, picker_custom_emoji)
}

pub fn build_display_formatted_body_from_custom_emoji(
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Option<String> {
    emoji_html::build_display_formatted_body_from_custom_emoji(body, picker_custom_emoji)
}
