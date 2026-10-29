//! Desktop notifications: deadlines near or past, and tasks Claude blocks.
//! Sent by the running server, at most once per task and occurrence, the
//! keys of sent notifications kept in `runtime/notified.json`.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use altium_core::actor::Actor;
use altium_core::config::GlobalConfig;
use altium_core::ops;
use altium_core::project::Project;
use altium_core::store::Store;
use altium_core::task::{Task, TaskStatus};
use altium_core::tasks::{DeadlineState, deadline_state, is_open};
use altium_core::time::Clock;
use chrono::Duration as Span;

use crate::AppState;

pub const NOTIFIED_FILE: &str = "notified.json";

/// Windows shows notifications under this application id; `altium service
/// install` registers its display name.
pub const APP_ID: &str = "Altium";

const PERIOD: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Normal,
    Critical,
}

/// A notification to send.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
    pub urgency: Urgency,
    /// Dashboard page to open from the notification.
    pub url: String,
}

/// Sends notifications; replaced by a recorder in tests.
pub trait Emitter: Send + Sync {
    fn send(&self, notice: &Notice) -> Result<(), String>;
}

/// Keys of notifications already sent: `T-3:near:2026-11-02`.
pub type Notified = BTreeSet<String>;

/// "1 j 23 h", "5 h 10 min", "12 min".
pub fn remaining(span: Span) -> String {
    let minutes = span.num_minutes().max(0);
    let (days, hours, mins) = (minutes / 1440, minutes % 1440 / 60, minutes % 60);
    match (days, hours) {
        (0, 0) => format!("{mins} min"),
        (0, h) => format!("{h} h {mins:02} min"),
        (d, h) => format!("{d} j {h} h"),
    }
}

/// The notifications due for one project, marking their keys in `notified`.
/// `startup` groups deadlines passed while the server was stopped into one
/// summary.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    project: &Project,
    tasks: &[Task],
    blocked_by: impl Fn(&Task) -> Option<(Actor, String)>,
    notified: &mut Notified,
    clock: &Clock,
    near_hours: u32,
    base_url: &str,
    startup: bool,
) -> Vec<Notice> {
    let url = |task: &Task| format!("{base_url}/projects/{}/tasks?task={}", project.id, task.id);
    let mut notices = Vec::new();
    let mut overdue = Vec::new();
    for task in tasks {
        if !is_open(task.status) {
            continue;
        }
        if let Some(due) = task.due {
            let state = deadline_state(task, clock, near_hours);
            let key = |kind: &str| format!("{}:{kind}:{due}", task.id);
            match state {
                DeadlineState::Overdue if notified.insert(key("overdue")) => {
                    // An overdue task needs no "near" notification any more.
                    notified.insert(key("near"));
                    overdue.push(task);
                }
                DeadlineState::Near if notified.insert(key("near")) => notices.push(Notice {
                    title: format!("{} · {}", project.name, task.title),
                    body: format!("échéance dans {}", remaining(clock.deadline(due) - clock.now)),
                    urgency: Urgency::Normal,
                    url: url(task),
                }),
                _ => {}
            }
        }
        if task.status == TaskStatus::Blocked
            && let Some((actor, at)) = blocked_by(task)
        {
            let key = format!("{}:blocked:{at}", task.id);
            if notified.insert(key) {
                // Only Claude's blocks are worth a notification.
                if matches!(actor, Actor::Claude { .. }) {
                    notices.push(Notice {
                        title: format!("{} · {}", project.name, task.title),
                        body: format!("bloqué : {}", task.blocked_reason.as_deref().unwrap_or("?")),
                        urgency: Urgency::Normal,
                        url: url(task),
                    });
                }
            }
        }
    }
    match overdue.as_slice() {
        [] => {}
        [task] => notices.push(Notice {
            title: format!("{} · {}", project.name, task.title),
            body: "en retard".into(),
            urgency: Urgency::Critical,
            url: url(task),
        }),
        many if startup => notices.push(Notice {
            title: format!("{} · {} tasks en retard", project.name, many.len()),
            body: many.iter().map(|t| t.title.as_str()).collect::<Vec<_>>().join(", "),
            urgency: Urgency::Critical,
            url: format!("{base_url}/projects/{}/tasks?deadline=overdue", project.id),
        }),
        many => notices.extend(many.iter().map(|task| Notice {
            title: format!("{} · {}", project.name, task.title),
            body: "en retard".into(),
            urgency: Urgency::Critical,
            url: url(task),
        })),
    }
    // Keys of closed or rescheduled tasks would grow forever.
    let live: BTreeSet<String> = tasks.iter().map(|t| format!("{}:", t.id)).collect();
    notified.retain(|key| live.iter().any(|prefix| key.starts_with(prefix)));
    notices
}

