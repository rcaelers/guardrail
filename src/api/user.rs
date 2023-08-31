use async_trait::async_trait;

use super::base::BaseApi;

use crate::model::user::UserRepo;

pub struct UserApi;

#[async_trait]
impl BaseApi<UserRepo> for UserApi {}
