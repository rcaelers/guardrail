use async_trait::async_trait;
use resend_rs::Resend;
use resend_rs::types::CreateEmailBaseOptions;

use crate::{Email, EmailError, EmailSender};

pub struct ResendEmailSender {
    client: Resend,
}

impl ResendEmailSender {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Resend::new(&api_key),
        }
    }
}

#[async_trait]
impl EmailSender for ResendEmailSender {
    async fn send(&self, email: Email) -> Result<(), EmailError> {
        let mut options =
            CreateEmailBaseOptions::new(&email.from, [email.to.as_str()], &email.subject)
                .with_html(&email.html);

        if let Some(text) = &email.text {
            options = options.with_text(text);
        }

        self.client
            .emails
            .send(options)
            .await
            .map_err(|e| EmailError::Provider(e.to_string()))?;

        Ok(())
    }
}