/// Who last set the task's status to blocked, and when, from the history.
fn blocked_by(store: &Store, task: &Task) -> Option<(Actor, String)> {
    ops::history(store, &task.id.to_string())
        .into_iter()
        .rev()
        .find(|c| c.fields.get("st").and_then(|v| v.as_str()) == Some("blocked"))
        .map(|c| (c.by, c.at.to_string()))
}

/// One pass over every project.
pub fn run_once(state: &AppState, emitter: &dyn Emitter, clock: &Clock, startup: bool) {
    let config: GlobalConfig = state.config();
    if !config.notifications() {
        return;
    }
    let base_url = format!("http://127.0.0.1:{}", state.port);
    for listed in state.registry.list().unwrap_or_default() {
        let Some(project) = listed.project else { continue };
        if project.config.notifications == Some(false) {
            continue;
        }
        let Ok(store) = Store::open(&listed.entry.path) else {
            continue;
        };
        let Ok(tasks) = store.tasks() else { continue };
        let near = config.near_hours(&project.config);
        let notices = store.update_runtime(NOTIFIED_FILE, |notified: &mut Notified| {
            plan(
                &project,
                &tasks,
                |t| blocked_by(&store, t),
                notified,
                clock,
                near,
                &base_url,
                startup,
            )
        });
        for notice in notices.unwrap_or_default() {
            if let Err(e) = emitter.send(&notice) {
                altium_core::log::write("notifications", &format!("{}: {e}", notice.title));
            }
        }
    }
}

/// Desktop notifications through the system service (D-Bus, WinRT).
pub struct Desktop;

impl Emitter for Desktop {
    fn send(&self, notice: &Notice) -> Result<(), String> {
        let mut notification = notify_rust::Notification::new();
        notification
            .summary(&notice.title)
            .body(&notice.body)
            .appname("Altium")
            .urgency(match notice.urgency {
                Urgency::Normal => notify_rust::Urgency::Normal,
                Urgency::Critical => notify_rust::Urgency::Critical,
            });
        #[cfg(windows)]
        notification.app_id(APP_ID);
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            notification.action("default", "Ouvrir").action("open", "Ouvrir");
            let handle = notification.show().map_err(|e| e.to_string())?;
            let url = notice.url.clone();
            std::thread::spawn(move || {
                handle.wait_for_action(|action| {
                    if action == "default" || action == "open" {
                        let _ = opener::open(&url);
                    }
                });
            });
            return Ok(());
        }
        #[allow(unreachable_code)]
        notification.show().map(|_| ()).map_err(|e| e.to_string())
    }
}

