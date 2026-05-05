use std::sync::Arc;

use object_store::ObjectStore;
use repos::Repo;
use webauthn_rs::prelude::Webauthn;

use common::settings::Settings;
use crate::auth_cache::AuthCache;
use crate::provisioner::IdentityProvisioner;

#[derive(Clone)]
pub struct AppState {
    pub(crate) repo: Arc<Repo>,
    pub(crate) settings: Arc<Settings>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) webauthn: Arc<Webauthn>,
    pub(crate) provisioner: Option<Arc<dyn IdentityProvisioner>>,
    pub(crate) storage: Arc<dyn ObjectStore>,
    pub(crate) auth_cache: AuthCache,
}
