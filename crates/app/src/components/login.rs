use leptos::*;
use leptos_router::*;
use std::time::Duration;
use web_sys::SubmitEvent;

use crate::{auth::passkeys::login_passkey, components::passkey_logo::PasskeyLogo};

#[component]
pub fn LoginPage(trigger: RwSignal<i64>) -> impl IntoView {
    let input_element: NodeRef<html::Input> = create_node_ref();

    let login_passkey_action = create_action(|user_name: &String| {
        let user_name = user_name.to_owned();
        async move { login_passkey(user_name).await }
    });

    let _submitted = login_passkey_action.input();
    let pending = login_passkey_action.pending();
    let value = login_passkey_action.value();

    let result_message = move || {
        value.get().map(|v| match v {
            Ok(()) => view! {
                <div id="info-label" class="alert alert-success rounded-btn mt-4 p-3">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        class="stroke-current shrink-0 h-6 w-6"
                        fill="none"
                        viewBox="0 0 24 24"
                    >
                        <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                        ></path>
                    </svg>
                    <span class="font-semibold">Login successful</span>
                </div>
            }
            .into_view(),
            Err(e) => view! {
                <div id="info-label" class="alert alert-failure rounded-btn mt-4 p-3">
                    <span class="font-semibold">Login failed</span>
                    {e.to_string()}
                </div>
            }
            .into_view(),
        })
    };

    let perform_redirect = move || {
        trigger.update(|n| *n += 1);

        let navigate = use_navigate();
        navigate("/", NavigateOptions::default());
        let result = web_sys::window()
            .expect("Failed to get window")
            .location()
            .set_href("/crashes");
        if let Err(e) = result {
            logging::error!("failed to reload: {:?}", e);
        }
        let result = web_sys::window()
            .expect("Failed to get window")
            .location()
            .reload();
        if let Err(e) = result {
            logging::error!("failed to reload: {:?}", e);
        }
    };

    create_effect(move |_| {
        if value.get().is_some() {
            set_timeout(perform_redirect, Duration::from_secs(3));
        }
    });

    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let user_name = input_element.get().expect("<input> to exist").value();
        login_passkey_action.dispatch(user_name);
    };

    view! {
        <form on:submit=on_submit>
            <div class="absolute flex items-center inset-0 max-w-full">
                <div class="card flex flex-col max-w-lg w-full mx-auto">
                    <label class="font-semibold" for="username">
                        Username
                    </label>
                    <input
                        class="mt-1 input input-bordered"
                        type="text"
                        d="username"
                        name="username"
                        autocapitalize="none"
                        placeholder="user name"
                        node_ref=input_element
                    />
                    {result_message}
                    <Show when=move || value().is_none()>
                        <button id="login-button" class="btn btn-primary mt-4" type="submit">
                            <PasskeyLogo/>
                            <span id="login-button-text" class="ml-2 text-base">
                                login with Passkey
                            </span>
                            <span
                                id="loading"
                                class:hidden=move || !pending()
                                class="loading loading-dots loading-lg"
                            ></span>
                        </button>
                    </Show>
                </div>
            </div>
        </form>
    }
}
