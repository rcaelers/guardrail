use cfg_if::cfg_if;

pub mod auth;
pub mod classes;
pub mod components;
pub mod data_provider;
pub mod settings;

cfg_if! { if #[cfg(feature="ssr")] {
    pub mod entity;
    pub mod model;
}}

use leptos::*;
use leptos_meta::*;
use leptos_router::*;

use auth::AuthenticatedUser;
use components::{
    error_template::{AppError, ErrorTemplate},
    login::LoginPage,
    navbar::Navbar,
    products::ProductsPage,
    profile::ProfilePage,
    register::RegisterPage,
};

type UserResource = Resource<i64, Option<AuthenticatedUser>>;

#[server(GetUser)]
pub async fn authenticated_user() -> Result<Option<AuthenticatedUser>, ServerFnError> {
    Ok(use_context::<Option<AuthenticatedUser>>().and_then(|u| u))
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let user_info_trigger = create_rw_signal(0);

    let user = create_local_resource(user_info_trigger, move |_| async move {
        authenticated_user().await.unwrap_or(None)
    });

    view! {
        <Stylesheet id="leptos" href="/pkg/site.css"/>
        <Stylesheet href="https://fonts.googleapis.com/css?family=Montserrat:300,400,500&display=swap"/>

        <Html class="dark" lang="en"/>

        <Title text="GuardRail"/>
        <Meta charset="utf-8"/>
        <Meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0"/>
        <Meta name="description" content="Crashpad server"/>
        <Meta name="keywords" content="crashes, minidump"/>

        <Title text="Welcome to Leptos"/>

        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! { <ErrorTemplate outside_errors/> }.into_view()
        }>
            <div class="container min-h-screen max-w-full">
                <Navbar trigger=user_info_trigger user=user/>
                <Routes>
                    <Route path="" view=HomePage/>
                    <Route
                        path="/auth/login"
                        view=move || view! { <LoginPage trigger=user_info_trigger/> }
                    />
                    <Route path="/auth/register" view=RegisterPage/>
                    <Route path="/auth/profile" view=ProfilePage/>
                    <Route path="/admin/products" view=ProductsPage/>
                </Routes>
            </div>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    view! { <h1>"Welcome to Guardrail!"</h1> }
}
