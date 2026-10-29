//! `altium init`, `list` and `forget`, their help and their command-line
//! errors, run as a user would, against a throwaway registry
//! (`ALTIUM_CONFIG_DIR`) and project directories.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use altium_core::Store;
use altium_core::registry::{Registry, canonical_path};
use altium_core::task::{Task, TaskId};
use altium_core::time::Timestamp;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use predicates::str::contains;

struct Env {
    tmp: tempfile::TempDir,
}

impl Env {
    fn new() -> Env {
        Env {
            tmp: tempfile::tempdir().unwrap(),
        }
    }

    /// A project directory, created empty.
    fn dir(&self, relative: &str) -> PathBuf {
        let dir = canonical_path(self.tmp.path()).unwrap().join("projects").join(relative);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The registry directory. On Windows, `XDG_CONFIG_HOME` and `HOME` do
    /// not move it: without `ALTIUM_CONFIG_DIR`, tests would use the real one.
    fn config_dir(&self) -> PathBuf {
        self.tmp.path().join("config")
    }

    fn altium(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_altium"));
        command
            .args(args)
            .current_dir(cwd)
            .env("ALTIUM_CONFIG_DIR", self.config_dir())
            .env("HOME", self.tmp.path().join("home"));
        command
    }

    fn registry(&self) -> Registry {
        Registry::at(self.config_dir())
    }
}

#[test]
fn init_in_an_empty_dir_creates_and_registers_the_project() {
    let env = Env::new();
    let foo = env.dir("foo");

    env.altium(&foo, &["init"])
        .assert()
        .success()
        .stdout(contains("Projet « foo » initialisé"))
        .stdout(contains("dashboard  http://127.0.0.1:7420/projects/"));

    let store = Store::open(&foo).unwrap();
    let project = store.project().unwrap();
    assert_eq!(project.name, "foo");
    assert!(store.tasks().unwrap().is_empty());
    assert!(store.memory_nodes().unwrap().is_empty());
    assert!(foo.join(".altium/.gitignore").is_file());

    let entries = env.registry().entries().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        (entries[0].id.as_str(), entries[0].path.as_path()),
        (project.id.as_str(), foo.as_path())
    );
}

#[test]
fn init_prints_the_project_id() {
    let env = Env::new();
    let foo = env.dir("foo");
    let output = env.altium(&foo, &["init"]).output().unwrap();
    let id = Store::open(&foo).unwrap().project().unwrap().id;
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains(&format!("id         {id}"))
    );
}

#[test]
fn init_with_a_name() {
    let env = Env::new();
    let dir = env.dir("api");
    env.altium(&dir, &["init", "--name", "Mon API"])
        .assert()
        .success()
        .stdout(contains("Projet « Mon API » initialisé"));
    assert_eq!(Store::open(&dir).unwrap().project().unwrap().name, "Mon API");

    let empty = env.dir("empty");
    env.altium(&empty, &["init", "--name", "  "])
        .assert()
        .failure()
        .stderr(contains("ne peut pas être vide"));
}

#[test]
fn reinit_keeps_the_data_and_registers_again() {
    let env = Env::new();
    let foo = env.dir("foo");
    env.altium(&foo, &["init"]).assert().success();
    Store::open(&foo)
        .unwrap()
        .update_tasks(|tasks| {
            tasks.push(Task::new(TaskId(1), "Garder cette task", Timestamp::now()));
            Ok::<_, altium_core::StoreError>(())
        })
        .unwrap();
    let tasks_before = fs::read(foo.join(".altium/tasks.jsonl")).unwrap();

    env.altium(&foo, &["init", "--name", "Autre"])
        .assert()
        .success()
        .stdout(contains("déjà initialisé"))
        .stdout(contains("--name est ignorée"))
        .stdout(contains("de nouveau suivi").not());

    env.altium(&foo, &["forget"]).assert().success();
    env.altium(&foo, &["init"])
        .assert()
        .success()
        .stdout(contains("de nouveau suivi"));

    assert_eq!(fs::read(foo.join(".altium/tasks.jsonl")).unwrap(), tasks_before);
    assert_eq!(Store::open(&foo).unwrap().project().unwrap().name, "foo");
    assert_eq!(env.registry().entries().unwrap().len(), 1);
}

