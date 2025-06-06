use leptos::prelude::*;

#[allow(non_snake_case)]
#[component]
pub fn PasskeyLogo() -> impl IntoView {
    view! {
        <span id="passkey" class="icon w-5 h-5 fill-white">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
                <g id="icon-passkey">
                    <circle id="icon-passkey-head" cx="10.5" cy="6" r="4.5"></circle>
                    <path
                        id="icon-passkey-key"
                        d="M22.5,10.5a3.5,3.5,0,1,0-5,3.15V19L19,20.5,21.5,18,20,16.5,21.5,15l-1.24-1.24A3.5,3.5,0,0,0,22.5,10.5Zm-3.5,0a1,1,0,1,1,1-1A1,1,0,0,1,19,10.5Z"
                    ></path>
                    <path
                        id="icon-passkey-body"
                        d="M14.44,12.52A6,6,0,0,0,12,12H9a6,6,0,0,0-6,6v2H16V14.49A5.16,5.16,0,0,1,14.44,12.52Z"
                    ></path>
                </g>
            </svg>
        </span>
    }
}
