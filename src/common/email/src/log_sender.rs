use async_trait::async_trait;

use crate::{Email, EmailError, EmailSender};

/// Logs emails instead of sending them. Used when no provider API key is configured.
pub struct LogEmailSender;

#[async_trait]
impl EmailSender for LogEmailSender {
    async fn send(&self, email: Email) -> Result<(), EmailError> {
        tracing::info!(
            to = %email.to,
            subject = %email.subject,
            body = %email.text.as_deref().unwrap_or(&email.html),
            "email (log-only, no provider configured)",
        );
        Ok(())
    }
}
