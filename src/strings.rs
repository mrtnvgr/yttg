use serde::{Deserialize, Serialize};
use teloxide::types::User;

use crate::ytdlp::Format;

#[derive(Serialize, Deserialize, Default, Clone, Copy)]
pub enum Language {
    #[default]
    English,
    Russian,
}

// From IETF language tag
impl From<Option<String>> for Language {
    fn from(value: Option<String>) -> Self {
        match value {
            Some(value) if value == "en" => Self::English,
            Some(value) if value == "ru" => Self::Russian,
            _ => Self::default(),
        }
    }
}

impl From<Option<User>> for Language {
    fn from(value: Option<User>) -> Self {
        match value {
            Some(value) => value.language_code.into(),
            _ => Self::default(),
        }
    }
}

pub enum Response {
    ErrorSendALink,
    ErrorBrokenSession,
    ChooseFormat,
    PleaseWaitForTheMedia,
    ErrorFailedToDownload,
}

impl Response {
    pub const fn as_str(&self, language: Language) -> &str {
        match (self, language) {
            (Self::ErrorSendALink, Language::English) => "Hi! I can download from YouTube, send a link to a video",
            (Self::ErrorSendALink, Language::Russian) => "Привет! Я умею скачивать с ютуба, отправь ссылку на видео",

            (Self::ErrorBrokenSession, Language::English) => "Please repeat your request, it's been too long I've forgotten X_X",
            (Self::ErrorBrokenSession, Language::Russian) => "Пожалуйста повторите свой запрос, я забыл какая ссылка была X_X",

            (Self::ChooseFormat, Language::English) => "Choose what you want to download:",
            (Self::ChooseFormat, Language::Russian) => "Выбери формат:",

            (Self::PleaseWaitForTheMedia, Language::English) => "The downloaded media will be attached to this message ⌛",
            (Self::PleaseWaitForTheMedia, Language::Russian) => "Как скачаю, прикреплю в это сообщение ⌛",

            (Self::ErrorFailedToDownload, Language::English) => "Failed to download :(",
            (Self::ErrorFailedToDownload, Language::Russian) => "Не удалось скачать :(",
        }
    }
}

impl Format {
    pub const fn as_str(&self, language: Language) -> &str {
        match (self, language) {
            (Self::FullHD, Language::English) => "📺 High quality",
            (Self::FullHD, Language::Russian) => "📺 Высокое качество",

            (Self::HD, Language::English) => "📺 Normal quality",
            (Self::HD, Language::Russian) => "📺 Среднее качество",

            (Self::LowRes, Language::English) => "📺 Low quality",
            (Self::LowRes, Language::Russian) => "📺 Низкое качество",

            (Self::AudioOnly, Language::English) => "🔊 Audio-only",
            (Self::AudioOnly, Language::Russian) => "🔊 Только звук",
        }
    }
}
