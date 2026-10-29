//! Protection against other web sites: every request must be addressed to
//! the dashboard itself (Host, against DNS rebinding), and every request
//! that changes data must come from it (Origin, against CSRF).

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;

/// Hosts the dashboard answers to.
fn allowed_hosts(port: u16) -> [String; 2] {
    [format!("127.0.0.1:{port}"), format!("localhost:{port}")]
}

/// Whether a request with these headers may go through.
pub fn check(method: &Method, headers: &HeaderMap, port: u16) -> Result<(), &'static str> {
    let hosts = allowed_hosts(port);
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    if !hosts.iter().any(|h| h.eq_ignore_ascii_case(host)) {
        return Err("unexpected Host header");
    }
    let changes = !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS);
    if changes {
        // Browsers always send Origin on cross-site writes; a missing one is a
        // local tool such as curl.
        if let Some(origin) = headers.get(header::ORIGIN) {
            let origin = origin.to_str().unwrap_or_default();
            let ok = hosts
                .iter()
                .any(|h| origin.eq_ignore_ascii_case(&format!("http://{h}")));
            if !ok {
                return Err("cross-origin request refused");
            }
        }
        if headers
            .get("sec-fetch-site")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|site| site == "cross-site")
        {
            return Err("cross-site request refused");
        }
    }
    Ok(())
}

pub async fn middleware(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    match check(request.method(), request.headers(), state.port) {
        Ok(()) => next.run(request).await,
        Err(reason) => (StatusCode::FORBIDDEN, reason).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(host: &str, origin: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_str(host).unwrap());
        if let Some(origin) = origin {
            headers.insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
        }
        headers
    }

    #[test]
    fn host_and_origin() {
        let port = 7420;
        assert!(check(&Method::GET, &headers("127.0.0.1:7420", None), port).is_ok());
        assert!(check(&Method::GET, &headers("localhost:7420", None), port).is_ok());
        assert!(check(&Method::GET, &headers("evil.example:7420", None), port).is_err());
        let same = headers("127.0.0.1:7420", Some("http://127.0.0.1:7420"));
        assert!(check(&Method::POST, &same, port).is_ok());
        let foreign = headers("127.0.0.1:7420", Some("https://exemple.com"));
        assert!(check(&Method::PATCH, &foreign, port).is_err());
        // Reading from another site is harmless: the browser hides the answer.
        assert!(check(&Method::GET, &foreign, port).is_ok());
    }
}
