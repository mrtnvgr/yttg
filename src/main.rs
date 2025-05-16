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
use strings::{CHOOSE_FORMAT, ERROR_BROKEN_SESSION, ERROR_SEND_A_LINK, FAILED_TO_DOWNLOAD, PLEASE_WAIT_FOR_THE_MEDIA};
use teloxide::dispatching::dialogue::GetChatId;
use teloxide::requests::Requester;
use teloxide::sugar::bot::BotMessagesExt;
use teloxide::types::InputMedia;
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

    let Some(message_text) = msg.text() else { return Ok(()) };
    let Ok(url) = Url::parse(message_text) else {
        bot.send_message(msg.chat.id, ERROR_SEND_A_LINK).await?;
        return Ok(());
    };

    if let Some(userdata) = db.lock().await.users.get_mut(user_id) {
        userdata.url = Some(url);
    };

    let bot_msg = bot.send_message(msg.chat.id, CHOOSE_FORMAT).reply_to(msg.id).await?;

    let buttons = Format::get_buttons(bot_msg.id);
    bot.edit_reply_markup(&bot_msg).reply_markup(buttons).await?;

    Ok(())
}

async fn receive_request_format(
    bot: Bot,
    q: CallbackQuery,
    ytdlp: Arc<Mutex<Downloader>>,
    db: Arc<Mutex<DB>>,
) -> ResponseResult<()> {
    bot.answer_callback_query(&q.id).await?;

    let Some(chat_id) = q.chat_id() else { return Ok(()) };

    let Some(request) = q.data.and_then(|x| serde_json::from_str::<DownloadRequest>(&x).ok()) else {
        if let Some(request_message) = q.message {
            let id = request_message.id();
            bot.edit_message_text(chat_id, id, ERROR_BROKEN_SESSION).await?;
        }
        return Ok(());
    };

    let Some(url) = db.lock().await.users.get_mut(&q.from.id).and_then(|x| x.url.take()) else {
        bot.edit_message_text(chat_id, request.id, ERROR_BROKEN_SESSION).await?;
        return Ok(());
    };

    let username = q.from.username.clone().unwrap_or_default();
    log::info!("@{username}: {} ({})", url, request.format);

    bot.edit_message_text(chat_id, request.id, PLEASE_WAIT_FOR_THE_MEDIA).await?;

    let Some(downloaded_media) = ytdlp.lock().await.download(url, request.format).await else {
        bot.edit_message_text(chat_id, request.id, FAILED_TO_DOWNLOAD).await?;
        return Ok(());
    };

    if let Some(userdata) = db.lock().await.users.get_mut(&q.from.id) {
        if matches!(request.format, Format::AudioOnly) {
            userdata.downloads.audios += 1;
        } else {
            userdata.downloads.videos += 1;
        }
    };
    db.lock().await.save();

    let media = InputMedia::from_format(request.format, &downloaded_media);

    if log_error(bot.edit_message_media(chat_id, request.id, media).await).is_err() {
        bot.edit_message_text(chat_id, request.id, FAILED_TO_DOWNLOAD).await?;
    };

    // The media path can be accessed, because we downloaded it there
    let _ = misc::log_error(std::fs::remove_file(&downloaded_media.path));

    Ok(())
}
