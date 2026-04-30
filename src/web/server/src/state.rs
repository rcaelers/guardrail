use std::sync::Arc;

use repos::Repo;
use webauthn_rs::prelude::Webauthn;

use common::settings::Settings;
use crate::provisioner::IdentityProvisioner;

#[derive(Clone)]
pub struct AppState {
    pub(crate) repo: Repo,
    pub(crate) settings: Arc<Settings>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) webauthn: Arc<Webauthn>,
    pub(crate) provisioner: Option<Arc<dyn IdentityProvisioner>>,
}
