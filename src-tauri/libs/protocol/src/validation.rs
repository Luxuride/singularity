use matrix_sdk::ruma::{OwnedEventId, OwnedRoomId, OwnedUserId};

/// Parse and validate a user ID string.
pub fn parse_user_id(user_id_raw: &str) -> Result<OwnedUserId, String> {
    OwnedUserId::try_from(user_id_raw).map_err(|_| format!("Invalid user ID: {user_id_raw}"))
}

/// Parse and validate a room ID string.
pub fn parse_room_id(room_id_raw: &str) -> Result<OwnedRoomId, String> {
    OwnedRoomId::try_from(room_id_raw).map_err(|_| format!("Invalid room ID: {room_id_raw}"))
}

/// Parse and validate an event ID string.
pub fn parse_event_id(event_id_raw: &str) -> Result<OwnedEventId, String> {
    OwnedEventId::try_from(event_id_raw).map_err(|_| format!("Invalid event ID: {event_id_raw}"))
}

/// Check if an error message indicates the room is not available in the current session.
#[allow(dead_code)]
pub fn is_room_unavailable_error(error: &str) -> bool {
    error.contains("Room is not available in current session")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_user_id() {
        let user_id = parse_user_id("@alice:example.org");
        assert!(user_id.is_ok());
        assert_eq!(user_id.unwrap().as_str(), "@alice:example.org");
    }

    #[test]
    fn parse_invalid_user_id() {
        let result = parse_user_id("not-a-valid-user-id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid user ID"));
    }

    #[test]
    fn parse_valid_room_id() {
        let room_id = parse_room_id("!room:example.org");
        assert!(room_id.is_ok());
        assert_eq!(room_id.unwrap().as_str(), "!room:example.org");
    }

    #[test]
    fn parse_invalid_room_id() {
        let result = parse_room_id("not-a-valid-room-id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid room ID"));
    }

    #[test]
    fn parse_valid_event_id() {
        let event_id = parse_event_id("$event:example.org");
        assert!(event_id.is_ok());
        assert_eq!(event_id.unwrap().as_str(), "$event:example.org");
    }

    #[test]
    fn parse_invalid_event_id() {
        let result = parse_event_id("not-a-valid-event-id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid event ID"));
    }

    #[test]
    fn detects_room_unavailable_error() {
        let error = "Room is not available in current session";
        assert!(is_room_unavailable_error(error));
    }

    #[test]
    fn detects_room_unavailable_error_with_yet() {
        let error = "Room is not available in current session yet";
        assert!(is_room_unavailable_error(error));
    }

    #[test]
    fn ignores_other_errors() {
        assert!(!is_room_unavailable_error("Some other error"));
        assert!(!is_room_unavailable_error("Room not found"));
    }
}
