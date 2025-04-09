use crate::config::{Config, UserData};
use crate::misc::is_unknown_user;
use std::env::var;
use std::string::ToString;
use std::sync::{Arc, LazyLock};
use teloxide::prelude::*;
use teloxide::sugar::request::RequestReplyExt;
use teloxide::utils::command::BotCommands;
use tokio::sync::Mutex;

pub static ADMIN_ID: LazyLock<UserId> = LazyLock::new(|| {
    let admin_id = var("ADMIN_ID").expect("Failed to get an admin id");
    UserId(admin_id.parse().expect("Invalid admin id"))
});

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Список доступных команд:")]
pub enum AdminCommands {
    #[command(description = "показать список команд")]
    Help,
    #[command(description = "({id} {alias}) дать пользователю доступ к боту", parse_with = "split")]
    Add { user_id: String, alias: String },
    #[command(description = "({id}) отобрать у пользователя доступ к боту")]
    Remove(String),
    #[command(description = "вывести список пользователей бота")]
    List,
}

impl AdminCommands {
    pub async fn print_help(bot: Bot, msg: Message) -> ResponseResult<()> {
        bot.send_message(msg.chat.id, Self::descriptions().to_string())
            .reply_to(msg.id)
            .await?;
        Ok(())
    }

    pub async fn add_user(
        bot: Bot,
        msg: Message,
        config: Arc<Mutex<Config>>,
        user_id: String,
        alias: String,
    ) -> ResponseResult<()> {
        if let Some(user_id) = get_user_id_from_string(&bot, &msg, user_id).await? {
            if is_unknown_user(&config, user_id).await {
                config.lock().await.users.insert(user_id, UserData::aliased(alias));
                config.lock().await.save();

                let text = "Пользователь добавлен :)";
                bot.send_message(msg.chat.id, text).reply_to(msg.id).await?;
            };
        };

        Ok(())
    }

    pub async fn remove_user(bot: Bot, msg: Message, config: Arc<Mutex<Config>>, user_id: String) -> ResponseResult<()> {
        if let Some(user_id) = get_user_id_from_string(&bot, &msg, user_id).await? {
            if !is_unknown_user(&config, user_id).await {
                config.lock().await.users.remove(&user_id);
                config.lock().await.save();

                let text = "Пользователь удалён :(";
                bot.send_message(msg.chat.id, text).reply_to(msg.id).await?;
            };
        };

        Ok(())
    }

    pub async fn list_users(bot: Bot, msg: Message, config: Arc<Mutex<Config>>) -> ResponseResult<()> {
        let format_user = |(id, data): (&UserId, &UserData)| format!("{} ({id}): [{}]", data.alias, data.downloads);

        let mut users: Vec<String> = config.lock().await.users.iter().map(format_user).collect();

        let heading = if users.is_empty() {
            "Пользователей нету! :0"
        } else {
            "Пользователи:"
        };

        users.insert(0, heading.to_owned());

        bot.send_message(msg.chat.id, users.join("\n")).reply_to(msg.id).await?;

        Ok(())
    }
}

async fn get_user_id_from_string(bot: &Bot, msg: &Message, user_id: String) -> ResponseResult<Option<UserId>> {
    let Ok(user_id) = user_id.parse::<u64>() else {
        bot.send_message(
            msg.chat.id,
            "Неверный ID пользователя.\nВоспользуйся ботом @UserBotInfoBot :)",
        )
        .reply_to(msg.id)
        .await?;

        return Ok(None);
    };

    Ok(Some(UserId(user_id)))
}

pub fn is_admin_texting(msg: Message) -> bool {
    let admin_id = &ADMIN_ID;
    msg.from.is_some_and(|x| x.id.0 == admin_id.0)
}
