use leptos::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    wasm_bindgen::{JsCast, JsValue},
    Request, RequestInit, RequestMode, Response,
};
use webauthn_rs_proto::{
    CreationChallengeResponse, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse,
};

use super::error::AuthError;

pub async fn login_passkey(username: String) -> Result<(), AuthError> {
    let req_challenge_resp = login_begin(username).await?;
    let pub_key_cred = login_update_challenge(req_challenge_resp).await?;
    login_complete(pub_key_cred).await?;
    Ok(())
}

async fn login_begin(username: String) -> Result<RequestChallengeResponse, AuthError> {
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::SameOrigin);

    let dest = format!("/auth/authenticate_start/{username}");
    let request = Request::new_with_str_and_init(&dest, &opts)?;

    request.headers().set("content-type", "application/json")?;

    let resp_value = JsFuture::from(window().fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;

    if resp.status() == 200 {
        let jsval = JsFuture::from(resp.json()?).await?;
        let req_challenge_resp = serde_wasm_bindgen::from_value(jsval)
            .map_err(|e| AuthError::PasskeyError(e.to_string()))?;
        Ok(req_challenge_resp)
    } else {
        let error = JsFuture::from(resp.text()?)
            .await?
            .as_string()
            .unwrap_or_else(|| "Unknown error".to_string());
        Err(AuthError::PasskeyError(error))
    }
}

async fn login_update_challenge(
    req_challenge_resp: RequestChallengeResponse,
) -> Result<PublicKeyCredential, AuthError> {
    let cred_req_options: web_sys::CredentialRequestOptions = req_challenge_resp.into();

    let promise = window()
        .navigator()
        .credentials()
        .get_with_options(&cred_req_options)?;
    let fut = JsFuture::from(promise);
    let jsval = fut.await?;

    let pub_key_cred = PublicKeyCredential::from(web_sys::PublicKeyCredential::from(jsval));
    Ok(pub_key_cred)
}

async fn login_complete(pub_key_cred: PublicKeyCredential) -> Result<(), AuthError> {
    let req_jsvalue = serde_json::to_string(&pub_key_cred)
        .map(|s| JsValue::from(&s))
        .map_err(|e| AuthError::PasskeyError(e.to_string()))?;

    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::SameOrigin);
    opts.set_body(&req_jsvalue);

    let request = Request::new_with_str_and_init("/auth/authenticate_finish", &opts)?;
    request.headers().set("content-type", "application/json")?;

    let resp_value = JsFuture::from(window().fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;

    if resp.status() == 200 {
        Ok(())
    } else {
        let error = JsFuture::from(resp.text()?)
            .await?
            .as_string()
            .unwrap_or_else(|| "Unknown error".to_string());
        Err(AuthError::PasskeyError(error))
    }
}

pub async fn register_passkey(username: String) -> Result<(), AuthError> {
    let creation_challenge_resp = register_begin(username).await?;
    let reg_pub_key_cred = register_update_challenge(creation_challenge_resp).await?;
    register_complete(reg_pub_key_cred).await?;
    Ok(())
}

async fn register_begin(username: String) -> Result<CreationChallengeResponse, AuthError> {
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::SameOrigin);

    let dest = format!("/auth/register_start/{username}");
    let request = Request::new_with_str_and_init(&dest, &opts)?;

    request.headers().set("content-type", "application/json")?;

    let resp_value = JsFuture::from(window().fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;

    if resp.status() == 200 {
        let jsval = JsFuture::from(resp.json()?).await?;
        let creation_challenge_resp = serde_wasm_bindgen::from_value(jsval)
            .map_err(|e| AuthError::PasskeyError(e.to_string()))?;
        Ok(creation_challenge_resp)
    } else {
        let error = JsFuture::from(resp.text()?)
            .await?
            .as_string()
            .unwrap_or_else(|| "Unknown error".to_string());
        Err(AuthError::PasskeyError(error))
    }
}

async fn register_update_challenge(
    creation_challenge_resp: CreationChallengeResponse,
) -> Result<RegisterPublicKeyCredential, AuthError> {
    let cred_creation_options: web_sys::CredentialCreationOptions = creation_challenge_resp.into();

    let promise = window()
        .navigator()
        .credentials()
        .create_with_options(&cred_creation_options)?;
    let fut = JsFuture::from(promise);

    let jsval = fut.await?;
    let reg_pub_key_cred =
        RegisterPublicKeyCredential::from(web_sys::PublicKeyCredential::from(jsval));
    Ok(reg_pub_key_cred)
}

async fn register_complete(reg_pub_key_cred: RegisterPublicKeyCredential) -> Result<(), AuthError> {
    let req_jsvalue = serde_json::to_string(&reg_pub_key_cred)
        .map(|s| JsValue::from(&s))
        .map_err(|e| AuthError::PasskeyError(e.to_string()))?;

    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::SameOrigin);
    opts.set_body(&req_jsvalue);

    let request = Request::new_with_str_and_init("/auth/register_finish", &opts)?;
    request.headers().set("content-type", "application/json")?;

    let resp_value = JsFuture::from(window().fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;

    if resp.status() == 200 {
        Ok(())
    } else {
        let error = JsFuture::from(resp.text()?)
            .await?
            .as_string()
            .unwrap_or_else(|| "Unknown error".to_string());
        Err(AuthError::PasskeyError(error))
    }
}
