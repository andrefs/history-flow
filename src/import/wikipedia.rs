//! Wikipedia Action API adapter — metadata-only probe.

use super::{ImportError, SourceProbe};

use crate::config::Source;
use crate::import::Revision;
use chrono::{DateTime, Utc};
use serde::Deserialize;

const API: &str = "https://en.wikipedia.org/w/api.php";

/// Count revisions and capture the oldest/newest timestamps for a page.
/// Fetches `ids|timestamp` metadata only (never revision content), paging
/// via the API's `rvcontinue` cursor until pages are exhausted.
pub fn probe(title: &str) -> Result<SourceProbe, ImportError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("history-flow/0.1 (https://github.com/andrefs/history-flow; Rust)")
        .build()
        .map_err(|e| ImportError::Network(e.to_string()))?;
    let mut count: u64 = 0;
    let mut newest: Option<DateTime<Utc>> = None;
    let mut oldest: Option<DateTime<Utc>> = None;
    let mut rvcontinue: Option<String> = None;

    loop {
        let mut params: Vec<(&str, &str)> = vec![
            ("action", "query"),
            ("format", "json"),
            ("formatversion", "2"),
            ("prop", "revisions"),
            ("rvprop", "ids|timestamp"),
            ("rvlimit", "max"),
            ("titles", title),
        ];
        if let Some(cursor) = &rvcontinue {
            params.push(("rvcontinue", cursor));
        }

        let resp: ApiResponse = client
            .get(API)
            .query(&params)
            .send()
            .map_err(|e| ImportError::Network(e.to_string()))?
            .error_for_status()
            .map_err(|e| ImportError::Network(e.to_string()))?
            .json()
            .map_err(|e| ImportError::Network(e.to_string()))?;

        let page = resp
            .query
            .pages
            .into_iter()
            .next()
            .ok_or_else(|| ImportError::Api("empty pages array".into()))?;
        if page.missing == Some(true) {
            return Err(ImportError::MissingPage(page.title));
        }

        let revisions = page.revisions.unwrap_or_default();
        count += revisions.len() as u64;
        for r in &revisions {
            newest.get_or_insert(r.timestamp);
            oldest = Some(r.timestamp);
        }

        match resp.continue_.and_then(|c| c.rvcontinue) {
            Some(cursor) => rvcontinue = Some(cursor),
            None => break,
        }
    }

    Ok(SourceProbe {
        revision_count: count,
        oldest_revision: oldest,
        newest_revision: newest,
        source: Source::Wikipedia,
    })
}

/// Fetch all revisions with full content for a page.
/// Returns revisions in chronological order (oldest first).
pub fn fetch_revisions(title: &str) -> Result<Vec<Revision>, ImportError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("history-flow/0.1 (https://github.com/andrefs/history-flow; Rust)")
        .build()
        .map_err(|e| ImportError::Network(e.to_string()))?;

    let mut all_revisions = Vec::new();
    let mut rvcontinue: Option<String> = None;

    loop {
        let mut params: Vec<(&str, &str)> = vec![
            ("action", "query"),
            ("format", "json"),
            ("formatversion", "2"),
            ("prop", "revisions"),
            ("rvprop", "ids|timestamp|user|content"),
            ("rvslots", "main"),
            ("rvlimit", "max"),
            ("rvdir", "newer"),
            ("titles", title),
        ];
        if let Some(cursor) = &rvcontinue {
            params.push(("rvcontinue", cursor));
        }

        let resp: FetchResponse = client
            .get(API)
            .query(&params)
            .send()
            .map_err(|e| ImportError::Network(e.to_string()))?
            .error_for_status()
            .map_err(|e| ImportError::Network(e.to_string()))?
            .json()
            .map_err(|e| ImportError::Network(e.to_string()))?;

        let page = resp
            .query
            .pages
            .into_iter()
            .next()
            .ok_or_else(|| ImportError::Api("empty pages array".into()))?;
        if page.missing == Some(true) {
            return Err(ImportError::MissingPage(page.title));
        }

        for r in page.revisions.unwrap_or_default() {
            if let Some(content) = r.slots.main.content {
                all_revisions.push(Revision {
                    id: r.revid.to_string(),
                    author: r.user,
                    timestamp: r.timestamp,
                    content,
                });
            }
        }

        match resp.continue_.and_then(|c| c.rvcontinue) {
            Some(cursor) => rvcontinue = Some(cursor),
            None => break,
        }
    }

    Ok(all_revisions)
}

// Deserialize structs
#[derive(Deserialize)]
struct FetchResponse {
    query: FetchQuery,
    #[serde(rename = "continue")]
    continue_: Option<FetchContinue>,
}

#[derive(Deserialize)]
struct FetchContinue {
    rvcontinue: Option<String>,
}

#[derive(Deserialize)]
struct FetchQuery {
    pages: Vec<FetchPage>,
}

#[derive(Deserialize)]
struct FetchPage {
    title: String,
    missing: Option<bool>,
    revisions: Option<Vec<FetchRevision>>,
}

#[derive(Deserialize)]
struct FetchRevision {
    revid: u64,
    timestamp: DateTime<Utc>,
    user: String,
    slots: FetchSlots,
}

#[derive(Deserialize)]
struct FetchSlots {
    main: FetchSlotMain,
}

#[derive(Deserialize)]
struct FetchSlotMain {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ApiResponse {
    #[serde(rename = "continue")]
    continue_: Option<Continue>,
    query: Query,
}

#[derive(Deserialize)]
struct Continue {
    rvcontinue: Option<String>,
}

#[derive(Deserialize)]
struct Query {
    pages: Vec<Page>,
}

#[derive(Deserialize)]
struct ProbeRevision {
    timestamp: DateTime<Utc>,
}

#[derive(Deserialize)]
struct Page {
    title: String,

    missing: Option<bool>,

    revisions: Option<Vec<ProbeRevision>>,
}
