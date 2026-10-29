//! REST API under `/api`, the SSE stream, and the embedded front.

use std::convert::Infallible;
use std::sync::Arc;

use altium_core::StoreError;
use altium_core::activity;
use altium_core::actor::Actor;
use altium_core::graph::Search;
use altium_core::memory::NodeKind;
use altium_core::ops::{self, OpError};
use altium_core::patch::{Edit, TaskPatch};
use altium_core::project::Project;
use altium_core::store::Store;
use altium_core::task::{Priority, TaskId, TaskStatus};
use altium_core::tasks::{self, NewTask, TaskError};
use altium_core::time::{Clock, Deadline};
use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, Uri, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post, put};
use futures_util::Stream;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use crate::{AppState, assets, security, views};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/shutdown", post(shutdown))
        .route("/api/projects", get(projects))
        .route("/api/projects/{id}", get(project).delete(forget))
        .route("/api/projects/{id}/status", put(set_status))
        .route("/api/projects/{id}/tasks", get(task_list).post(create_task))
        .route(
            "/api/projects/{id}/tasks/{task}",
            get(task_detail).patch(update_task).delete(delete_task),
        )
        .route("/api/projects/{id}/tasks/{task}/move", post(move_task))
        .route("/api/projects/{id}/activity", get(activity_feed))
        .route("/api/projects/{id}/stats", get(stats))
        .route("/api/projects/{id}/memory", get(memory_index))
        .route("/api/projects/{id}/memory/{node}", get(memory_node))
        .route("/api/projects/{id}/memory/{node}/pin", put(pin_node))
        .route("/api/projects/{id}/git", get(git))
        .route("/api/stream", get(stream))
        .fallback(static_file)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            security::middleware,
        ))
        .with_state(state)
}

/// An API error, as `{"error": "..."}` with a status code.
pub struct ApiError(StatusCode, Value);

impl ApiError {
    fn new(status: StatusCode, message: impl ToString) -> ApiError {
        ApiError(status, json!({"error": message.to_string()}))
    }

    fn not_found(what: impl ToString) -> ApiError {
        ApiError::new(StatusCode::NOT_FOUND, what)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(self.1)).into_response()
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> ApiError {
        let status = match &e {
            StoreError::ReadOnly { .. } => StatusCode::CONFLICT,
            StoreError::Invalid { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            StoreError::LockTimeout(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, e)
    }
}

impl From<TaskError> for ApiError {
    fn from(e: TaskError) -> ApiError {
        match e {
            TaskError::Store(e) => e.into(),
            TaskError::NotFound(id) => ApiError::not_found(format!("tâche {id} introuvable")),
            TaskError::Conflict(conflict) => {
                let clock = Clock::system();
                let task = views::task(&conflict.task, std::slice::from_ref(&conflict.task), &clock, 48);
                ApiError(
                    StatusCode::CONFLICT,
                    json!({"error": "conflict", "fields": conflict.fields, "task": task}),
                )
            }
            TaskError::OpenSubtasks { open, .. } => ApiError(
                StatusCode::CONFLICT,
                json!({
                    "error": "open_subtasks",
                    "open": open.iter().map(ToString::to_string).collect::<Vec<_>>(),
                }),
            ),
            other => ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, other),
        }
    }
}

impl From<OpError> for ApiError {
    fn from(e: OpError) -> ApiError {
        match e {
            OpError::Store(e) => e.into(),
            OpError::NodeNotFound(_) => ApiError::not_found(e),
            OpError::Invalid(_) => ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, e),
        }
    }
}

type ApiResult<T> = Result<T, ApiError>;

/// Opens the registered project `id`.
fn open(state: &AppState, id: &str) -> ApiResult<(Store, Project)> {
    let entry = state
        .registry
        .entries()?
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| ApiError::not_found(format!("projet {id} inconnu")))?;
    let store = Store::open(&entry.path).map_err(|_| ApiError::not_found("dossier du projet introuvable"))?;
    let project = store.project()?;
    if project.id != id {
        return Err(ApiError::not_found("le dossier contient un autre projet"));
    }
    Ok((store, project))
}

