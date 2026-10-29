//! The dashboard API over a real loopback socket: security headers, task
//! routes and the change stream.

use std::time::{Duration, Instant};

use altium_core::registry::Registry;
use altium_core::store::Store;
use altium_server::AppState;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

struct Server {
    port: u16,
    project: String,
    root: std::path::PathBuf,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
}

async fn start() -> Server {
    let config = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();
    let registry = Registry::at(config.path());
    let store = Store::init(project_dir.path(), "Demo").unwrap();
    let project = store.project().unwrap();
    registry.register(&project, project_dir.path()).unwrap();
    let listener = altium_server::bind(0).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = AppState::new(registry, port);
    tokio::spawn(altium_server::serve(listener, state, false));
    Server {
        port,
        project: project.id,
        root: project_dir.path().to_path_buf(),
        _dirs: (config, project_dir),
    }
}

/// Sends one request with the given extra headers; returns status and body.
async fn request(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: Option<Value>) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let mut text = format!(
        "{method} {path} HTTP/1.1\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    if !headers.iter().any(|(name, _)| name.eq_ignore_ascii_case("host")) {
        text.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    }
    if !body.is_empty() {
        text.push_str("Content-Type: application/json\r\n");
    }
    for (name, value) in headers {
        text.push_str(&format!("{name}: {value}\r\n"));
    }
    text.push_str("\r\n");
    text.push_str(&body);
    stream.write_all(text.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let response = String::from_utf8_lossy(&response).into_owned();
    let status = response.split_whitespace().nth(1).unwrap().parse().unwrap();
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();
    (status, body)
}

#[tokio::test]
async fn foreign_host_and_origin_are_refused() {
    let server = start().await;
    let port = server.port;
    let (status, _) = request(port, "GET", "/api/health", &[("Host", "evil.example")], None).await;
    assert_eq!(status, 403);
    let (status, body) = request(port, "GET", "/api/health", &[], None).await;
    assert_eq!(status, 200);
    assert!(body.contains("\"ok\":true"));

    let path = format!("/api/projects/{}/tasks", server.project);
    let new = json!({"title": "Depuis un autre site"});
    let foreign = [("Origin", "https://exemple.com")];
    let (status, _) = request(port, "POST", &path, &foreign, Some(new.clone())).await;
    assert_eq!(status, 403);
    let cross = [("Sec-Fetch-Site", "cross-site")];
    let (status, _) = request(port, "POST", &path, &cross, Some(new.clone())).await;
    assert_eq!(status, 403);
    let origin = format!("http://127.0.0.1:{port}");
    let (status, _) = request(port, "POST", &path, &[("Origin", &origin)], Some(new)).await;
    assert_eq!(status, 201);
}

#[tokio::test]
async fn task_routes_and_conflicts() {
    let server = start().await;
    let port = server.port;
    let base = format!("/api/projects/{}", server.project);

    let (status, body) = request(port, "GET", "/api/projects", &[], None).await;
    assert_eq!(status, 200);
    assert!(body.contains("Demo"), "{body}");

    let (status, body) = request(
        port,
        "POST",
        &format!("{base}/tasks"),
        &[],
        Some(json!({"title": "Écrire l'API", "priority": "P1"})),
    )
    .await;
    assert_eq!(status, 201, "{body}");
    let task: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(task["id"], "T-1");

    let path = format!("{base}/tasks/T-1");
    let edit = json!({"title": {"base": "Écrire l'API", "value": "Écrire l'API REST"}});
    let (status, body) = request(port, "PATCH", &path, &[], Some(edit.clone())).await;
    assert_eq!(status, 200, "{body}");
    // Same edit again: its base is stale now.
    let (status, body) = request(port, "PATCH", &path, &[], Some(edit)).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("conflict"));

    let (status, body) = request(port, "GET", &format!("{base}/tasks"), &[], None).await;
    assert_eq!(status, 200);
    assert!(body.contains("Écrire l'API REST"), "{body}");

    let (status, _) = request(port, "GET", &format!("{base}/tasks/T-9"), &[], None).await;
    assert_eq!(status, 404);
    let (status, _) = request(port, "DELETE", &path, &[], None).await;
    assert!(status == 200 || status == 204, "{status}");
    let (status, _) = request(port, "GET", "/api/projects/inconnu", &[], None).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn stream_reports_a_file_change_quickly() {
    let server = start().await;
    let port = server.port;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let text = format!("GET /api/stream HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/event-stream\r\n\r\n");
    stream.write_all(text.as_bytes()).await.unwrap();

    let mut seen = String::new();
    let mut buffer = [0u8; 4096];
    while !seen.contains("event: hello") {
        let n = stream.read(&mut buffer).await.unwrap();
        seen.push_str(&String::from_utf8_lossy(&buffer[..n]));
    }
    // Let the watcher settle before changing a file behind the server's back.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let started = Instant::now();
    let store = Store::open(&server.root).unwrap();
    altium_core::ops::set_status(
        &store,
        Some("Écrit à côté du serveur".into()),
        None,
        &altium_core::actor::Actor::User,
        &altium_core::time::Clock::system(),
    )
    .unwrap();
    let deadline = Duration::from_secs(2);
    loop {
        let read = tokio::time::timeout(deadline.saturating_sub(started.elapsed()), stream.read(&mut buffer)).await;
        let n = read.expect("no change event within 2 s").unwrap();
        seen.push_str(&String::from_utf8_lossy(&buffer[..n]));
        if seen.contains("\"scope\":\"status\"") {
            break;
        }
    }
    assert!(started.elapsed() < deadline);
}

#[tokio::test]
async fn shutdown_stops_the_server() {
    let server = start().await;
    let (status, _) = request(server.port, "POST", "/api/shutdown", &[], None).await;
    assert_eq!(status, 204);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(TcpStream::connect(("127.0.0.1", server.port)).await.is_err());
}
