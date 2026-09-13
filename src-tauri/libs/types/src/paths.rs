use std::path::{Path, PathBuf};

/// Resolved application directories. The Tauri binder constructs this from the
/// `AppHandle` during setup; domain crates take `&Paths` instead of an
/// `AppHandle` so they stay Tauri-free.
#[derive(Clone, Debug)]
pub struct Paths {
    data_dir: PathBuf,
    cache_dir: PathBuf,
}

impl Paths {
    pub fn new(data_dir: PathBuf, cache_dir: PathBuf) -> Self {
        Self {
            data_dir,
            cache_dir,
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn data_file(&self, name: &str) -> PathBuf {
        self.data_dir.join(name)
    }

    pub fn cache_file(&self, name: &str) -> PathBuf {
        self.cache_dir.join(name)
    }
}
