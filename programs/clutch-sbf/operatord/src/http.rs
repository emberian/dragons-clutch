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
use std::time::Duration;
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
const MAX_REQUEST_BODY_BYTES: usize = 512 * 1024;
const MAX_REQUEST_LINE_BYTES: usize = 8 * 1024;
const MAX_HEADER_LINE_BYTES: usize = 8 * 1024;
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_HEADER_COUNT: usize = 64;
const CONNECTION_IO_TIMEOUT: Duration = Duration::from_secs(10);

/// What a POST to `/api` is answered by.
pub type Action = Arc<dyn Fn(&Value) -> Value + Send + Sync>;

#[derive(Clone, Debug, PartialEq)]
pub struct JsonReadResponse {
    pub status: u16,
    pub body: Value,
}

/// Optional read-only route owner. Returning `None` delegates to the static
/// bench. It cannot receive a request body or the mutating `/api` capability.
pub type ReadApi = Arc<dyn Fn(&str, &str) -> Option<JsonReadResponse> + Send + Sync>;

/// Optional pure POST route owner. It receives bounded bytes only after the
/// loopback Host, same-origin/CLI Origin, and JSON content type have passed.
/// Returning `None` means that this owner does not recognize the path.
pub type PostApi = Arc<dyn Fn(&str, &[u8]) -> Option<JsonReadResponse> + Send + Sync>;

pub struct Server {
    listener: TcpListener,
    bus: Arc<Bus>,
    root: PathBuf,
    action: Option<Action>,
    read_api: Option<ReadApi>,
    post_api: Option<PostApi>,
    capability: Option<String>,
}

impl Server {
    /// Bind the bench to a loopback port.
    pub fn bind(port: u16, bus: Arc<Bus>, root: PathBuf, action: Action) -> Result<Self> {
        Self::bind_with_read_api(port, bus, root, action, None)
    }

    /// Bind with an additional GET-only JSON projection surface.
    pub fn bind_with_read_api(
        port: u16,
        bus: Arc<Bus>,
        root: PathBuf,
        action: Action,
        read_api: Option<ReadApi>,
    ) -> Result<Self> {
        Self::bind_inner(port, bus, root, Some(action), read_api, None)
    }

    /// Bind a static, read-only projection plus pure JSON compiler surface.
    ///
    /// Unlike the Operator Bench server this mode has no capability cookie,
    /// event stream, or `/api` action owner. A POST callback can therefore
    /// compute a response but cannot reach the mutation/session surface.
    pub fn bind_pure(
        port: u16,
        bus: Arc<Bus>,
        root: PathBuf,
        read_api: Option<ReadApi>,
        post_api: Option<PostApi>,
    ) -> Result<Self> {
        Self::bind_inner(port, bus, root, None, read_api, post_api)
    }

    fn bind_inner(
        port: u16,
        bus: Arc<Bus>,
        root: PathBuf,
        action: Option<Action>,
        read_api: Option<ReadApi>,
        post_api: Option<PostApi>,
    ) -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let capability = action.as_ref().map(|_| Keypair::new().pubkey().to_string());
        Ok(Self {
            listener,
            bus,
            root,
            action,
            read_api,
            post_api,
            capability,
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
            let action = self.action.clone();
            let read_api = self.read_api.clone();
            let post_api = self.post_api.clone();
            let capability = self.capability.clone();
            thread::spawn(move || {
                if stream
                    .set_read_timeout(Some(CONNECTION_IO_TIMEOUT))
                    .and_then(|()| stream.set_write_timeout(Some(CONNECTION_IO_TIMEOUT)))
                    .is_err()
                {
                    return;
                }
                let _ignored = handle(
                    stream,
                    &bus,
                    &root,
                    action.as_deref(),
                    read_api.as_deref(),
                    post_api.as_deref(),
                    capability.as_deref(),
                );
            });
        }
    }
}

struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    body_too_large: bool,
    has_content_length: bool,
}

fn bounded_line<R: BufRead>(reader: &mut R, maximum: usize) -> Result<Option<String>> {
    let mut bytes = Vec::new();
    let count = reader
        .take(u64::try_from(maximum)?.saturating_add(1))
        .read_until(b'\n', &mut bytes)?;
    if count == 0 {
        return Ok(None);
    }
    if bytes.len() > maximum {
        return Err("request line exceeds its fixed bound".into());
    }
    Ok(Some(String::from_utf8(bytes)?))
}

