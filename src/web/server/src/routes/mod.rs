pub(crate) mod auth;
pub(crate) mod db_api;
pub(crate) mod home;
pub(crate) mod impersonation;
pub(crate) mod invite;

use crate::error::{AppError, AppResult};

pub(crate) fn render(template: impl askama::Template) -> AppResult<axum::response::Html<String>> {
    template
        .render()
        .map(axum::response::Html)
        .map_err(AppError::internal)
}
