use askama::Template;

use crate::auth::AuthSession;

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate<'a> {
    pub title: &'a str,
    pub app_name: &'a str,
    pub auth: AuthSession,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate<'a> {
    pub title: &'a str,
    pub app_name: &'a str,
    pub auth: AuthSession,
    pub next: String,
}
