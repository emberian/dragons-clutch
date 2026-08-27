//! The event log the browser reads, and nothing else.
//!
//! The page has no RPC client and no builder; the *only* thing it learns is
//! what is published here.  The log is append-only and replayed in full to
//! every subscriber, so a browser that connects half way through the walk
//! sees the same forty-four rows as one that was open from the start, and a
//! reload never loses evidence.

use serde_json::Value;
use std::io::Write;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

/// One append-only event log with live server-sent-event subscribers.
#[derive(Default)]
pub struct Bus {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    log: Vec<String>,
    clients: Vec<TcpStream>,
}

fn frame(payload: &str) -> String {
    format!("data: {payload}\n\n")
}

impl Bus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Append one event and push it to every live subscriber.
    ///
    /// A subscriber whose socket has gone away is dropped rather than
    /// retried: the log keeps the evidence, so a reconnect replays it.
    pub fn publish(&self, event: &Value) {
        let payload = event.to_string();
        let text = frame(&payload);
        let mut inner = self.lock();
        inner.log.push(payload);
        inner
            .clients
            .retain_mut(|client| client.write_all(text.as_bytes()).and_then(|()| client.flush()).is_ok());
    }

    /// Attach one subscriber, replaying the whole log first.
    pub fn subscribe(&self, mut client: TcpStream) {
        let mut inner = self.lock();
        let mut backlog = String::new();
        for payload in &inner.log {
            backlog.push_str(&frame(payload));
        }
        if client
            .write_all(backlog.as_bytes())
            .and_then(|()| client.flush())
            .is_ok()
        {
            inner.clients.push(client);
        }
    }

    /// How many events have been published so far.
    pub fn len(&self) -> usize {
        self.lock().log.len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