fn read_request(stream: &TcpStream) -> Result<Request> {
    let mut reader = BufReader::new(stream);
    let line = bounded_line(&mut reader, MAX_REQUEST_LINE_BYTES)?.unwrap_or_default();
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let mut headers: HashMap<String, String> = HashMap::new();
    let mut header_count = 0_usize;
    let mut aggregate_header_bytes = 0_usize;
    loop {
        let Some(header) = bounded_line(&mut reader, MAX_HEADER_LINE_BYTES)? else {
            break;
        };
        header_count = header_count.saturating_add(1);
        aggregate_header_bytes = aggregate_header_bytes.saturating_add(header.len());
        if header_count > MAX_HEADER_COUNT
            || header.len() > MAX_HEADER_LINE_BYTES
            || aggregate_header_bytes > MAX_HEADER_BYTES
        {
            return Err("request headers exceed the fixed bound".into());
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            if name.is_empty() || headers.insert(name, value.trim().to_string()).is_some() {
                return Err("duplicate or malformed request header".into());
            }
        } else {
            return Err("malformed request header".into());
        }
    }
    if headers.contains_key("transfer-encoding") {
        return Err("transfer-encoding is unsupported".into());
    }
    let has_content_length = headers.contains_key("content-length");
    let length = match headers.get("content-length") {
        Some(value) => value
            .parse::<usize>()
            .map_err(|_| "invalid content-length")?,
        None => 0,
    };
    let body_too_large = length > MAX_REQUEST_BODY_BYTES;
    let mut body = if body_too_large {
        Vec::new()
    } else {
        vec![0_u8; length]
    };
    if !body.is_empty() {
        reader.read_exact(&mut body)?;
    }
    Ok(Request {
        method,
        path,
        headers,
        body,
        body_too_large,
        has_content_length,
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

fn json_status(status: u16) -> &'static str {
    match status {
        200 => "200 OK",
        400 => "400 Bad Request",
        404 => "404 Not Found",
        405 => "405 Method Not Allowed",
        409 => "409 Conflict",
        411 => "411 Length Required",
        413 => "413 Content Too Large",
        415 => "415 Unsupported Media Type",
        422 => "422 Unprocessable Content",
        503 => "503 Service Unavailable",
        _ => "500 Internal Server Error",
    }
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
    action: Option<&(dyn Fn(&Value) -> Value + Send + Sync)>,
    read_api: Option<&(dyn Fn(&str, &str) -> Option<JsonReadResponse> + Send + Sync)>,
    post_api: Option<&(dyn Fn(&str, &[u8]) -> Option<JsonReadResponse> + Send + Sync)>,
    capability: Option<&str>,
) -> Result<()> {
    let port = stream.local_addr()?.port();
    let request = match read_request(&stream) {
        Ok(request) => request,
        Err(_) => {
            return respond(
                &mut stream,
                "400 Bad Request",
                "text/plain; charset=utf-8",
                b"400\n",
            )
        }
    };
    if !valid_loopback_host(&request, port) {
        return respond(
            &mut stream,
            "403 Forbidden",
            "text/plain; charset=utf-8",
            b"403\n",
        );
    }
    if request.body_too_large {
        return respond(
            &mut stream,
            json_status(413),
            "application/json",
            json!({"error":"request body exceeds 524288 bytes"})
                .to_string()
                .as_bytes(),
        );
    }
    if request.path.starts_with("/v1/") {
        if request.method == "POST" {
            if !request.has_content_length {
                return respond(
                    &mut stream,
                    json_status(411),
                    "application/json",
                    json!({"error":"POST requires one explicit Content-Length"})
                        .to_string()
                        .as_bytes(),
                );
            }
            if !valid_api_headers(&request) {
                return respond(
                    &mut stream,
                    json_status(415),
                    "application/json",
                    json!({"error":"POST requires application/json and an absent or exact same-origin Origin"})
                        .to_string()
                        .as_bytes(),
                );
            }
            let Some(post_api) = post_api else {
                return not_found(&mut stream);
            };
            let Some(reply) = post_api(&request.path, &request.body) else {
                return not_found(&mut stream);
            };
            return respond(
                &mut stream,
                json_status(reply.status),
                "application/json",
                reply.body.to_string().as_bytes(),
            );
        }
        let Some(read_api) = read_api else {
            return not_found(&mut stream);
        };
        let Some(reply) = read_api(&request.method, &request.path) else {
            return not_found(&mut stream);
        };
        return respond(
            &mut stream,
            json_status(reply.status),
            "application/json",
            reply.body.to_string().as_bytes(),
        );
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/events") => {
            let Some(capability) = capability else {
                return not_found(&mut stream);
            };
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
            let (Some(action), Some(capability)) = (action, capability) else {
                return not_found(&mut stream);
            };
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
                    if let Some(capability) = capability {
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
                    } else {
                        respond(&mut stream, "200 OK", media, &body)
                    }
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
            body_too_large: false,
            has_content_length: true,
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
