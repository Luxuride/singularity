//! Matrix protocol helpers: serialized sync, timeline/event parsing,
//! homeserver URL normalization, and ID validation.

pub mod endpoints;
pub mod events_schema;
pub mod sync;
pub mod validation;

pub use validation::{parse_event_id, parse_room_id, parse_user_id};