#[test]
fn init_completes_a_partial_altium_dir() {
    let env = Env::new();
    let foo = env.dir("foo");
    env.altium(&foo, &["init", "--name", "Mon API"]).assert().success();
    let id = Store::open(&foo).unwrap().project().unwrap().id;
    let altium = foo.join(".altium");
    fs::remove_file(altium.join("project.toml")).unwrap();
    fs::remove_file(altium.join("status.json")).unwrap();
    fs::write(altium.join("tasks.jsonl"), "").unwrap();
    let tasks_before = fs::read(altium.join("tasks.jsonl")).unwrap();

    // The registry still knows the project: it keeps its id and name.
    env.altium(&foo, &["init"])
        .assert()
        .success()
        .stdout(contains("Projet « Mon API » complété"))
        .stdout(contains("créés      status.json, project.toml"));
    let project = Store::open(&foo).unwrap().project().unwrap();
    assert_eq!((project.id, project.name.as_str()), (id, "Mon API"));
    assert_eq!(fs::read(altium.join("tasks.jsonl")).unwrap(), tasks_before);
    assert_eq!(env.registry().entries().unwrap().len(), 1);

    let bar = env.dir("bar");
    fs::create_dir_all(bar.join(".altium/memory")).unwrap();
    env.altium(&bar, &["init"])
        .assert()
        .success()
        .stdout(contains("Projet « bar » complété"))
        .stdout(contains(
            "créés      .gitignore, status.json, tasks.jsonl, project.toml",
        ));
}

#[test]
fn init_keeps_altium_visible_to_git() {
    let env = Env::new();
    let foo = env.dir("foo");
    fs::write(foo.join(".gitignore"), "*\n!/.gitignore\n!/src/**\n").unwrap();

    env.altium(&foo, &["init"])
        .assert()
        .success()
        .stdout(contains("Le .gitignore excluait .altium/ (règle « * »)"));
    let gitignore = fs::read_to_string(foo.join(".gitignore")).unwrap();
    assert!(gitignore.starts_with("*\n!/.gitignore\n!/src/**\n#"));
    assert!(gitignore.ends_with("!/.altium/\n!/.altium/**\n"));
    env.altium(&foo, &["init"])
        .assert()
        .success()
        .stdout(contains(".gitignore").not());

    // Git itself agrees: `.altium/` is versioned, its `runtime/` is not.
    let git = |args: &[&str]| Command::new("git").args(args).current_dir(&foo).status();
    if git(&["init", "-q"]).is_ok_and(|status| status.success()) {
        let ignored = |path| git(&["check-ignore", "-q", path]).unwrap().success();
        assert!(!ignored(".altium/project.toml"));
        assert!(!ignored(".altium/tasks.jsonl"));
        assert!(ignored(".altium/runtime/lock"));
    }
}

#[test]
fn init_in_a_subdirectory_is_refused_unless_forced() {
    let env = Env::new();
    let foo = env.dir("foo");
    let src = env.dir("foo/src");
    env.altium(&foo, &["init"]).assert().success();

    env.altium(&src, &["init"])
        .assert()
        .failure()
        .stderr(contains("fait partie du projet « foo »"))
        .stderr(contains("--force"));
    assert!(!src.join(".altium").exists());

    env.altium(&src, &["init", "--force"])
        .assert()
        .success()
        .stdout(contains("Projet « src » initialisé"));
    assert_eq!(env.registry().entries().unwrap().len(), 2);
}

