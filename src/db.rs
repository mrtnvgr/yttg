use crate::WORKDIR;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::LazyLock;
use teloxide::types::UserId;
use url::Url;

static DB_PATH: LazyLock<PathBuf> = LazyLock::new(|| WORKDIR.join("db.json"));

#[derive(Serialize, Deserialize, Default)]
pub struct DB {
    pub users: HashMap<UserId, UserData>,
}

impl DB {
    pub fn load_or_init() -> Self {
        #[allow(clippy::option_if_let_else)]
        if let Ok(path) = File::open(DB_PATH.as_path()) {
            let reader = BufReader::new(path);
            serde_json::from_reader(reader).expect("Failed to deserialize db")
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        if let Err(err) = File::create(DB_PATH.as_path()).map(|x| serde_json::to_writer(x, self)) {
            log::error!("DB saving error: {err}");
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct UserData {
    pub alias: String,
    pub downloads: Downloads,

    // Temporary state
    #[serde(skip)]
    pub url: Option<Url>,
}

impl UserData {
    pub fn aliased(alias: String) -> Self {
        Self {
            alias,
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Downloads {
    pub videos: usize,
    pub audios: usize,
}

impl Display for Downloads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(📺: {}) (🔊: {})", self.videos, self.audios)
    }
}
