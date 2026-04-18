use askama::Template;

use crate::auth::AuthSession;

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate<'a> {
    pub title: &'a str,
    pub app_name: &'a str,
    pub auth: AuthSession,
    pub error: String,
    pub has_error: bool,
    pub login_url: String,
    pub oidc_enabled: bool,
    pub self_service_url: String,
}
