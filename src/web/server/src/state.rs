use std::sync::Arc;

use email::EmailSender;
use object_store::ObjectStore;
use repos::Repo;

use crate::auth_cache::AuthCache;
use crate::provisioner::IdentityProvisioner;
use crate::settings::Settings;

#[derive(Clone)]
pub struct AppState {
    pub(crate) repo: Arc<Repo>,
    pub(crate) settings: Arc<Settings>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) provisioner: Option<Arc<dyn IdentityProvisioner>>,
    pub(crate) email_sender: Option<Arc<dyn EmailSender>>,
    pub(crate) storage: Arc<dyn ObjectStore>,
    pub(crate) auth_cache: AuthCache,
}
