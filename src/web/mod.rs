//! Web server — interactive single-input page and pipeline handlers.
use crate::web::config_form::WebForm;
use axum::{
    Router,
    extract::Query,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::get,
};
use std::net::SocketAddr;
use std::path::Path;
use tokio;
use tower_http::services::ServeDir;

mod config_form;

/// Start the History Flow web server on `addr`.
/// Serves the interactive single-input page at `/` and runs the pipeline on `/render`.
pub async fn run_server(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let assets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let app = Router::new()
        .route("/render", get(render_page))
        .route("/probe", get(probe_page))
        .nest_service("/static", ServeDir::new(&assets_dir))
        .fallback(index_page);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("serving on http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index_page() -> Html<String> {
    // Try reading the file first (for dev), then fallback to include_str!
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/web/static/index.html");
    let html = tokio::fs::read_to_string(&path)
        .await
        .unwrap_or_else(|_| include_str!("../web/static/index.html").to_string());
    Html(html)
}

async fn render_page(Query(form): Query<WebForm>) -> impl IntoResponse {
    let config = form.into_config();
    match run_pipeline(&config) {
        Ok(spec) => Json(spec).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn probe_page(Query(form): Query<WebForm>) -> impl IntoResponse {
    if !form.is_probe() {
        return (StatusCode::BAD_REQUEST, "probe=1 required").into_response();
    }
    let config = form.into_config();
    match crate::import::probe(&config) {
        Ok(p) => Json(serde_json::json!({
            "revision_count": p.revision_count,
            "oldest_revision": p.oldest_revision,
            "newest_revision": p.newest_revision,
            "source": p.source,
        }))
        .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn run_pipeline(config: &crate::config::Config) -> Result<serde_json::Value, String> {
    let revisions = crate::import::import_revisions(config).map_err(|e| e.to_string())?;
    let revisions = crate::import::select_revisions(
        revisions,
        config.import.mode,
        config.import.last,
        config.import.nth,
    );
    let contents: Vec<Vec<String>> = revisions
        .iter()
        .map(|r| r.content.lines().map(String::from).collect())
        .collect();
    let diffs: Vec<Vec<crate::attribution::DiffOp>> = contents
        .windows(2)
        .map(|w| crate::attribution::diff::diff_lines(&w[0], &w[1]))
        .collect();
    let grid =
        crate::attribution::run_attribution(&revisions, &diffs).map_err(|e| e.to_string())?;
    let spec = crate::visualize::build_spec(&grid);
    Ok(spec)
}
