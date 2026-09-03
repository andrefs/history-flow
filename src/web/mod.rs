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
    let assets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/js");
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
    if let Err(msg) = forbid_local_git(&config) {
        return (StatusCode::FORBIDDEN, msg).into_response();
    }
    match run_pipeline_async(config).await {
        Ok(spec) => Json(spec).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn probe_page(Query(form): Query<WebForm>) -> impl IntoResponse {
    if !form.is_probe() {
        return (StatusCode::BAD_REQUEST, "probe=1 required").into_response();
    }
    let config = form.into_config();
    if let Err(msg) = forbid_local_git(&config) {
        return (StatusCode::FORBIDDEN, msg).into_response();
    }
    match tokio::task::spawn_blocking(move || crate::import::probe(&config)).await {
        Ok(Ok(p)) => Json(serde_json::json!({
            "revision_count": p.revision_count,
            "oldest_revision": p.oldest_revision,
            "newest_revision": p.newest_revision,
            "source": p.source,
        }))
        .into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
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
    let spec = crate::visualize::build_spec_with_title(&grid, chart_title(config).as_deref());
    Ok(spec)
}

/// Derive a display title for the chart from the config's target.
/// Wikipedia -> page title; GitHub -> file name; git page -> file name.
fn chart_title(config: &crate::config::Config) -> Option<String> {
    if let Some(page) = &config.import.page {
        return Some(page.rsplit('/').next().unwrap_or(page).to_string());
    }
    let url = config.import.url.as_ref()?;
    if let Some(rest) = url.strip_prefix("https://en.wikipedia.org/wiki/") {
        return Some(
            rest.split(['#', '?'])
                .next()
                .unwrap_or(rest)
                .replace('_', " "),
        );
    }
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("github.com/"))
    {
        // take the file path after blob/<rev>/
        let parts: Vec<&str> = rest.split('/').collect();
        if let Some(i) = parts.iter().position(|&p| p == "blob") {
            let path = parts[i + 2..].join("/");
            if !path.is_empty() {
                return Some(path.rsplit('/').next().unwrap_or(&path).to_string());
            }
        }
    }
    Some(url.clone())
}

async fn run_pipeline_async(config: crate::config::Config) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(move || run_pipeline(&config))
        .await
        .map_err(|e| e.to_string())?
}

/// Reject git sources that would read from the server's local filesystem.
/// The web form only supplies a `target` (Wikipedia URL/title or GitHub blob
/// URL); a `repo` can never name a local path. This is defense-in-depth: if a
/// git repo is configured that points at an existing local path, refuse it.
fn forbid_local_git(config: &crate::config::Config) -> Result<(), String> {
    if let Some(url) = &config.import.url
        && (url.starts_with("https://github.com/") || url.starts_with("github.com/"))
    {
        return Ok(()); // remote GitHub blob URL — safe
    }
    if let Some(repo) = &config.import.repo {
        // `owner/repo` (no path separator that resolves locally) is remote.
        // A value naming an existing local directory is a local-repo read.
        if Path::new(repo).exists() {
            return Err("remote-only web server: local git repositories are not allowed".into());
        }
        if !repo.contains('/') {
            return Err("remote-only web server: local git repositories are not allowed".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn cfg_with_url(url: &str) -> Config {
        let mut c = Config::default();
        c.import.url = Some(url.to_string());
        c
    }

    #[test]
    fn title_from_wikipedia_url() {
        assert_eq!(
            chart_title(&cfg_with_url(
                "https://en.wikipedia.org/wiki/History_of_the_potato"
            ))
            .as_deref(),
            Some("History of the potato")
        );
    }

    #[test]
    fn title_from_github_blob_uses_file_name() {
        assert_eq!(
            chart_title(&cfg_with_url(
                "https://github.com/o/r/blob/main/src/deep/foo.rs"
            ))
            .as_deref(),
            Some("foo.rs")
        );
    }

    #[test]
    fn title_from_plain_target() {
        assert_eq!(
            chart_title(&cfg_with_url("History of the potato")).as_deref(),
            Some("History of the potato")
        );
    }

    #[test]
    fn title_from_git_page_is_basename() {
        let mut c = Config::default();
        c.import.page = Some("src/deep/foo.rs".to_string());
        assert_eq!(chart_title(&c).as_deref(), Some("foo.rs"));
    }
}
