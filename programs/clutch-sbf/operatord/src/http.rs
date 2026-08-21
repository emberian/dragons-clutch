//! A loopback-only HTTP/1.1 surface: static files, one action endpoint, and
//! one server-sent-event stream.
//!
//! Hand-written on `std::net` for the same reason the page has no framework:
//! a bench that exists to make a trust boundary visible should not ask you to
//! trust a dependency tree to see it.  The listener binds `127.0.0.1` and
//! nothing else, so there is no configuration under which this serves a
//! network.

use crate::bus::Bus;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fs, thread};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Extensions the bench is allowed to serve, and their media types.
///
/// An allowlist rather than a guess: a file the bench did not mean to publish
/// is a 404, not an `application/octet-stream`.
const MEDIA: [(&str, &str); 5] = [
    ("html", "text/html; charset=utf-8"),
    ("css", "text/css; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("svg", "image/svg+xml"),
    ("json", "application/json"),
];

/// What a POST to `/api` is answered by.
pub type Action = Arc<dyn Fn(&Value) -> Value + Send + Sync>;

pub struct Server {
    listener: TcpListener,
    bus: Arc<Bus>,
    root: PathBuf,
    action: Action,
}

impl Server {
    /// Bind the bench to a loopback port.
    pub fn bind(port: u16, bus: Arc<Bus>, root: PathBuf, action: Action) -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        Ok(Self {
            listener,
            bus,
            root,
            action,
        })
    }

    pub fn port(&self) -> Result<u16> {
        Ok(self.listener.local_addr()?.port())
    }

    /// Serve until the process ends; one thread per connection.
    pub fn serve_forever(self) {
        for stream in self.listener.incoming() {
            let Ok(stream) = stream else { continue };
            let bus = Arc::clone(&self.bus);
            let root = self.root.clone();
            let action = Arc::clone(&self.action);
            thread::spawn(move || {
                let _ignored = handle(stream, &bus, &root, action.as_ref());
            });
        }
    }
}

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_request(stream: &TcpStream) -> Result<Request> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let mut headers: HashMap<String, String> = HashMap::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            break;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let length: usize = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0_u8; length.min(1 << 20)];
    if !body.is_empty() {
        reader.read_exact(&mut body)?;
    }
    Ok(Request {
        method,
        path,
        body,
    })
}

fn respond(stream: &mut TcpStream, status: &str, media: &str, body: &[u8]) -> Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {media}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

fn not_found(stream: &mut TcpStream) -> Result<()> {
    respond(stream, "404 Not Found", "text/plain; charset=utf-8", b"404\n")
}

/// Resolve one request path inside the bench's own directory.
///
/// Refuses traversal and anything outside the media allowlist, so the daemon
/// cannot be turned into a file server for the repository it lives in.
fn resolve(root: &Path, path: &str) -> Option<(PathBuf, &'static str)> {
    let trimmed = path.split('?').next().unwrap_or(path);
    let relative = match trimmed {
        "/" => "index.html",
        other => other.strip_prefix('/')?,
    };
    if relative.is_empty()
        || relative.contains("..")
        || relative.contains("//")
        || relative.starts_with('/')
    {
        return None;
    }
    if !relative
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._-/".contains(&byte))
    {
        return None;
    }
    let extension = relative.rsplit('.').next()?;
    let media = MEDIA
        .iter()
        .find(|(name, _)| *name == extension)
        .map(|(_, media)| *media)?;
    Some((root.join(relative), media))
}

fn handle(mut stream: TcpStream, bus: &Bus, root: &Path, action: &dyn Fn(&Value) -> Value) -> Result<()> {
    let request = read_request(&stream)?;
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/events") => {
            let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                        Cache-Control: no-store\r\nConnection: keep-alive\r\n\
                        X-Accel-Buffering: no\r\n\r\n";
            stream.write_all(head.as_bytes())?;
            stream.flush()?;
            bus.subscribe(stream);
            Ok(())
        }
        ("POST", "/api") => {
            let parsed: Value = serde_json::from_slice(&request.body)
                .unwrap_or_else(|error| json!({"action": "invalid", "detail": error.to_string()}));
            let reply = action(&parsed);
            respond(
                &mut stream,
                "200 OK",
                "application/json",
                reply.to_string().as_bytes(),
            )
        }
        ("GET", path) => match resolve(root, path) {
            Some((file, media)) => match fs::read(&file) {
                Ok(body) => respond(&mut stream, "200 OK", media, &body),
                Err(_) => not_found(&mut stream),
            },
            None => not_found(&mut stream),
        },
        _ => respond(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            b"405\n",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bench_root_is_its_index() {
        let root = Path::new("/bench");
        let (file, media) = resolve(root, "/").expect("root resolves");
        assert_eq!(file, root.join("index.html"));
        assert_eq!(media, "text/html; charset=utf-8");
    }

    #[test]
    fn traversal_and_unlisted_media_are_refused() {
        let root = Path::new("/bench");
        assert!(resolve(root, "/../../Cargo.toml").is_none());
        assert!(resolve(root, "/a/../../b.js").is_none());
        assert!(resolve(root, "/keys.json.pem").is_none());
        assert!(resolve(root, "/notes.txt").is_none());
        assert!(resolve(root, "//etc/passwd").is_none());
        assert!(resolve(root, "/app.js").is_some());
        assert!(resolve(root, "/styles.css").is_some());
    }

    #[test]
    fn a_query_string_does_not_change_the_file() {
        let root = Path::new("/bench");
        let (file, _) = resolve(root, "/app.js?v=2").expect("query is stripped");
        assert_eq!(file, root.join("app.js"));
    }
}
