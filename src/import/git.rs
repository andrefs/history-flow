//! Git adapter — enumerates a tracked file's history via the `git` binary.

use super::ImportError;
use super::SourceProbe;
use crate::config::Config;
use crate::config::Source;
use crate::import::Revision;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

/// Resolve (owner/repo, file path) from a config: either the explicit `repo` +
/// `page` pair, or parse them out of a GitHub `url`.
pub fn target(config: &Config) -> Result<(String, String), ImportError> {
    if let Some(url) = &config.import.url {
        return super::parse_github_url(url);
    }
    match (&config.import.repo, &config.import.page) {
        (Some(repo), Some(page)) => Ok((repo.clone(), page.clone())),
        _ => Err(ImportError::NoSource),
    }
}

/// Run `git` with `args` in `dir`, returning stdout on success.
fn run_git(dir: &std::path::Path, args: &[&str]) -> Result<String, ImportError> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| ImportError::Git(format!("cannot run git: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(ImportError::Git(format!(
            "`git {}` failed: {err}",
            args.join(" ")
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Return a working directory for `repo`: the repo itself if it is a local
/// path, otherwise a blobless clone of `owner/repo` into a temp cache dir.
fn clone_or_open(repo: &str) -> Result<PathBuf, ImportError> {
    if Path::new(repo).exists() {
        return Ok(repo.into());
    }
    if !repo.contains('/') {
        return Err(ImportError::Git(format!("unknown repo: {repo}")));
    }
    let dir = std::env::temp_dir()
        .join("history-flow")
        .join(repo.replace('/', "_"));
    if !dir.exists() {
        let url = format!("https://github.com/{repo}");
        let out = std::process::Command::new("git")
            .args(["clone", "--filter=blob:none", "--no-checkout", &url])
            .arg(&dir)
            .output()
            .map_err(|e| ImportError::Git(format!("clone failed: {e}")))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(ImportError::Git(format!("clone {repo} failed: {err}")));
        }
    }
    Ok(dir)
}

/// Format the author date (`%aI`) of a commit as UTC.
fn commit_ts(dir: &Path, sha: &str) -> Result<DateTime<Utc>, ImportError> {
    let iso = run_git(dir, &["log", "-1", "--format=%aI", sha])?;
    DateTime::parse_from_rfc3339(iso.trim())
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| ImportError::Git(format!("bad timestamp {iso:?}: {e}")))
}

/// Count revisions touching `path` and report the history's time range.
pub fn probe(config: &Config) -> Result<SourceProbe, ImportError> {
    let (repo, path) = target(config)?;
    let dir = clone_or_open(&repo)?;
    let count: u64 = run_git(&dir, &["rev-list", "--count", "HEAD", "--", &path])?
        .trim()
        .parse()
        .map_err(|e| ImportError::Git(format!("bad rev-list count: {e}")))?;

    let (oldest, newest) = if count == 0 {
        (None, None)
    } else {
        let shas = run_git(&dir, &["rev-list", "--reverse", "HEAD", "--", &path])?;
        let lines: Vec<&str> = shas.lines().collect();
        let oldest = commit_ts(&dir, lines[0])?;
        let newest = commit_ts(&dir, lines[lines.len() - 1])?;
        (Some(oldest), Some(newest))
    };

    Ok(SourceProbe {
        revision_count: count,
        oldest_revision: oldest,
        newest_revision: newest,
        source: Source::Git,
    })
}

/// Enumerate every revision of `path`, oldest first, with full content.
pub fn fetch_revisions(config: &Config) -> Result<Vec<Revision>, ImportError> {
    let (repo, path) = target(config)?;
    let dir = clone_or_open(&repo)?;
    let shas = run_git(&dir, &["rev-list", "--reverse", "HEAD", "--", &path])?;
    eprintln!(
        "fetching {} git revisions for \"{}\"...",
        shas.lines().count(),
        path
    );
    let mut out = Vec::new();
    for sha in shas.lines() {
        let content = run_git(&dir, &["show", &format!("{sha}:{path}")])?;
        let author = run_git(&dir, &["log", "-1", "--format=%an", sha])?
            .trim()
            .to_string();
        let timestamp = commit_ts(&dir, sha)?;
        out.push(Revision {
            id: sha.to_string(),
            author,
            timestamp,
            content,
        });
    }
    Ok(out)
}
