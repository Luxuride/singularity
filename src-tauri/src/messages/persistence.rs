use std::collections::{HashMap, VecDeque};

use tokio::sync::RwLock;

use super::types::MatrixChatMessage;
use super::types::MatrixGetChatMessagesResponse;

const MESSAGE_CACHE_MAX_ROOMS: usize = 64;

#[derive(Default)]
struct MessageCacheInner {
    by_room: HashMap<String, MatrixGetChatMessagesResponse>,
    lru_order: VecDeque<String>,
}

#[derive(Default)]
pub struct MessageCacheState {
    inner: RwLock<MessageCacheInner>,
}

impl MessageCacheState {
    pub async fn load_initial_room_messages(
        &self,
        room_id: &str,
        from: Option<&str>,
        limit: Option<u32>,
    ) -> Option<MatrixGetChatMessagesResponse> {
        if !is_cacheable_initial_request(from, limit) {
            return None;
        }

        let inner = self.inner.read().await;
        inner.by_room.get(room_id).cloned()
    }

    pub async fn store_initial_room_messages(&self, response: &MatrixGetChatMessagesResponse) {
        let room_id = response.room_id.clone();

        let mut inner = self.inner.write().await;
        inner.by_room.insert(room_id.clone(), response.clone());

        if let Some(index) = inner.lru_order.iter().position(|candidate| candidate == &room_id) {
            inner.lru_order.remove(index);
        }

        inner.lru_order.push_back(room_id);

        while inner.by_room.len() > MESSAGE_CACHE_MAX_ROOMS {
            let Some(oldest_room_id) = inner.lru_order.pop_front() else {
                break;
            };

            inner.by_room.remove(&oldest_room_id);
        }
    }

    pub async fn clear(&self) {
        let mut inner = self.inner.write().await;
        inner.by_room.clear();
        inner.lru_order.clear();
    }
}

pub fn is_cacheable_initial_request(from: Option<&str>, limit: Option<u32>) -> bool {
    if from.is_some() {
        return false;
    }

    let effective_limit = limit.unwrap_or(50).min(100);
    effective_limit == 50
}

pub(crate) fn sort_messages_by_timestamp(messages: &mut [MatrixChatMessage]) {
    messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
}
