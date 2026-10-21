//! Telegram bot skeleton. Real implementation: ticket 03.
use crate::model::TelegramSettings;
use crate::notify::{Notice, Notifier};
use std::sync::Arc;

pub trait Control: Send + Sync {
    fn submit(&self, project_hint: Option<&str>, prompt: &str) -> Result<String, String>;
    fn answer(&self, task_id: &str, text: &str) -> Result<(), String>;
    fn approve_outward(&self, task_id: &str, yes: bool) -> Result<(), String>;
    fn status_text(&self) -> String;
    fn digest_text(&self) -> String;
}

pub struct Telegram;
pub struct TelegramHandle;

impl Telegram {
    pub fn start(
        _settings: TelegramSettings,
        _token: Option<String>,
        _control: Arc<dyn Control>,
        _on_paired: Box<dyn Fn(i64) + Send>,
    ) -> Arc<TelegramHandle> {
        Arc::new(TelegramHandle)
    }
}

impl TelegramHandle {
    pub fn pairing_code(&self) -> String {
        String::new()
    }
    pub fn status(&self) -> String {
        "not implemented".to_string()
    }
    pub fn restart(&self, _settings: TelegramSettings, _token: Option<String>) {}
}

impl Notifier for TelegramHandle {
    fn send(&self, _n: &Notice) {}
}

pub fn load_token() -> Option<String> {
    None
}
pub fn save_token(_t: &str) -> Result<(), String> {
    Err("not implemented".to_string())
}
