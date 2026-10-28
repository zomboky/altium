//! `schema/task.schema.json` matches what Altium actually reads and writes.

use std::path::Path;

use altium_core::task::{Priority, TASK_SCHEMA, Task, TaskId, TaskStatus, parse_tasks, render_tasks};
use altium_core::time::Timestamp;
use serde_json::Value;

const FIXTURE: &str = include_str!("fixtures/tasks.jsonl");

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(TASK_SCHEMA).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn assert_valid(validator: &jsonschema::Validator, line: &str) {
    let instance: Value = serde_json::from_str(line).unwrap();
    if let Err(error) = validator.validate(&instance) {
        panic!("{line}\ndoes not match the schema: {error}");
    }
}

#[test]
fn fixtures_match_the_schema_and_the_parser() {
    let validator = validator();
    for line in FIXTURE.lines() {
        assert_valid(&validator, line);
    }

    // The fixture is in canonical form: parsing and rendering keeps it as is.
    let tasks = parse_tasks(FIXTURE, Path::new("tasks.jsonl")).unwrap();
    assert_eq!(tasks.len(), 5);
    assert_eq!(render_tasks(&tasks), FIXTURE);
}

#[test]
fn rendered_tasks_match_the_schema() {
    let now = Timestamp::now();
    let minimal = Task::new(TaskId(1), "Minimal", now);
    let full = Task {
        status: TaskStatus::Done,
        priority: Priority::P0,
        due: Some("2026-11-02T18:00".parse().unwrap()),
        tags: vec!["a".into(), "b".into()],
        parent: Some(TaskId(1)),
        position: Some(0),
        description: Some("desc".into()),
        blocked_reason: Some("why".into()),
        note: Some("note".into()),
        done_at: Some(now),
        ..Task::new(TaskId(2), "Full", now)
    };

    let validator = validator();
    for line in render_tasks(&[minimal, full]).lines() {
        assert_valid(&validator, line);
    }
}

#[test]
fn schema_rejects_what_the_parser_rejects() {
    let validator = validator();
    let bad_lines = [
        r#"{"id":"T-1","title":"x","prio":"P1","rev":1,"ca":"2026-10-28T10:00:00Z","ua":"2026-10-28T10:00:00Z"}"#,
        r#"{"id":"12","title":"x","rev":1,"ca":"2026-10-28T10:00:00Z","ua":"2026-10-28T10:00:00Z"}"#,
        r#"{"id":"T-1","title":"x","st":"wip","rev":1,"ca":"2026-10-28T10:00:00Z","ua":"2026-10-28T10:00:00Z"}"#,
        r#"{"id":"T-1","title":"x","due":"02/11/2026","rev":1,"ca":"2026-10-28T10:00:00Z","ua":"2026-10-28T10:00:00Z"}"#,
    ];
    for line in bad_lines {
        let instance: Value = serde_json::from_str(line).unwrap();
        assert!(!validator.is_valid(&instance), "schema accepts {line}");
        assert!(
            parse_tasks(line, Path::new("tasks.jsonl")).is_err(),
            "parser accepts {line}"
        );
    }
}
