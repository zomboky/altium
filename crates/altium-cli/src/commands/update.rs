//! `altium update`: rebuilds Altium from the latest commit of its git
//! repository and replaces the running binary. Once a day, the first
//! command run starts it in the background.
//!
//! The source lives in its own clone (`altium/source` in the local data
//! directory, or in `$ALTIUM_CONFIG_DIR`), never in a working copy the user
//! edits. Building needs git, cargo and npm, like `make build`.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use altium_core::config::GlobalConfig;
use altium_core::registry::{CONFIG_DIR_VAR, Registry};
use altium_core::{git, log};
use anyhow::{Context, Result, bail};

use super::serve;
use crate::display;
use crate::http;

/// The commit this binary was built from, empty when unknown.
pub const COMMIT: &str = env!("ALTIUM_COMMIT");

/// Branch followed by the updates.
const BRANCH: &str = "main";

/// Set to any value, turns off the daily update.
const NO_AUTO_VAR: &str = "ALTIUM_NO_AUTO_UPDATE";

/// An update older than this is considered dead and its lock ignored.
const LOCK_TIMEOUT: Duration = Duration::from_secs(2 * 3600);

/// Where this binary stands compared with the repository.
#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    UpToDate,
    /// The repository has `commits` more commits.
    Behind {
        commits: usize,
    },
    /// This binary has commits the repository does not have.
    Ahead,
    /// Built from a commit the repository does not know, or from no commit.
    Unknown,
}

fn short(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

/// How a step's output is shown.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Output {
    Terminal,
    /// The daily update: everything goes to `update.log`.
    Log,
}

struct Updater {
    repo: String,
    source: PathBuf,
    output: Output,
}

impl Updater {
    fn new(output: Output) -> Result<Updater> {
        let config = GlobalConfig::for_user();
        Ok(Updater {
            repo: config.update_repo().to_owned(),
            source: source_dir()?,
            output,
        })
    }

    fn command(&self, program: &str, args: &[&str], cwd: &Path) -> Result<()> {
        let mut command = Command::new(program);
        command.args(args).current_dir(cwd).stdin(Stdio::null());
        if self.output == Output::Log {
            let file = log_file()?;
            command.stdout(file.try_clone()?).stderr(file);
            git::no_window(&mut command);
        }
        let status = command
            .status()
            .with_context(|| format!("{program} introuvable : il faut git, cargo et npm pour mettre Altium à jour"))?;
        if !status.success() {
            bail!("`{program} {}` a échoué ({status})", args.join(" "));
        }
        Ok(())
    }

    fn git_output(&self, args: &[&str]) -> Option<String> {
        let mut command = Command::new("git");
        command.args(args).current_dir(&self.source).stdin(Stdio::null());
        git::no_window(&mut command);
        let output = command.output().ok()?;
        let text = String::from_utf8(output.stdout).ok()?.trim().to_owned();
        output.status.success().then_some(text)
    }

    /// Clones the repository the first time, fetches it afterwards; returns
    /// the latest commit of the branch.
    fn fetch(&self) -> Result<String> {
        if self.source.join(".git").exists() {
            self.command("git", &["fetch", "--quiet", "origin", BRANCH], &self.source)?;
        } else {
            let parent = self.source.parent().context("dossier des sources sans parent")?;
            fs::create_dir_all(parent)?;
            let target = self.source.to_string_lossy().into_owned();
            self.command(
                "git",
                &["clone", "--quiet", "--branch", BRANCH, &self.repo, &target],
                parent,
            )?;
        }
        self.git_output(&["rev-parse", &format!("origin/{BRANCH}")])
            .context("branche main introuvable dans le dépôt")
    }

    fn status(&self, current: &str, latest: &str) -> Status {
        let is_ancestor = |a: &str, b: &str| self.git_output(&["merge-base", "--is-ancestor", a, b]).is_some();
        if current == latest {
            Status::UpToDate
        } else if current.is_empty()
            || self
                .git_output(&["cat-file", "-e", &format!("{current}^{{commit}}")])
                .is_none()
        {
            Status::Unknown
        } else if is_ancestor(current, latest) {
            let commits = self
                .git_output(&["rev-list", "--count", &format!("{current}..{latest}")])
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            Status::Behind { commits }
        } else {
            Status::Ahead
        }
    }

