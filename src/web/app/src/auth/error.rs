use thiserror::Error;
use web_sys::wasm_bindgen::JsValue;

#[derive(Error, Clone, Debug)]
pub enum AuthError {
    #[error("Passkey failure: {0}")]
    PasskeyError(String),

    #[error("Logout failure: {0}")]
    LogoutError(String),
}

impl From<JsValue> for AuthError {
    fn from(value: JsValue) -> Self {
        Self::PasskeyError(format!("{value:?}"))
    }
}
