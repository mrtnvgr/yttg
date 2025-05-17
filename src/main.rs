mod admin;
mod db;
mod misc;
mod strings;
mod ytdlp;

use admin::{AdminCommands, is_admin_texting};
use db::DB;
use dptree::case;
use misc::log_error;
use std::env::var;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use strings::{Language, Response};
use teloxide::dispatching::dialogue::GetChatId;
use teloxide::requests::Requester;
use teloxide::sugar::bot::BotMessagesExt;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, InputMedia};
use teloxide::{prelude::*, sugar::request::RequestReplyExt};
use tokio::sync::Mutex;
use url::Url;
use ytdlp::{DownloadRequest, Downloader, Format, InputMediaFromFile};

// TODO: multiple download requests for one user?

pub static WORKDIR: LazyLock<PathBuf> = LazyLock::new(|| var("WORKDIR").expect("Failed to get a working directory").into());

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting...");

    let bot = Bot::from_env();

    log::info!("Preparing yt-dlp (can take a while)...");
    let ytdlp = Arc::new(Mutex::new(Downloader::new().await));
    log::info!("...done");

    let db = Arc::new(Mutex::new(DB::load_or_init()));

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<AdminCommands>()
                .filter(is_admin_texting)
                .branch(case![AdminCommands::Help].endpoint(AdminCommands::print_help))
                .branch(case![AdminCommands::Add { user_id, alias }].endpoint(AdminCommands::add_user))
                .branch(case![AdminCommands::Remove(user_id)].endpoint(AdminCommands::remove_user))
                .branch(case![AdminCommands::List].endpoint(AdminCommands::list_users)),
        )
        .branch(Update::filter_message().endpoint(receive_download_request))
        .branch(Update::filter_callback_query().endpoint(receive_request_format));

    tokio::spawn(ytdlp::update_ytdlp(ytdlp.clone()));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![ytdlp, db])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn receive_download_request(bot: Bot, msg: Message, db: Arc<Mutex<DB>>) -> ResponseResult<()> {
    let Some(ref user_id) = msg.from.as_ref().map(|x| x.id) else {
        return Ok(());
    };

    if misc::is_unknown_user(&db, *user_id).await {
        return Ok(());
    };

    let language: Language = msg.from.clone().into();

    let Some(message_text) = msg.text() else { return Ok(()) };
    let Ok(url) = Url::parse(message_text) else {
        let message = Response::ErrorSendALink.as_str(language);
        bot.send_message(msg.chat.id, message).await?;
        return Ok(());
    };

    if let Some(userdata) = db.lock().await.users.get_mut(user_id) {
        userdata.url = Some(url);
    }

    let message = Response::ChooseFormat.as_str(language);
    let bot_msg = bot.send_message(msg.chat.id, message).reply_to(msg.id).await?;

    let buttons = Format::buttons().into_iter().map(|x| {
        x.into_iter()
            .map(|format| DownloadRequest { format, language })
            .map(InlineKeyboardButton::from)
    });

    let buttons = InlineKeyboardMarkup::new(buttons);
    bot.edit_reply_markup(&bot_msg).reply_markup(buttons).await?;

    Ok(())
}

async fn receive_request_format(
    bot: Bot,
    q: CallbackQuery,
    ytdlp: Arc<Mutex<Downloader>>,
    db: Arc<Mutex<DB>>,
) -> ResponseResult<()> {
    if misc::is_unknown_user(&db, q.from.id).await {
        return Ok(());
    };

    bot.answer_callback_query(&q.id).await?;

    let Some(chat_id) = q.chat_id() else { return Ok(()) };
    let Some(message_id) = q.message.map(|x| x.id()) else {
        return Ok(());
    };

    let language: Language = q.from.language_code.into();

    let Some(request) = q.data.and_then(|x| serde_json::from_str::<DownloadRequest>(&x).ok()) else {
        let message = Response::ErrorBrokenSession.as_str(language);
        bot.edit_message_text(chat_id, message_id, message).await?;
        return Ok(());
    };

    let Some(url) = db.lock().await.users.get_mut(&q.from.id).and_then(|x| x.url.take()) else {
        let message = Response::ErrorBrokenSession.as_str(language);
        bot.edit_message_text(chat_id, message_id, message).await?;
        return Ok(());
    };

    let username = q.from.username.clone().unwrap_or_default();
    let format = request.format.as_str(language);
    log::info!("@{username}: {url} ({format})");

    let message = Response::PleaseWaitForTheMedia.as_str(language);
    bot.edit_message_text(chat_id, message_id, message).await?;

    let Some(downloaded_media) = ytdlp.lock().await.download(url, request.format).await else {
        let message = Response::ErrorFailedToDownload.as_str(language);
        bot.edit_message_text(chat_id, message_id, message).await?;
        return Ok(());
    };

    if let Some(userdata) = db.lock().await.users.get_mut(&q.from.id) {
        if matches!(request.format, Format::AudioOnly) {
            userdata.downloads.audios += 1;
        } else {
            userdata.downloads.videos += 1;
        }
    }
    db.lock().await.save();

    let media = InputMedia::from_format(request.format, &downloaded_media);

    if log_error(bot.edit_message_media(chat_id, message_id, media).await).is_err() {
        let message = Response::ErrorFailedToDownload.as_str(language);
        bot.edit_message_text(chat_id, message_id, message).await?;
    };

    // The media path can be accessed, because we downloaded it there
    let _ = misc::log_error(std::fs::remove_file(&downloaded_media.path));

    Ok(())
}
