//! Repository information read with the `git` command: no C dependency, and
//! git behaves as configured by the user.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

/// Longest a git command may run.
pub const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct GitInfo {
    /// `None` on a detached HEAD.
    pub branch: Option<String>,
    pub modified: usize,
    pub untracked: usize,
    /// Commits ahead of and behind the upstream branch, when there is one.
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub commits: Vec<Commit>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Commit {
    pub hash: String,
    pub message: String,
    /// ISO 8601 committer date.
    pub date: String,
}

/// Reads the repository containing `root`; `None` when it is not in a git
/// repository, git is missing or too slow.
pub fn read(root: &Path) -> Option<GitInfo> {
    let status = run(root, &["status", "--porcelain=v2", "--branch"])?;
    let mut info = parse_status(&status);
    if let Some(log) = run(root, &["log", "-5", "--format=%h%x1f%s%x1f%cI"]) {
        info.commits = parse_log(&log);
    }
    Some(info)
}

fn parse_status(text: &str) -> GitInfo {
    let mut info = GitInfo::default();
    for line in text.lines() {
        if let Some(head) = line.strip_prefix("# branch.head ") {
            info.branch = (head != "(detached)").then(|| head.to_owned());
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            let mut parts = ab.split_whitespace();
            info.ahead = parts.next().and_then(|a| a.trim_start_matches('+').parse().ok());
            info.behind = parts.next().and_then(|b| b.trim_start_matches('-').parse().ok());
        } else if line.starts_with("1 ") || line.starts_with("2 ") || line.starts_with("u ") {
            info.modified += 1;
        } else if line.starts_with("? ") {
            info.untracked += 1;
        }
    }
    info
}

fn parse_log(text: &str) -> Vec<Commit> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\u{1f}');
            Some(Commit {
                hash: parts.next()?.to_owned(),
                message: parts.next()?.to_owned(),
                date: parts.next()?.to_owned(),
            })
        })
        .collect()
}

/// Runs `git <args>` in `root` and returns its output if it succeeds within
/// [`TIMEOUT`].
fn run(root: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    no_window(&mut command);
    let mut child = command.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut text = String::new();
        stdout.read_to_string(&mut text).ok().map(|_| text)
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let text = reader.join().ok().flatten()?;
                return status.success().then_some(text);
            }
            Ok(None) if start.elapsed() < TIMEOUT => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// On Windows, keeps console programs started by the windowless server from
/// opening a console window.
pub fn no_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    #[test]
    fn reads_a_repository() {
        let remote = tempfile::tempdir().unwrap();
        git(remote.path(), &["init", "-q", "--bare", "-b", "main"]);
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        fs::write(dir.path().join("a.txt"), "a").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-q", "-m", "Premier commit"]);
        git(
            dir.path(),
            &["remote", "add", "origin", &remote.path().display().to_string()],
        );
        git(dir.path(), &["push", "-q", "-u", "origin", "main"]);
        git(
            dir.path(),
            &["checkout", "-q", "-b", "feat/x", "--track", "origin/main"],
        );
        for n in 0..2 {
            fs::write(dir.path().join("a.txt"), format!("{n}")).unwrap();
            git(dir.path(), &["commit", "-q", "-am", &format!("Commit {n}")]);
        }
        fs::write(dir.path().join("a.txt"), "changed").unwrap();
        fs::write(dir.path().join("new.txt"), "new").unwrap();

        let info = read(dir.path()).unwrap();
        assert_eq!(info.branch.as_deref(), Some("feat/x"));
        assert_eq!((info.modified, info.untracked), (1, 1));
        assert_eq!((info.ahead, info.behind), (Some(2), Some(0)));
        assert_eq!(info.commits.len(), 3);
        assert_eq!(info.commits[0].message, "Commit 1");
    }

    #[test]
    fn no_section_outside_a_repository() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(dir.path()).is_none());
    }
}
