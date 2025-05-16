use crate::db::DB;
use std::fmt::Debug;
use std::sync::Arc;
use teloxide::types::UserId;
use tokio::sync::Mutex;

pub async fn is_unknown_user(db: &Arc<Mutex<DB>>, user_id: UserId) -> bool {
    !db.lock().await.users.contains_key(&user_id)
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
