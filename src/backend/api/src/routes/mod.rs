mod crashes;
mod health;
mod symbols;

use axum::{
    Router,
    routing::{get, post},
};

use crate::state::AppState;
use symbols::SymbolsApi;

pub async fn routes(_app_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/symbols/{token}/upload", post(SymbolsApi::upload))
        .route(
            "/symbols/{token}/{module_id}/{build_id}",
            axum::routing::delete(SymbolsApi::delete),
        )
        // Token-authenticated crash access for analysis integrations. Scoped to
        // the token's product; reports redacted unless crash-read-full.
        .route("/crashes", get(crashes::list_groups))
        .route("/crashes/{group_id}", get(crashes::get_group))
        .route("/crashes/{group_id}/notes", post(crashes::add_group_note))
        .route("/crashes/{group_id}/status", post(crashes::set_group_status))
        .route("/crashes/{group_id}/merge", post(crashes::merge_groups))
        .route("/crashes/by-crash/{crash_id}", get(crashes::get_crash))
        .route("/live", get(health::live))
        .route("/ready", get(health::ready))
}
