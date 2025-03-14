use leptos::prelude::*;

#[cfg(feature = "ssr")]
use tracing::error;

use web_sys::MouseEvent;

#[cfg(feature = "ssr")]
use crate::auth;

#[allow(non_snake_case)]
#[component]
pub fn LogoutButton(trigger: RwSignal<i64>) -> impl IntoView {
    let logout_action = ServerAction::<Logout>::new();

    let on_click = move |_ev: MouseEvent| {
        logout_action.dispatch(Logout {});
    };

    Effect::new(move |_| {
        if logout_action.value().get().is_some() {
            trigger.update(|n| *n += 1);
        }
    });

    view! {
        <div class="pt-2">
            <button class="button" on:click=on_click>
                "Log Out"
            </button>
        </div>
    }
}

#[server(Logout)]
pub async fn logout() -> Result<(), ServerFnError> {
    let mut auth_session = use_context::<auth::AuthSession>()
        .ok_or_else(|| ServerFnError::new("Failed to get auth session"))?;

    auth_session.logout().await.map_err(|e| {
        error!("Failed to log out: {:?}", e);
        ServerFnError::new("Failed to log out")
    })?;

    leptos_axum::redirect("/");
    Ok(())
}
