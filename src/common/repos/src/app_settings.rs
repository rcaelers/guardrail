use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::error::{RepoError, handle_surreal_error};
use data::app_settings::{AppEmailSettings, AppSettings};

const MAIN_ID: &str = "main";

pub struct AppSettingsRepo {}

impl AppSettingsRepo {
    /// Fetch global app settings, creating the singleton record if absent.
    pub async fn get_or_create(db: &Surreal<Any>) -> Result<AppSettings, RepoError> {
        let mut result = db
            .query(
                "UPSERT type::record('app_settings', $id) SET \
                 created_at = created_at OR time::now(), \
                 updated_at = time::now() \
                 RETURN *, meta::id(id) AS id",
            )
            .bind(("id", MAIN_ID))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one::<AppSettings>(&mut result, 0)?
            .ok_or_else(|| RepoError::DatabaseError("upsert returned no row".into()))
    }

    /// Update the email section of global app settings.
    pub async fn upsert_email(
        db: &Surreal<Any>,
        email: AppEmailSettings,
    ) -> Result<AppSettings, RepoError> {
        let mut result = db
            .query(
                "UPSERT type::record('app_settings', $id) SET \
                 email.recovery_subject = $subject, \
                 email.recovery_html_template = $html, \
                 email.recovery_text_template = $text, \
                 created_at = created_at OR time::now(), \
                 updated_at = time::now() \
                 RETURN *, meta::id(id) AS id",
            )
            .bind(("id", MAIN_ID))
            .bind(("subject", email.recovery_subject))
            .bind(("html", email.recovery_html_template))
            .bind(("text", email.recovery_text_template))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one::<AppSettings>(&mut result, 0)?
            .ok_or_else(|| RepoError::DatabaseError("upsert returned no row".into()))
    }
}
