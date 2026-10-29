//! Where a project stands: summary edits, phase, task progress and the
//! health indicator.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::actor::Actor;
use crate::project::{MAX_SUMMARY_CHARS, ProjectConfig, ProjectStatus};
use crate::task::{Task, TaskStatus};
use crate::tasks::{DeadlineState, deadline_state};
use crate::time::Clock;

/// Sets the summary and/or phase of `status`, checking both.
pub fn set_status(
    status: &mut ProjectStatus,
    config: &ProjectConfig,
    summary: Option<String>,
    phase: Option<String>,
    actor: Actor,
    clock: &Clock,
) -> Result<(), String> {
    if let Some(summary) = &summary {
        let chars = summary.trim().chars().count();
        if chars > MAX_SUMMARY_CHARS {
            return Err(format!(
                "summary is {chars} characters, the limit is {MAX_SUMMARY_CHARS}"
            ));
        }
    }
    if let Some(phase) = &phase {
        let phases = config.phases();
        if !phases.contains(phase) {
            return Err(format!(
                "unknown phase `{phase}` (expected one of: {})",
                phases.join(", ")
            ));
        }
    }
    let before = status.clone();
    if let Some(summary) = summary {
        status.summary = summary.trim().to_owned();
    }
    if let Some(phase) = phase {
        status.phase = phase;
    }
    if *status != before {
        status.updated_at = Some(clock.timestamp());
        status.updated_by = Some(actor);
    }
    Ok(())
}

/// Task counts of a project.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Progress {
    pub by_status: BTreeMap<String, usize>,
    pub done: usize,
    /// Tasks that count: all but cancelled ones.
    pub total: usize,
    /// Rounded down, 0 without tasks.
    pub percent: u32,
    pub overdue: usize,
    pub near: usize,
    pub blocked: usize,
}

pub fn progress(tasks: &[Task], clock: &Clock, near_hours: u32) -> Progress {
    let mut progress = Progress::default();
    for task in tasks {
        let status = serde_json::to_value(task.status).expect("status serializes");
        *progress
            .by_status
            .entry(status.as_str().unwrap_or_default().to_owned())
            .or_default() += 1;
        match task.status {
            TaskStatus::Cancelled => continue,
            TaskStatus::Done => progress.done += 1,
            TaskStatus::Blocked => progress.blocked += 1,
            _ => {}
        }
        progress.total += 1;
        match deadline_state(task, clock, near_hours) {
            DeadlineState::Overdue => progress.overdue += 1,
            DeadlineState::Near => progress.near += 1,
            _ => {}
        }
    }
    progress.percent = (progress.done * 100).checked_div(progress.total).unwrap_or(0) as u32;
    progress
}

/// Health flags of a project; several can be set at once, none is "calm".
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Health {
    pub overdue: bool,
    pub blocked: bool,
    pub active: bool,
}

impl Health {
    pub fn new(progress: &Progress, active_session: bool) -> Health {
        Health {
            overdue: progress.overdue > 0,
            blocked: progress.blocked > 0,
            active: active_session,
        }
    }

    pub fn is_calm(&self) -> bool {
        !(self.overdue || self.blocked || self.active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskId;
    use crate::time::Timestamp;

    fn clock() -> Clock {
        Clock::fixed("2026-10-28T12:00:00Z".parse::<Timestamp>().unwrap().as_datetime())
    }

    #[test]
    fn ten_tasks_progress() {
        let c = clock();
        let tasks: Vec<Task> = (1..=10)
            .map(|n| {
                let mut task = Task::new(TaskId(n), "x", c.timestamp());
                task.status = match n {
                    1..=4 => TaskStatus::Done,
                    5 => TaskStatus::Cancelled,
                    _ => TaskStatus::Todo,
                };
                if n == 6 {
                    task.due = Some("2026-10-27".parse().unwrap());
                }
                task
            })
            .collect();
        let p = progress(&tasks, &c, 48);
        assert_eq!((p.done, p.total, p.percent, p.overdue), (4, 9, 44, 1));
        let health = Health::new(&p, true);
        assert!(health.overdue && health.active && !health.blocked);
    }

    #[test]
    fn summary_and_phase_are_checked() {
        let c = clock();
        let config = ProjectConfig::default();
        let mut status = ProjectStatus::default();
        let claude = Actor::Claude { session: None };
        assert!(set_status(&mut status, &config, Some("x".repeat(601)), None, claude.clone(), &c).is_err());
        assert!(set_status(&mut status, &config, None, Some("shipping".into()), claude.clone(), &c).is_err());
        set_status(
            &mut status,
            &config,
            Some("API en place".into()),
            Some("testing".into()),
            claude.clone(),
            &c,
        )
        .unwrap();
        assert_eq!((status.phase.as_str(), status.updated_by), ("testing", Some(claude)));
    }
}