fn near(state: &AppState, project: &Project) -> u32 {
    state.config().near_hours(&project.config)
}

async fn shutdown(State(state): State<Arc<AppState>>) -> StatusCode {
    state.shutdown.notify_one();
    StatusCode::NO_CONTENT
}

async fn health() -> Json<Value> {
    Json(json!({"ok": true, "version": altium_core::VERSION, "front": assets::has_front()}))
}

async fn projects(State(state): State<Arc<AppState>>) -> ApiResult<Json<Value>> {
    let state2 = state.clone();
    let cards = tokio::task::spawn_blocking(move || -> ApiResult<Vec<views::ProjectCard>> {
        let config = state2.config();
        let clock = Clock::system();
        Ok(state2
            .registry
            .list()?
            .iter()
            .map(|listed| views::card(listed, &config, &clock))
            .collect())
    })
    .await
    .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e))??;
    let mut list = Vec::new();
    for card in cards {
        let mut value = serde_json::to_value(&card).expect("cards serialize");
        let git = if card.present {
            let path = std::path::PathBuf::from(&card.path);
            state.git(path).await.and_then(|g| g.branch)
        } else {
            None
        };
        value["branch"] = json!(git);
        list.push(value);
    }
    Ok(Json(json!({"projects": list})))
}

async fn project(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let (store, project) = open(&state, &id)?;
    let detail = views::detail(&store, &project, &state.config(), &Clock::system());
    Ok(Json(serde_json::to_value(detail).expect("details serialize")))
}

async fn forget(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    match state.registry.forget(&id)? {
        Some(_) => {
            let _ = state.changes.send(crate::Change {
                project: None,
                scope: "projects",
            });
            Ok(StatusCode::NO_CONTENT)
        }
        None => Err(ApiError::not_found(format!("projet {id} inconnu"))),
    }
}

#[derive(Deserialize)]
struct StatusBody {
    summary: Option<String>,
    phase: Option<String>,
}

async fn set_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<StatusBody>,
) -> ApiResult<Json<Value>> {
    let (store, _) = open(&state, &id)?;
    let status = ops::set_status(&store, body.summary, body.phase, &Actor::User, &Clock::system())?;
    Ok(Json(serde_json::to_value(status).expect("status serializes")))
}

async fn task_list(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let (store, project) = open(&state, &id)?;
    let all = store.tasks()?;
    let list = views::task_list(&all, &Clock::system(), near(&state, &project));
    Ok(Json(serde_json::to_value(list).expect("tasks serialize")))
}

fn task_id(text: &str) -> ApiResult<TaskId> {
    text.parse()
        .map_err(|e: String| ApiError::new(StatusCode::BAD_REQUEST, e))
}

async fn task_detail(
    State(state): State<Arc<AppState>>,
    Path((id, task)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let (store, project) = open(&state, &id)?;
    let task_id = task_id(&task)?;
    let all = store.tasks()?;
    let found = tasks::find(&all, task_id)?;
    let mut value = serde_json::to_value(views::task(found, &all, &Clock::system(), near(&state, &project)))
        .expect("tasks serialize");
    value["history"] = serde_json::to_value(ops::history(&store, &task)).expect("history serializes");
    Ok(Json(value))
}

async fn create_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(new): Json<NewTask>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let (store, project) = open(&state, &id)?;
    let clock = Clock::system();
    let task = ops::create_task(&store, new, &Actor::User, &clock)?;
    let all = store.tasks()?;
    let view = views::task(&task, &all, &clock, near(&state, &project));
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(view).expect("tasks serialize")),
    ))
}

/// A field edit from the dashboard: the value it was based on and the new one.
#[derive(Deserialize)]
struct FieldEdit<T> {
    base: T,
    value: T,
}

