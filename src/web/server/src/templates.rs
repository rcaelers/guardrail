use askama::Template;

use crate::auth_user::AuthenticatedUser;

pub(crate) struct ProductGrant {
    pub name: String,
    pub role: String,
}

#[derive(Template)]
#[template(path = "email/invite.html")]
pub(crate) struct InviteEmailHtml<'a> {
    pub invite_url: &'a str,
    pub products: &'a [ProductGrant],
}

#[derive(Template)]
#[template(path = "email/invite.txt")]
pub(crate) struct InviteEmailText<'a> {
    pub invite_url: &'a str,
    pub products: &'a [ProductGrant],
}

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate<'a> {
    pub title: &'a str,
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
    pub auth: AuthenticatedUser,
    pub self_service_url: String,
    pub code: String,
    pub error: String,
    pub has_error: bool,
}
