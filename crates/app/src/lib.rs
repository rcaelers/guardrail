pub mod auth;
pub mod classes;
pub mod components;
pub mod data;
pub mod data_providers;

use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{
    StaticSegment,
    components::{FlatRoutes, Route, Router},
};

use auth::AuthenticatedUser;
use components::{
    // crash::CrashPage,
    // crashes::CrashesPage,
    login::LoginPage,
    navbar::Navbar,
    products::ProductsPage,
    profile::ProfilePage,
    register::RegisterPage,
    // symbols::SymbolsPage,
    // users::UsersPage,
    // versions::VersionsPage,
};

type UserResource = Resource<Option<AuthenticatedUser>>;

#[server(GetUser)]
pub async fn authenticated_user() -> Result<Option<AuthenticatedUser>, ServerFnError> {
    Ok(use_context::<Option<AuthenticatedUser>>().and_then(|u| u))
}

#[server(IsAdmin)]
pub async fn authenticated_user_is_admin() -> Result<bool, ServerFnError> {
    let user = authenticated_user()
        .await?
        .ok_or(ServerFnError::new("No authenticated user".to_string()))?;

    Ok(user.is_admin)
}

#[allow(non_snake_case)]
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let user_info_trigger = RwSignal::new(0);

    let user = Resource::new(user_info_trigger, move |_| async move {
        authenticated_user().await.unwrap_or(None)
    });

    view! {
        <head></head>

        <Stylesheet id="leptos" href="/pkg/site.css"/>
        <Stylesheet href="https://fonts.googleapis.com/css?family=Montserrat:300,400,500&display=swap"/>

        <Html {..}
              class="dark"
              lang="en"/>

        <Title text="GuardRail"/>
        <Meta charset="utf-8"/>
        <Meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0"/>
        <Meta name="description" content="Crashpad server"/>
        <Meta name="keywords" content="crashes, minidump"/>

        <Title text="Welcome to Leptos"/>

        <Router>
            <div class="container h-screen max-w-full flex flex-col">
                <header class="sticky top-0 z-50 p-1">
                    <Navbar trigger=user_info_trigger user=user/>
                </header>
                <main class="flex-1 overflow-hidden p-1 flex flex-col">
                    <FlatRoutes fallback=|| "Not found.">
                        <Route path=StaticSegment("") view=HomePage/>
                        <Route
                            path=StaticSegment("/auth/login")
                            view=move || view! { <LoginPage trigger=user_info_trigger/> }
                        />
                        <Route path=StaticSegment("/auth/register") view=RegisterPage/>
                        <Route path=StaticSegment("/auth/profile") view=ProfilePage/>
                        //<Route path=StaticSegment("/admin/users") view=UsersPage/>
                        <Route path=StaticSegment("/admin/products") view=ProductsPage/>
                        //<Route path=StaticSegment("/admin/versions") view=VersionsPage/>
                        //<Route path=StaticSegment("/admin/symbols") view=SymbolsPage/>
                        //<Route path=StaticSegment("/crashes") view=CrashesPage/>
                    </FlatRoutes>
                </main>
            </div>
        </Router>
    }
}

#[allow(non_snake_case)]
#[component]
fn HomePage() -> impl IntoView {
    view! { <h1>"Welcome to Guardrail!"</h1> }
}
