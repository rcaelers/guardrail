use leptos::{either::Either, prelude::*};

use crate::{authenticated_user, components::logout::LogoutButton};

#[allow(non_snake_case)]
#[component]
pub fn Navbar(trigger: RwSignal<i64>) -> impl IntoView {
    let user = Resource::new(
        || {},
        move |_| async move { authenticated_user().await.unwrap_or(None) },
    );

    view! {
        <script>
            window.addEventListener("click", function (e) {
              document.querySelectorAll(".dropdown").forEach(function (dropdown) {
                if (!dropdown.contains(e.target)) {
                  dropdown.open = false;
                }
              });
            });
        </script>

        <div class="navbar bg-base-200 rounded-lg relative z-10 p-0">
            <div class="navbar-start">
                <div class="dropdown">
                    <div tabindex="0" role="button" class="btn btn-ghost lg:hidden">
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            class="h-5 w-5"
                            fill="none"
                            viewBox="0 0 24 24"
                            stroke="currentColor"
                        >
                            <path
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                stroke-width="2"
                                d="M4 6h16M4 12h8m-8 6h16"
                            ></path>
                        </svg>
                    </div>
                    <ul
                        tabindex="0"
                        class="menu menu-sm dropdown-content mt-3 z-1 p-1 shadow-sm bg-base-100 rounded-box w-52"
                    >
                        <li>
                            <a href="/crashes">Crashes</a>
                        </li>
                        <li>
                            <a href="/admin/symbols">Symbols</a>
                        </li>
                        <li>
                            <details>
                                <summary>Admin</summary>
                                <ul class="p-2">
                                    <li>
                                        <a href="/admin/products">Products</a>
                                    </li>
                                    <li>
                                        <a href="/admin/versions">Versions</a>
                                    </li>
                                    <li>
                                        <a href="/admin/users">Users</a>
                                    </li>
                                </ul>
                            </details>
                        </li>
                    </ul>
                </div>
                <a class="btn btn-ghost text-l">Guardrail</a>
            </div>
            <div class="navbar-center hidden lg:flex">
                <ul class="menu menu-horizontal px-1">
                    <li>
                        <a href="/crashes">Crashes</a>
                    </li>
                    <li>
                        <a href="/admin/symbols">Symbols</a>
                    </li>
                    <li>
                        <details class="dropdown">
                            <summary>Admin</summary>
                            <ul class="menu mt-0 dropdown-content z-1 bg-base-200 rounded-box w-52">
                                <li>
                                    <a href="/admin/products">Products</a>
                                </li>
                                <li>
                                    <a href="/admin/versions">Versions</a>
                                </li>
                                <li>
                                    <a href="/admin/users">Users</a>
                                </li>
                            </ul>
                        </details>
                    </li>
                </ul>
            </div>
            <div class="navbar-end">
                <Suspense fallback=|| {
                    view! { "Loading..." }
                }>
                <ul class="menu menu-horizontal px-1">
                    {move || Suspend::new(async move {
                        match user.await.clone() {
                            None =>
                                Either::Left(
                                    view! {
                                        <li>
                                            <a class="px-2" href="/auth/login">
                                                login
                                            </a>
                                        </li>
                                        <li>
                                            <a class="px-2" href="/auth/register">
                                                register
                                            </a>
                                        </li>
                                     },
                                ),

                            Some(user) => {
                                Either::Right(
                                    view! {
                                        <li>
                                            <a class="px-2" href="/auth/profile">
                                            {{ user.username }}
                                            </a>
                                        </li>
                                        <li>
                                            <LogoutButton trigger=trigger />
                                        </li>
                                    },
                                )
                            }
                        }
                    })}
                </ul>
                </Suspense>
            </div>
        </div>
    }
}
