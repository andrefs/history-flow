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
    let has_head = run_git(&dir, &["rev-parse", "--verify", "HEAD"]).is_ok();
    let count: u64 = if has_head {
        run_git(&dir, &["rev-list", "--count", "HEAD", "--", &path])?
            .trim()
            .parse()
            .map_err(|e| ImportError::Git(format!("bad rev-list count: {e}")))?
    } else {
        0
    };

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQ: AtomicUsize = AtomicUsize::new(0);

    fn fresh_repo_dir() -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("hf-git-test-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        run_git(&dir, &["init", "-q", "-b", "main"]).unwrap();
        run_git(&dir, &["config", "user.email", "t@t"]).unwrap();
        run_git(&dir, &["config", "user.name", "tester"]).unwrap();
        dir
    }

    fn commit_file(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
        run_git(dir, &["add", name]).unwrap();
        run_git(dir, &["commit", "-q", "-m", "update"]).unwrap();
    }

    fn git_config(repo: String, page: String) -> Config {
        let mut c = Config::default();
        c.import.repo = Some(repo);
        c.import.page = Some(page);
        c
    }

    #[test]
    fn probe_counts_commits_touching_file() {
        let dir = fresh_repo_dir();
        commit_file(&dir, "notes.txt", "line one\n");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        commit_file(&dir, "notes.txt", "line one\nline two\n");

        let p = probe(&git_config(
            dir.to_string_lossy().into_owned(),
            "notes.txt".into(),
        ))
        .unwrap();

        assert_eq!(p.revision_count, 2);
        assert_eq!(p.source, Source::Git);
        let (o, n) = (p.oldest_revision.unwrap(), p.newest_revision.unwrap());
        assert!(o < n);
    }

    #[test]
    fn fetch_revisions_oldest_first_with_content() {
        let dir = fresh_repo_dir();
        commit_file(&dir, "notes.txt", "line one\n");
        commit_file(&dir, "notes.txt", "line one\nline two\n");

        let revs = fetch_revisions(&git_config(
            dir.to_string_lossy().into_owned(),
            "notes.txt".into(),
        ))
        .unwrap();

        assert_eq!(revs.len(), 2);
        assert_eq!(revs[0].content, "line one\n");
        assert_eq!(revs[1].content, "line one\nline two\n");
        assert_eq!(revs[0].author, "tester");
        assert!(revs[0].timestamp <= revs[1].timestamp);
        assert_ne!(revs[0].id, revs[1].id);
    }

    #[test]
    fn probe_empty_repo_is_no_revisions() {
        let dir = fresh_repo_dir();
        let p = probe(&git_config(
            dir.to_string_lossy().into_owned(),
            "notes.txt".into(),
        ))
        .unwrap();
        assert_eq!(p.revision_count, 0);
        assert_eq!(p.oldest_revision, None);
        assert_eq!(p.newest_revision, None);
    }
}