impl<T> From<FieldEdit<T>> for Edit<T> {
    fn from(edit: FieldEdit<T>) -> Edit<T> {
        Edit::from(edit.base, edit.value)
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PatchBody {
    title: Option<FieldEdit<String>>,
    description: Option<FieldEdit<Option<String>>>,
    status: Option<FieldEdit<TaskStatus>>,
    priority: Option<FieldEdit<Priority>>,
    due: Option<FieldEdit<Option<Deadline>>>,
    tags: Option<FieldEdit<Vec<String>>>,
    parent: Option<FieldEdit<Option<TaskId>>>,
    blocked_reason: Option<FieldEdit<Option<String>>>,
    note: Option<FieldEdit<Option<String>>>,
    /// Close even with open subtasks, after the user confirmed.
    force: bool,
}

async fn update_task(
    State(state): State<Arc<AppState>>,
    Path((id, task)): Path<(String, String)>,
    Json(body): Json<PatchBody>,
) -> ApiResult<Json<Value>> {
    let (store, project) = open(&state, &id)?;
    let task_id = task_id(&task)?;
    let patch = TaskPatch {
        title: body.title.map(Into::into),
        description: body.description.map(Into::into),
        status: body.status.map(Into::into),
        priority: body.priority.map(Into::into),
        due: body.due.map(Into::into),
        tags: body.tags.map(Into::into),
        parent: body.parent.map(Into::into),
        position: None,
        blocked_reason: body.blocked_reason.map(Into::into),
        note: body.note.map(Into::into),
    };
    let clock = Clock::system();
    let near = near(&state, &project);
    let result = ops::update_task(&store, task_id, &patch, body.force, &Actor::User, &clock);
    match result {
        Ok(task) => {
            let all = store.tasks()?;
            Ok(Json(
                serde_json::to_value(views::task(&task, &all, &clock, near)).expect("tasks serialize"),
            ))
        }
        Err(TaskError::Conflict(conflict)) => {
            let all = store.tasks()?;
            Err(ApiError(
                StatusCode::CONFLICT,
                json!({"error": "conflict", "fields": conflict.fields, "task": views::task(&conflict.task, &all, &clock, near)}),
            ))
        }
        Err(e) => Err(e.into()),
    }
}

async fn delete_task(
    State(state): State<Arc<AppState>>,
    Path((id, task)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let (store, _) = open(&state, &id)?;
    let removed = ops::delete_task(&store, task_id(&task)?, &Actor::User, &Clock::system())?;
    Ok(Json(
        json!({"removed": removed.iter().map(ToString::to_string).collect::<Vec<_>>()}),
    ))
}

#[derive(Deserialize)]
struct MoveBody {
    before: Option<TaskId>,
}

async fn move_task(
    State(state): State<Arc<AppState>>,
    Path((id, task)): Path<(String, String)>,
    Json(body): Json<MoveBody>,
) -> ApiResult<StatusCode> {
    let (store, _) = open(&state, &id)?;
    ops::move_task(&store, task_id(&task)?, body.before, &Actor::User, &Clock::system())?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ActivityQuery {
    tool: Option<String>,
    session: Option<String>,
    limit: Option<usize>,
}

async fn activity_feed(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<ActivityQuery>,
) -> ApiResult<Json<Value>> {
    let (store, _) = open(&state, &id)?;
    let config = state.config();
    let clock = Clock::system();
    let sessions = views::sessions(&store, &config, &clock);
    let events = activity::recent_events(&store);
    let mut calls = activity::tool_calls(&events);
    let tools: std::collections::BTreeSet<String> = calls.iter().map(|c| c.tool.clone()).collect();
    calls.retain(|c| {
        query.tool.as_ref().is_none_or(|t| &c.tool == t) && query.session.as_ref().is_none_or(|s| &c.session == s)
    });
    calls.truncate(query.limit.unwrap_or(200));
    Ok(Json(json!({"sessions": sessions, "calls": calls, "tools": tools})))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StatsQuery {
    days: Option<i64>,
}

async fn stats(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<StatsQuery>,
) -> ApiResult<Json<Value>> {
    let (store, _) = open(&state, &id)?;
    let stats = views::stats(&store, query.days.unwrap_or(7), &Clock::system());
    Ok(Json(serde_json::to_value(stats).expect("stats serialize")))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MemoryQuery {
    text: Option<String>,
    kind: Option<NodeKind>,
    tag: Option<String>,
}

async fn memory_index(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<MemoryQuery>,
) -> ApiResult<Json<Value>> {
    let (store, _) = open(&state, &id)?;
    let (graph, access) = ops::load_graph(&store)?;
    let mut lines = views::memory_index(&store, &graph);
    if query.text.is_some() || query.kind.is_some() || query.tag.is_some() {
        let search = Search {
            text: query.text,
            kind: query.kind,
            tag: query.tag,
            path: None,
        };
        let order: Vec<String> = graph
            .search(&search, &access, Clock::system().now)
            .iter()
            .map(|n| n.id.clone())
            .collect();
        lines.retain(|l| order.contains(&l.id));
        lines.sort_by_key(|l| order.iter().position(|id| id == &l.id));
    }
    Ok(Json(json!({"nodes": lines})))
}

async fn memory_node(
    State(state): State<Arc<AppState>>,
    Path((id, node)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let (store, _) = open(&state, &id)?;
    let (graph, _) = ops::load_graph(&store)?;
    let found = graph
        .get(&node)
        .ok_or_else(|| ApiError::not_found(format!("nœud {node} introuvable")))?;
    Ok(Json(
        serde_json::to_value(views::node(&store, &graph, found)).expect("nodes serialize"),
    ))
}

#[derive(Deserialize)]
struct PinBody {
    pinned: bool,
}

async fn pin_node(
    State(state): State<Arc<AppState>>,
    Path((id, node)): Path<(String, String)>,
    Json(body): Json<PinBody>,
) -> ApiResult<StatusCode> {
    let (store, _) = open(&state, &id)?;
    ops::pin(&store, &node, body.pinned, &Actor::User, &Clock::system())?;
    Ok(StatusCode::NO_CONTENT)
}

async fn git(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let (store, _) = open(&state, &id)?;
    let info = state.git(store.root().to_path_buf()).await;
    Ok(Json(json!({"git": info})))
}

/// Pushes changes as SSE events named after their scope.
async fn stream(State(state): State<Arc<AppState>>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut receiver = state.changes.subscribe();
    let stream = async_stream(move |sender| async move {
        let _ = sender.send(Event::default().event("hello").data("{}")).await;
        loop {
            match receiver.recv().await {
                Ok(change) => {
                    let data = serde_json::to_string(&change).expect("changes serialize");
                    if sender.send(Event::default().event("change").data(data)).await.is_err() {
                        break;
                    }
                }
                // Missed changes: the client refetches everything.
                Err(RecvError::Lagged(_)) => {
                    if sender.send(Event::default().event("resync").data("{}")).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// A stream fed by an async task through a channel.
fn async_stream<F, Fut>(f: F) -> impl Stream<Item = Result<Event, Infallible>>
where
    F: FnOnce(tokio::sync::mpsc::Sender<Event>) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let (sender, mut receiver) = tokio::sync::mpsc::channel(32);
    tokio::spawn(f(sender));
    futures_util::stream::poll_fn(move |cx| receiver.poll_recv(cx).map(|e| e.map(Ok)))
}

/// Embedded front files; unknown paths get `index.html` for the client router.
async fn static_file(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") {
        return ApiError::not_found("route inconnue").into_response();
    }
    match assets::get(path).filter(|_| !path.is_empty()) {
        Some(data) => {
            let mime = mime(path);
            let cache = if path.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            Response::builder()
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, cache)
                .body(Body::from(data.into_owned()))
                .expect("valid response")
        }
        None => Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(assets::index_html().into_owned()))
            .expect("valid response"),
    }
}

fn mime(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "json" | "webmanifest" => "application/json",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}
