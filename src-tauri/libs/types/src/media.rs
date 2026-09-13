/// How cached media is exposed to the frontend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaStorageMode {
    InMemory = 0,
    AssetStorage = 1,
}

impl MediaStorageMode {
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::AssetStorage,
            _ => Self::InMemory,
        }
    }
}
