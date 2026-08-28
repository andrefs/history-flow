//! Wikipedia Action API adapter — metadata-only probe.

use super::{ImportError, SourceProbe};

use crate::config::Source;
use crate::import::Revision;
use chrono::{DateTime, Utc};
use serde::Deserialize;

const API: &str = "https://en.wikipedia.org/w/api.php";

/// Build the blocking HTTP client with our user-agent.
fn new_client() -> Result<reqwest::blocking::Client, ImportError> {
    reqwest::blocking::Client::builder()
        .user_agent("history-flow/0.1 (https://github.com/andrefs/history-flow; Rust)")
        .build()
        .map_err(|e| ImportError::Network(e.to_string()))
}

/// Count revisions and capture the oldest/newest timestamps for a page.
/// Fetches `ids|timestamp` metadata only (never revision content), paging
/// via the API's `rvcontinue` cursor until pages are exhausted.
pub fn probe(title: &str) -> Result<SourceProbe, ImportError> {
    eprintln!("probing Wikipedia page \"{title}\"...");
    probe_with_client(&new_client()?, API, title)
}

/// Internal probe against an explicit API base URL (test seam).
fn probe_with_client(
    client: &reqwest::blocking::Client,
    api: &str,
    title: &str,
) -> Result<SourceProbe, ImportError> {
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
            .get(api)
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
        eprintln!("  ...{count} revisions so far");
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
    eprintln!("fetching full revision content for \"{title}\"...");
    fetch_with_client(&new_client()?, API, title)
}

/// Internal fetch against an explicit API base URL (test seam).
fn fetch_with_client(
    client: &reqwest::blocking::Client,
    api: &str,
    title: &str,
) -> Result<Vec<Revision>, ImportError> {
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
            .get(api)
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
        eprintln!("  ...{} revisions fetched", all_revisions.len());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Source;
    use chrono::{TimeZone, Utc};
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// Serve `PAGE1` on the first connection and `PAGE2` (no continue) on the
    /// second, so the paging loop exhausts after two requests.
    fn stub_api() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for body in [PAGE1, PAGE2] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                let _ = reader.read_line(&mut request_line);
                while reader.read_line(&mut String::new()).unwrap_or(0) > 2 {}
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(resp.as_bytes()).unwrap();
                stream.flush().unwrap();
            }
        });
        format!("http://{}", addr)
    }

    const PAGE1: &str = r#"{
  "continue": {"rvcontinue": "20240101000000|1234"},
  "query": {
    "pages": [{"title": "Test Page", "revisions": [
      {"timestamp": "2024-01-01T00:00:00Z"},
      {"timestamp": "2023-01-01T00:00:00Z"}
    ]}]
  }
}"#;

    const PAGE2: &str = r#"{
  "query": {
    "pages": [{"title": "Test Page", "revisions": [
      {"timestamp": "2022-01-01T00:00:00Z"}
    ]}]
  }
}"#;

    #[test]
    fn probe_pages_via_rvcontinue() {
        let base = stub_api();
        let client = new_client().unwrap();
        let result = probe_with_client(&client, &base, "Test Page").unwrap();
        assert_eq!(result.revision_count, 3);
        assert_eq!(result.source, Source::Wikipedia);
        assert_eq!(
            result.newest_revision,
            Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap())
        );
        assert_eq!(
            result.oldest_revision,
            Some(Utc.with_ymd_and_hms(2022, 1, 1, 0, 0, 0).unwrap())
        );
    }
}
