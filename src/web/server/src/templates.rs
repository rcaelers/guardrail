use askama::Template;

use crate::auth_user::AuthenticatedUser;

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate<'a> {
    pub title: &'a str,
    pub app_name: &'a str,
    pub auth: AuthenticatedUser,
    pub error: String,
    pub has_error: bool,
    pub login_url: String,
    pub self_service_url: String,
}

#[derive(Template)]
#[template(path = "invite.html")]
pub struct InviteTemplate<'a> {
    pub title: &'a str,
    pub app_name: &'a str,
    pub auth: AuthenticatedUser,
    pub self_service_url: String,
    pub code: String,
    pub error: String,
    pub has_error: bool,
}
