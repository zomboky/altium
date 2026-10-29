//! A minimal HTTP/1.1 GET against the local dashboard, enough to check it
//! is running without pulling an HTTP client.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use anyhow::{Context, Result};

/// Returns the status code and body of `GET path` on `host:port`.
pub fn get(host: &str, port: u16, path: &str) -> Result<(u16, String)> {
    let address: SocketAddr = format!("{host}:{port}").parse()?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(500))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let status = response
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .context("réponse HTTP invalide")?;
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();
    Ok((status, body))
}