    /// Checks the source out at `commit` and builds the release binary.
    fn build(&self, commit: &str) -> Result<PathBuf> {
        self.command("git", &["reset", "--quiet", "--hard", commit], &self.source)?;
        let npm = env::var("NPM").unwrap_or_else(|_| if cfg!(windows) { "npm.cmd" } else { "npm" }.into());
        let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        self.command(
            &npm,
            &["--prefix", "web", "ci", "--no-fund", "--no-audit"],
            &self.source,
        )?;
        self.command(&npm, &["--prefix", "web", "run", "build"], &self.source)?;
        let target = self.source.join("target").to_string_lossy().into_owned();
        self.command(
            &cargo,
            &[
                "build",
                "--release",
                "--locked",
                "-p",
                "altium-cli",
                "--target-dir",
                &target,
            ],
            &self.source,
        )?;
        Ok(self.source.join("target/release").join(exe_name()))
    }
}

fn exe_name() -> &'static str {
    if cfg!(windows) { "altium.exe" } else { "altium" }
}

/// The clone `altium update` builds from.
fn source_dir() -> Result<PathBuf> {
    if let Some(dir) = env::var_os(CONFIG_DIR_VAR).filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir).join("source"));
    }
    Ok(dirs::data_local_dir()
        .context("dossier de données local introuvable")?
        .join("altium")
        .join("source"))
}

/// Output of the daily update, next to the Altium log.
fn log_file() -> Result<File> {
    let path = log::path()
        .context("dossier du journal introuvable")?
        .with_file_name("update.log");
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    Ok(OpenOptions::new().create(true).append(true).open(path)?)
}

/// A binary from a cargo target directory: a developer build, which the
/// daily update leaves alone.
pub fn is_dev_build(exe: &Path) -> bool {
    let parts: Vec<_> = exe.components().collect();
    parts.windows(2).any(|pair| {
        pair[0] == Component::Normal("target".as_ref())
            && matches!(pair[1], Component::Normal(name) if name == "debug" || name == "release")
    })
}

/// Puts `new` in place of `exe`. A running binary cannot be overwritten on
/// Windows but can be renamed: it moves aside to `altium.old.exe`, removed
/// at the next update.
fn install(new: &Path, exe: &Path) -> Result<()> {
    let staged = exe.with_file_name(format!("altium.new{}", env::consts::EXE_SUFFIX));
    fs::copy(new, &staged).with_context(|| format!("copie vers {}", display::path(&staged)))?;
    #[cfg(windows)]
    {
        let old = old_exe(exe);
        let _ = fs::remove_file(&old);
        fs::rename(exe, &old).with_context(|| format!("impossible de déplacer {}", display::path(exe)))?;
        if let Err(e) = fs::rename(&staged, exe) {
            let _ = fs::rename(&old, exe);
            return Err(e).context("impossible d'installer le nouveau binaire");
        }
    }
    #[cfg(not(windows))]
    {
        // Renaming over the running binary is fine: it keeps its inode.
        fs::rename(&staged, exe).context("impossible d'installer le nouveau binaire")?;
    }
    Ok(())
}

fn old_exe(exe: &Path) -> PathBuf {
    exe.with_file_name(format!("altium.old{}", env::consts::EXE_SUFFIX))
}

