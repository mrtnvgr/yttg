use crate::WORKDIR;
use crate::misc::log_error;
use crate::strings::{AUDIO_FORMAT, FULLHD_FORMAT, HD_FORMAT, LOWRES_FORMAT};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use teloxide::types::MessageId;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use teloxide::types::{InputFile, InputMedia, InputMediaAudio, InputMediaVideo};
use tokio::sync::Mutex;
use tokio::time::sleep;
use url::Url;
use yt_dlp::Youtube;
use yt_dlp::model::{AudioCodecPreference, VideoCodecPreference};
use yt_dlp::model::{AudioQuality, VideoQuality};

pub struct Downloader {
    inner: Youtube,
}

impl Downloader {
    pub async fn new() -> Self {
        let workdir = WORKDIR.as_path();
        std::fs::create_dir_all(workdir).expect("Failed to create a directory for yt-dlp");
        let mut inner = Youtube::with_new_binaries(workdir, workdir)
            .await
            .expect("Failed to init yt-dlp");
        inner.with_timeout(Duration::from_secs(10 * 60));

        let _ = inner.update_downloader().await;

        Self { inner }
    }

    pub async fn download(&self, url: Url, format: Format) -> Option<DownloadedMedia> {
        let video = self.inner.fetch_video_infos(url.to_string()).await.ok()?;

        let is_audio_only = matches!(format, Format::AudioOnly);

        let extension = if is_audio_only { "mp3" } else { "mp4" };
        let output = format!("{}.{extension}", video.id);

        let ac = AudioCodecPreference::MP3;

        let path = if is_audio_only {
            let audio_format = video.select_audio_format(format.into(), ac)?;
            log_error(self.inner.download_format(audio_format, output).await).ok()?
        } else {
            let vq: VideoQuality = format.into();
            let aq: AudioQuality = format.into();

            let vc = VideoCodecPreference::AVC1;

            let task = self.inner.download_video_with_quality(url, output, vq, vc, aq, ac).await;
            log_error(task).ok()?
        };

        let title = video.title;
        Some(DownloadedMedia { title, path })
    }
}

pub struct DownloadedMedia {
    pub title: String,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize)]
pub struct DownloadRequest {
    pub id: MessageId,
    pub format: Format,
}

impl From<DownloadRequest> for InlineKeyboardButton {
    fn from(value: DownloadRequest) -> Self {
        let format = value.format.to_string();
        let callback = serde_json::to_string(&value).expect("Failed to serialize Download");
        Self::callback(format, callback)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Format {
    FullHD,    // 1080p
    HD,        // 720p
    LowRes,    // 480p
    AudioOnly, // audio only
}

impl From<Format> for VideoQuality {
    fn from(val: Format) -> Self {
        let height = match val {
            Format::FullHD => 1080,
            Format::HD | Format::AudioOnly => 720,
            Format::LowRes => 480,
        };

        Self::CustomHeight(height)
    }
}

impl From<Format> for AudioQuality {
    fn from(_: Format) -> Self {
        // always pick the best audio
        Self::Best
    }
}

impl Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::FullHD => FULLHD_FORMAT,
            Self::HD => HD_FORMAT,
            Self::LowRes => LOWRES_FORMAT,
            Self::AudioOnly => AUDIO_FORMAT,
        };

        write!(f, "{text}")
    }
}

impl Format {
    pub fn get_buttons(request_id: MessageId) -> InlineKeyboardMarkup {
        let video_row = vec![Self::FullHD, Self::HD, Self::LowRes];
        let audio_row = vec![Self::AudioOnly];

        let buttons = vec![video_row, audio_row].into_iter().map(|x| {
            x.into_iter()
                .map(|format| DownloadRequest { id: request_id, format })
                .map(InlineKeyboardButton::from)
        });

        InlineKeyboardMarkup::new(buttons)
    }
}

pub trait InputMediaFromFile {
    fn from_format(format: Format, downloaded_media: &DownloadedMedia) -> InputMedia;
}

impl InputMediaFromFile for InputMedia {
    fn from_format(format: Format, downloaded_media: &DownloadedMedia) -> Self {
        let file: InputFile = InputFile::file(&downloaded_media.path);

        let title = &downloaded_media.title;
        match format {
            Format::FullHD | Format::HD | Format::LowRes => Self::Video(InputMediaVideo::new(file)),
            Format::AudioOnly => Self::Audio(InputMediaAudio::new(file).title(title)),
        }
    }
}

pub async fn update_ytdlp(ytdlp: Arc<Mutex<Downloader>>) {
    loop {
        sleep(Duration::from_secs(60 * 60)).await;
        let _ = log_error(ytdlp.lock().await.inner.update_downloader().await);
    }
}
