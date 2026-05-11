mod log_sender;
mod resend;

pub use log_sender::LogEmailSender;
pub use resend::ResendEmailSender;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Email {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub html: String,
    pub text: Option<String>,
}

#[derive(Debug, Error)]
pub enum EmailError {
    #[error("provider error: {0}")]
    Provider(String),
}

#[async_trait]
pub trait EmailSender: Send + Sync {
    async fn send(&self, email: Email) -> Result<(), EmailError>;
}
