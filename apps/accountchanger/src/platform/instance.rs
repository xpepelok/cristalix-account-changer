use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use futures::channel::mpsc::UnboundedSender;

pub const DEFAULT_PORT: u16 = 47821;
const PORT_ENV: &str = "AC_INSTANCE_PORT";
const UPDATED_ATTEMPTS: usize = 20;
const RETRY_DELAY: Duration = Duration::from_millis(300);
const FOCUS_PATH: &str = "/api/focus";

pub fn port() -> u16 {
    std::env::var(PORT_ENV)
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

fn address(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

pub fn acquire(port: u16, updated: bool) -> Option<TcpListener> {
    let attempts = if updated { UPDATED_ATTEMPTS } else { 1 };
    for attempt in 0..attempts {
        if let Ok(listener) = TcpListener::bind(address(port)) {
            return Some(listener);
        }
        if attempt + 1 < attempts {
            thread::sleep(RETRY_DELAY);
        }
    }
    None
}

pub fn request_focus(port: u16) {
    let Ok(mut stream) = TcpStream::connect_timeout(&address(port), Duration::from_secs(2)) else {
        return;
    };
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    let request = format!(
        "POST {FOCUS_PATH} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(request.as_bytes()).is_ok() {
        let mut sink = Vec::new();
        stream.read_to_end(&mut sink).ok();
    }
}

fn is_focus_request(head: &str) -> bool {
    let mut parts = head.split_whitespace();
    matches!(parts.next(), Some("POST" | "GET")) && parts.next() == Some(FOCUS_PATH)
}

fn respond(mut stream: TcpStream, focus: &UnboundedSender<()>) {
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    let mut buffer = [0u8; 1024];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let head = String::from_utf8_lossy(&buffer[..read]);
    let first = head.lines().next().unwrap_or_default();
    let (status, body) = if is_focus_request(first) {
        focus.unbounded_send(()).ok();
        ("200 OK", "{\"ok\":true}\n")
    } else {
        ("404 Not Found", "{\"error\":\"not found\"}\n")
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).ok();
}

pub fn serve(listener: TcpListener, focus: UnboundedSender<()>) {
    thread::Builder::new()
        .name("single-instance".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                respond(stream, &focus);
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use futures::channel::mpsc::unbounded;

    #[test]
    fn focus_paths_are_recognised() {
        assert!(is_focus_request("POST /api/focus HTTP/1.1"));
        assert!(is_focus_request("GET /api/focus HTTP/1.1"));
        assert!(!is_focus_request("POST /api/quit HTTP/1.1"));
        assert!(!is_focus_request(""));
    }

    #[test]
    fn second_instance_reaches_first() {
        let listener = TcpListener::bind(address(0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (sender, mut receiver) = unbounded();
        serve(listener, sender);
        assert!(acquire(port, false).is_none());
        request_focus(port);
        assert_eq!(futures::executor::block_on(receiver.next()), Some(()));
    }
}
