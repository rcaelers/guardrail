use app::*;
use leptos::prelude::*;
use tracing::Level;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use tracing_subscriber_wasm::MakeConsoleWriter;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub fn hydrate() {
    tracing::subscriber::set_global_default(
        fmt::Subscriber::builder()
            .with_env_filter("app=debug,tower_http=debug,leptos=debug,leptos_axum=debug")
            .with_max_level(Level::DEBUG)
            .without_time()
            .with_ansi(false)
            .finish()
            .with(
                fmt::Layer::default()
                    .with_writer(MakeConsoleWriter::default())
                    .with_ansi(false)
                    .without_time(),
            ),
    )
    .expect("Unable to configure tracing");
    console_error_panic_hook::set_once();

    leptos::mount::hydrate_body(App);
}
