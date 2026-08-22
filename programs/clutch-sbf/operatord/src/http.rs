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
use solana_keypair::{Keypair, Signer};
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
const CAPABILITY_COOKIE: &str = "operator_capability";

/// What a POST to `/api` is answered by.
pub type Action = Arc<dyn Fn(&Value) -> Value + Send + Sync>;

pub struct Server {
    listener: TcpListener,
    bus: Arc<Bus>,
    root: PathBuf,
    action: Action,
    capability: String,
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
            capability: Keypair::new().pubkey().to_string(),
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
            let capability = self.capability.clone();
            thread::spawn(move || {
                let _ignored = handle(stream, &bus, &root, action.as_ref(), &capability);
            });
        }
    }
}

struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
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
        headers,
        body,
    })
}

fn respond_with_headers(
    stream: &mut TcpStream,
    status: &str,
    media: &str,
    body: &[u8],
    headers: &[(&str, &str)],
) -> Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {media}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\
         Content-Security-Policy: default-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; connect-src 'self'; style-src 'self' 'unsafe-inline'\r\n\
         Referrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\n\
         Connection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

fn respond(stream: &mut TcpStream, status: &str, media: &str, body: &[u8]) -> Result<()> {
    respond_with_headers(stream, status, media, body, &[])
}

fn not_found(stream: &mut TcpStream) -> Result<()> {
    respond(
        stream,
        "404 Not Found",
        "text/plain; charset=utf-8",
        b"404\n",
    )
}

/// Authenticate the browser-facing authority before serving any route.
///
/// Binding a listener to loopback does not by itself stop DNS rebinding: a
/// hostile page can resolve its own hostname to 127.0.0.1 and send requests
/// with that hostile `Host`.  The bench has only two valid authorities, both
/// tied to the port the kernel actually assigned.
fn valid_loopback_host(request: &Request, port: u16) -> bool {
    let Some(host) = request.headers.get("host") else {
        return false;
    };
    host == &format!("127.0.0.1:{port}") || host == &format!("localhost:{port}")
}

/// POSTs must be non-simple JSON requests from this exact bench origin.
/// Command-line clients do not send `Origin`, so they remain usable for the
/// checked local gate.  A browser that does send one must name the same
/// authority as `Host`; wildcard CORS is deliberately absent.
fn valid_api_headers(request: &Request) -> bool {
    let Some(content_type) = request.headers.get("content-type") else {
        return false;
    };
    if !content_type
        .split(';')
        .next()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
    {
        return false;
    }
    match request.headers.get("origin") {
        None => true,
        Some(origin) => request
            .headers
            .get("host")
            .is_some_and(|host| origin == &format!("http://{host}")),
    }
}

fn valid_capability(request: &Request, capability: &str) -> bool {
    let expected = format!("{CAPABILITY_COOKIE}={capability}");
    request.headers.get("cookie").is_some_and(|cookies| {
        cookies
            .split(';')
            .any(|cookie| cookie.trim() == expected.as_str())
    })
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

fn handle(
    mut stream: TcpStream,
    bus: &Bus,
    root: &Path,
    action: &dyn Fn(&Value) -> Value,
    capability: &str,
) -> Result<()> {
    let port = stream.local_addr()?.port();
    let request = read_request(&stream)?;
    if !valid_loopback_host(&request, port) {
        return respond(
            &mut stream,
            "403 Forbidden",
            "text/plain; charset=utf-8",
            b"403\n",
        );
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/events") => {
            if !valid_capability(&request, capability) {
                return respond(
                    &mut stream,
                    "403 Forbidden",
                    "text/plain; charset=utf-8",
                    b"403\n",
                );
            }
            let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                        Cache-Control: no-store\r\nConnection: keep-alive\r\n\
                        X-Content-Type-Options: nosniff\r\n\
                        Content-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\n\
                        Referrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\n\
                        X-Accel-Buffering: no\r\n\r\n";
            stream.write_all(head.as_bytes())?;
            stream.flush()?;
            bus.subscribe(stream);
            Ok(())
        }
        ("POST", "/api") => {
            if !valid_capability(&request, capability) {
                return respond(
                    &mut stream,
                    "403 Forbidden",
                    "text/plain; charset=utf-8",
                    b"403\n",
                );
            }
            if !valid_api_headers(&request) {
                return respond(
                    &mut stream,
                    "415 Unsupported Media Type",
                    "text/plain; charset=utf-8",
                    b"415\n",
                );
            }
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
                Ok(body) => {
                    let cookie = format!(
                        "{CAPABILITY_COOKIE}={capability}; HttpOnly; SameSite=Strict; Path=/"
                    );
                    respond_with_headers(
                        &mut stream,
                        "200 OK",
                        media,
                        &body,
                        &[("Set-Cookie", cookie.as_str())],
                    )
                }
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

    fn request(headers: &[(&str, &str)]) -> Request {
        Request {
            method: "POST".to_string(),
            path: "/api".to_string(),
            headers: headers
                .iter()
                .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                .collect(),
            body: b"{}".to_vec(),
        }
    }

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

    #[test]
    fn only_the_bound_loopback_authorities_are_accepted() {
        assert!(valid_loopback_host(
            &request(&[("host", "127.0.0.1:9130")]),
            9130
        ));
        assert!(valid_loopback_host(
            &request(&[("host", "localhost:9130")]),
            9130
        ));
        assert!(!valid_loopback_host(
            &request(&[("host", "attacker.invalid:9130")]),
            9130
        ));
        assert!(!valid_loopback_host(&request(&[]), 9130));
    }

    #[test]
    fn api_requires_json_and_same_origin_when_origin_is_present() {
        assert!(valid_api_headers(&request(&[
            ("host", "127.0.0.1:9130"),
            ("content-type", "application/json; charset=utf-8"),
            ("origin", "http://127.0.0.1:9130"),
        ])));
        assert!(valid_api_headers(&request(&[
            ("host", "localhost:9130"),
            ("content-type", "application/json"),
        ])));
        assert!(!valid_api_headers(&request(&[
            ("host", "127.0.0.1:9130"),
            ("content-type", "text/plain"),
        ])));
        assert!(!valid_api_headers(&request(&[
            ("host", "127.0.0.1:9130"),
            ("content-type", "application/json"),
            ("origin", "https://attacker.invalid"),
        ])));
    }

    #[test]
    fn api_and_event_stream_require_the_session_capability() {
        let accepted = request(&[(
            "cookie",
            "unrelated=1; operator_capability=secret-token; another=2",
        )]);
        assert!(valid_capability(&accepted, "secret-token"));
        assert!(!valid_capability(&accepted, "other-token"));
        assert!(!valid_capability(&request(&[]), "secret-token"));
    }
}
