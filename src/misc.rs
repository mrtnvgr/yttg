use crate::config::Config;
use std::fmt::Debug;
use std::sync::Arc;
use teloxide::types::UserId;
use tokio::sync::Mutex;

pub async fn is_unknown_user(config: &Arc<Mutex<Config>>, user_id: UserId) -> bool {
    !config.lock().await.users.contains_key(&user_id)
}

pub fn log_error<T, E>(x: Result<T, E>) -> Result<T, E>
where
    E: Debug,
{
    if let Err(ref err) = x {
        log::error!("{err:?}");
    }

    x
}
