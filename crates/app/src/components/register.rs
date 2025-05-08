use leptos::{html, prelude::*, task::spawn_local};
use web_sys::SubmitEvent;

use crate::{
    auth::{error::AuthError, passkeys::register_passkey},
    components::passkey_logo::PasskeyLogo,
};

#[component]
pub fn RegisterPage() -> impl IntoView {
    let input_element: NodeRef<html::Input> = NodeRef::new();

    let pending = RwSignal::new(false);
    let value = RwSignal::new(None);

    let result_message = move || {
        value.get().map(|v: Result<_, AuthError>| match v {
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
                    <span class="font-semibold">Registation successful</span>
                </div>
            }
            .into_any(),
            Err(e) => view! {
                <div id="info-label" class="alert alert-failure rounded-btn mt-4 p-3">
                    <span class="font-semibold">Registation failed</span>
                    {e.to_string()}
                </div>
            }
            .into_any(),
        })
    };

    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        pending.set(true);
        spawn_local(async move {
            let user_name = input_element
                .get_untracked()
                .expect("no <input> element")
                .value();
            value.set(Some(register_passkey(user_name).await));
            pending.set(false);
        });
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
                        // TODO: d="username"
                        name="username"
                        autocapitalize="none"
                        placeholder="user name"
                        node_ref=input_element
                    />
                    {result_message}
                    <Show when=move || value.read().is_none()>
                        <button id="register-button" class="btn btn-primary mt-4" type="submit" value="Submit">
                            <PasskeyLogo />
                            <span id="register-button-text" class="ml-2 text-base">
                                Register with Passkey
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