/// Restarts the dashboard on the new binary, if it was running.
fn restart_dashboard(exe: &Path) -> Result<bool> {
    let port = serve::port(None)?;
    if !serve::is_running(port) {
        return Ok(false);
    }
    #[cfg(target_os = "linux")]
    if serve::restart_unit()? {
        return Ok(true);
    }
    let _ = http::post("127.0.0.1", port, "/api/shutdown");
    for _ in 0..50 {
        if !serve::is_running(port) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    serve::start_background_with(exe, port)?;
    Ok(true)
}

/// Holds the update lock while alive.
struct Lock(PathBuf);

impl Lock {
    fn take(dir: &Path) -> Option<Lock> {
        let path = dir.join("update.lock");
        let _ = fs::create_dir_all(dir);
        let stale = fs::metadata(&path)
            .and_then(|m| m.modified())
            .is_ok_and(|t| SystemTime::now().duration_since(t).unwrap_or_default() > LOCK_TIMEOUT);
        if stale {
            let _ = fs::remove_file(&path);
        }
        OpenOptions::new().write(true).create_new(true).open(&path).ok()?;
        Some(Lock(path))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn run(check: bool, force: bool, auto: bool) -> Result<()> {
    let output = if auto { Output::Log } else { Output::Terminal };
    let registry = Registry::for_user()?;
    let Some(_lock) = Lock::take(registry.dir()) else {
        if auto {
            return Ok(());
        }
        bail!("une mise à jour est déjà en cours");
    };
    let exe = dunce::canonicalize(env::current_exe()?)?;
    let _ = fs::remove_file(old_exe(&exe));
    let updater = Updater::new(output)?;
    let say = |message: &str| match output {
        Output::Terminal => println!("{message}"),
        Output::Log => log::write("update", message),
    };

    if output == Output::Terminal {
        println!("Recherche d'une nouvelle version dans {}…", updater.repo);
    }
    let latest = updater.fetch()?;
    let status = updater.status(COMMIT, &latest);
    let (current, next) = (short(COMMIT), short(&latest));
    match status {
        Status::UpToDate => {
            if !auto {
                say(&format!("Altium est à jour ({current})."));
            }
            return Ok(());
        }
        Status::Ahead if !force => {
            if !auto {
                say(&format!(
                    "Ce binaire ({current}) est plus récent que {BRANCH} ({next}) : rien à installer."
                ));
            }
            return Ok(());
        }
        Status::Unknown if !force => {
            if !auto {
                say(&format!(
                    "Ce binaire vient du commit {}, absent de {} : c'est une version de développement. \
                     `altium update --force` installe {next}.",
                    if current.is_empty() { "inconnu" } else { current },
                    updater.repo
                ));
            }
            return Ok(());
        }
        Status::Behind { commits } if check => {
            say(&format!(
                "Nouvelle version disponible : {current} → {next} ({commits} commit{}). Lance `altium update`.",
                if commits > 1 { "s" } else { "" }
            ));
            return Ok(());
        }
        _ if check => {
            say(&format!("`altium update --force` installerait {next}."));
            return Ok(());
        }
        _ => {}
    }

    if output == Output::Terminal {
        println!(
            "Compilation de {next} dans {} (quelques minutes la première fois)…",
            display::path(&updater.source)
        );
    }
    let built = updater.build(&latest)?;
    install(&built, &exe)?;
    let restarted = restart_dashboard(&exe).unwrap_or_else(|e| {
        log::write("update", &format!("redémarrage du dashboard : {e:#}"));
        false
    });
    say(&format!(
        "Altium mis à jour : {} → {next}{}.",
        if current.is_empty() { "?" } else { current },
        if restarted { ", dashboard redémarré" } else { "" }
    ));
    Ok(())
}

/// The daily update: at the first command of the day, starts
/// `altium update --auto` in the background. Never fails and costs one
/// small file read the rest of the day.
pub fn daily() {
    if env::var_os(NO_AUTO_VAR).is_some() {
        return;
    }
    let Ok(exe) = env::current_exe() else { return };
    if is_dev_build(&exe) {
        return;
    }
    let Ok(registry) = Registry::for_user() else { return };
    let Ok(config) = GlobalConfig::load(registry.dir()) else {
        return;
    };
    if !config.auto_update() {
        return;
    }
    let stamp = registry.dir().join("update-check");
    let today = chrono::Local::now().date_naive().to_string();
    if fs::read_to_string(&stamp).is_ok_and(|day| day.trim() == today) {
        return;
    }
    if fs::create_dir_all(registry.dir()).is_err() || fs::write(&stamp, &today).is_err() {
        return;
    }
    let mut command = Command::new(exe);
    command
        .args(["update", "--auto"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | BELOW_NORMAL_PRIORITY_CLASS);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    if let Err(e) = command.spawn() {
        log::write("update", &format!("lancement de la mise à jour quotidienne : {e}"));
    }
}

/// Runs the daily update for `altium update --auto`, reporting errors in
/// the log rather than on a console nobody sees.
pub fn run_auto() {
    if let Err(e) = run(false, false, true) {
        log::write("update", &format!("{e:#}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_builds_are_binaries_of_a_cargo_target_dir() {
        assert!(is_dev_build(Path::new("/src/altium/target/debug/altium")));
        #[cfg(windows)]
        assert!(is_dev_build(Path::new(r"H:\altium\target\release\altium.exe")));
        assert!(!is_dev_build(Path::new("/home/me/.local/bin/altium")));
        assert!(!is_dev_build(Path::new("/srv/target/altium")));
    }

    #[test]
    fn short_commits() {
        assert_eq!(short("0123456789abcdef"), "0123456");
        assert_eq!(short(""), "");
    }
}