/// Runs the scheduler every minute, and on every change of tasks.
pub fn spawn(state: Arc<AppState>) {
    let mut changes = state.changes.subscribe();
    tokio::spawn(async move {
        let first = state.clone();
        let _ = tokio::task::spawn_blocking(move || run_once(&first, &Desktop, &Clock::system(), true)).await;
        let mut timer = tokio::time::interval(PERIOD);
        timer.tick().await;
        loop {
            tokio::select! {
                _ = timer.tick() => {}
                change = changes.recv() => match change {
                    Ok(change) if change.scope == "tasks" => {}
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                },
            }
            let state = state.clone();
            let _ = tokio::task::spawn_blocking(move || run_once(&state, &Desktop, &Clock::system(), false)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use altium_core::patch::{Edit, TaskPatch};
    use altium_core::registry::Registry;
    use altium_core::tasks::NewTask;
    use altium_core::time::Timestamp;

    use super::*;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<Notice>>);

    impl Emitter for Recorder {
        fn send(&self, notice: &Notice) -> Result<(), String> {
            self.0.lock().unwrap().push(notice.clone());
            Ok(())
        }
    }

    impl Recorder {
        fn take(&self) -> Vec<Notice> {
            std::mem::take(&mut self.0.lock().unwrap())
        }
    }

    fn at(text: &str) -> Clock {
        Clock::fixed(text.parse::<Timestamp>().unwrap().as_datetime())
    }

    fn setup() -> (tempfile::TempDir, Arc<AppState>, Store) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("foo");
        std::fs::create_dir_all(&root).unwrap();
        let store = Store::init(&root, "foo").unwrap();
        let registry = Registry::at(tmp.path().join("config"));
        registry.register(&store.project().unwrap(), &root).unwrap();
        (tmp, AppState::new(registry, 7420), store)
    }

    fn add(store: &Store, title: &str, due: &str, by: &Actor) -> Task {
        let new = NewTask {
            title: title.into(),
            due: Some(due.parse().unwrap()),
            ..NewTask::default()
        };
        ops::create_task(store, new, by, &Clock::system()).unwrap()
    }

    #[test]
    fn near_then_overdue_once_each_even_after_restart() {
        let (_tmp, state, store) = setup();
        add(&store, "Écrire le README", "2026-10-30T11:00", &Actor::User);
        let recorder = Recorder::default();

        run_once(&state, &recorder, &at("2026-10-28T12:00:00Z"), true);
        let sent = recorder.take();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].title, "foo · Écrire le README");
        assert_eq!(sent[0].body, "échéance dans 1 j 23 h");

        // Restarted server, same situation: nothing new.
        run_once(&state, &recorder, &at("2026-10-28T12:05:00Z"), true);
        assert!(recorder.take().is_empty());

        run_once(&state, &recorder, &at("2026-10-30T11:01:00Z"), false);
        let sent = recorder.take();
        assert_eq!((sent.len(), sent[0].urgency), (1, Urgency::Critical));
    }

    #[test]
    fn rescheduled_deadline_notifies_again() {
        let (_tmp, state, store) = setup();
        let task = add(&store, "x", "2026-10-29", &Actor::User);
        let recorder = Recorder::default();
        run_once(&state, &recorder, &at("2026-10-28T12:00:00Z"), false);
        assert_eq!(recorder.take().len(), 1);
        let patch = TaskPatch {
            due: Some(Edit::set(Some("2026-11-05".parse().unwrap()))),
            ..TaskPatch::default()
        };
        ops::update_task(&store, task.id, &patch, false, &Actor::User, &Clock::system()).unwrap();
        run_once(&state, &recorder, &at("2026-11-04T12:00:00Z"), false);
        assert_eq!(recorder.take().len(), 1);
    }

    #[test]
    fn startup_summarizes_missed_deadlines() {
        let (_tmp, state, store) = setup();
        for n in 0..3 {
            add(&store, &format!("t{n}"), "2026-10-20", &Actor::User);
        }
        let recorder = Recorder::default();
        run_once(&state, &recorder, &at("2026-10-28T12:00:00Z"), true);
        let sent = recorder.take();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].title, "foo · 3 tasks en retard");
    }

    #[test]
    fn only_claude_blocks_notify() {
        let (_tmp, state, store) = setup();
        let claude = Actor::Claude { session: None };
        let block = |id, by: &Actor| {
            let patch = TaskPatch {
                status: Some(Edit::set(TaskStatus::Blocked)),
                blocked_reason: Some(Edit::set(Some("clé API manquante".into()))),
                ..TaskPatch::default()
            };
            ops::update_task(&store, id, &patch, false, by, &Clock::system()).unwrap();
        };
        let mine = add(&store, "Mine", "2027-01-01", &Actor::User);
        let stripe = add(&store, "Intégrer Stripe", "2027-01-01", &Actor::User);
        block(mine.id, &Actor::User);
        block(stripe.id, &claude);
        let recorder = Recorder::default();
        run_once(&state, &recorder, &Clock::system(), false);
        let sent = recorder.take();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].title, "foo · Intégrer Stripe");
        assert_eq!(sent[0].body, "bloqué : clé API manquante");
    }

    #[test]
    fn notifications_off_for_a_project() {
        let (_tmp, state, store) = setup();
        add(&store, "x", "2026-10-29", &Actor::User);
        store
            .update_project(|p| {
                p.config.notifications = Some(false);
                Ok::<_, altium_core::StoreError>(())
            })
            .unwrap();
        let recorder = Recorder::default();
        run_once(&state, &recorder, &at("2026-10-28T12:00:00Z"), false);
        assert!(recorder.take().is_empty());
    }
}