#[test]
fn list_shows_present_and_missing_projects() {
    let env = Env::new();
    env.altium(&env.dir("outside"), &["list"])
        .assert()
        .success()
        .stdout(contains("Aucun projet suivi"));

    let (a, b) = (env.dir("a"), env.dir("b"));
    env.altium(&a, &["init"]).assert().success();
    env.altium(&b, &["init"]).assert().success();
    fs::remove_dir_all(&b).unwrap();

    let output = env.altium(&a, &["list"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line = |name: &str| stdout.lines().find(|l| l.starts_with(name)).unwrap().to_owned();
    assert!(line("NOM").contains("CHEMIN"));
    assert!(line("a ").ends_with("présent"));
    assert!(line("b ").ends_with("introuvable"));
    assert!(stdout.contains("altium forget <chemin|id>"));
}

#[test]
fn moved_project_keeps_a_single_entry() {
    let env = Env::new();
    let old = env.dir("foo");
    env.altium(&old, &["init"]).assert().success();

    let new = env.dir("work").join("foo");
    fs::rename(&old, &new).unwrap();
    env.altium(&new, &["init"])
        .assert()
        .success()
        .stdout(contains("chemin a été mis à jour"));

    let entries = env.registry().entries().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, new);
    env.altium(&new, &["list"])
        .assert()
        .success()
        .stdout(contains("présent"))
        .stdout(contains("introuvable").not());
}

#[test]
fn forget_keeps_the_altium_dir() {
    let env = Env::new();
    let (a, b) = (env.dir("a"), env.dir("b"));
    env.altium(&a, &["init"]).assert().success();
    env.altium(&b, &["init"]).assert().success();

    // From inside the project, by path, and by id.
    env.altium(&a, &["forget"])
        .assert()
        .success()
        .stdout(contains("Projet « a » retiré du suivi"))
        .stdout(contains("n'a pas été modifié"));
    let id = Store::open(&b).unwrap().project().unwrap().id;
    env.altium(&a, &["forget", &id]).assert().success();
    assert!(env.registry().entries().unwrap().is_empty());
    assert!(a.join(".altium").is_dir() && b.join(".altium").is_dir());

    env.altium(&a, &["forget", "inconnu"])
        .assert()
        .failure()
        .stderr(contains("aucun projet suivi ne correspond à « inconnu »"));
    env.altium(&env.dir("outside"), &["forget"])
        .assert()
        .failure()
        .stderr(contains("aucun projet Altium ici"));
}

#[test]
fn forget_by_path_works_once_the_directory_is_gone() {
    let env = Env::new();
    let gone = env.dir("gone");
    env.altium(&gone, &["init"]).assert().success();
    fs::remove_dir_all(&gone).unwrap();

    env.altium(&env.dir("outside"), &["forget", gone.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Projet « gone » retiré du suivi"));
}

#[test]
fn help_is_in_french() {
    let env = Env::new();
    let dir = env.dir("foo");

    env.altium(&dir, &["--help"])
        .assert()
        .success()
        .stdout(contains("Utilisation : altium <COMMANDE>"))
        .stdout(contains("Commandes :"))
        .stdout(contains("-V, --version  Affiche la version"))
        .stdout(contains("Usage").not())
        .stdout(contains("Print").not());
    env.altium(&dir, &["forget", "-h"])
        .assert()
        .success()
        .stdout(contains("Utilisation : altium forget [PROJET]"))
        .stdout(contains("Arguments :"))
        .stdout(contains("-h, --help  Affiche l'aide"));
    env.altium(&dir, &["-V"])
        .assert()
        .success()
        .stdout(format!("altium {}\n", altium_core::VERSION));
}

#[test]
fn command_line_errors_are_in_french() {
    let env = Env::new();
    let dir = env.dir("foo");

    env.altium(&dir, &["inti"])
        .assert()
        .code(2)
        .stderr(contains("erreur : commande inconnue « inti »"))
        .stderr(contains("conseil : vouliez-vous dire « init » ?"))
        .stderr(contains("Utilisation : altium <COMMANDE>"))
        .stderr(contains("Pour plus d'informations, essayez « --help »."));
    // Clap writes these tips as English sentences.
    env.altium(&dir, &["--force", "init"])
        .assert()
        .code(2)
        .stderr(contains("conseil : « init --force » existe"));
    env.altium(&dir, &["forget", "--foo"]).assert().code(2).stderr(contains(
        "conseil : pour passer « --foo » comme valeur, utilisez « -- --foo »",
    ));
    env.altium(&dir, &["init", "--name"])
        .assert()
        .code(2)
        .stderr(contains("erreur : il manque une valeur pour « --name <NOM> »"));
    assert!(!dir.join(".altium").exists());
}
