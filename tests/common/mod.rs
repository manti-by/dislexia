//! A hand-written mock Ollama server: `TcpListener` on an ephemeral port,
//! hand-rolled HTTP/1.1 responses, no extra dependency (`wiki/pages/2026-10-26-09-testing.md`
//! §2). Routes are selected by markers in the request body.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

pub struct MockServer {
    pub addr: String,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for MockServer {
    fn drop(&mut self) {
        // The listener thread blocks on accept forever; the test process
        // tears it down with itself, so never join it.
        drop(self.handle.take());
    }
}

pub fn spawn() -> MockServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("local addr").to_string();
    let handle = thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            handle_connection(stream);
        }
    });
    MockServer {
        addr,
        handle: Some(handle),
    }
}

fn handle_connection(stream: TcpStream) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(stream) => stream,
        Err(_) => return,
    });
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();

    // Headers until the blank line; track content-length.
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 && reader.read_exact(&mut body).is_err() {
        return;
    }
    let body = String::from_utf8_lossy(&body).to_string();

    let mut stream = stream;
    if path.contains("/api/tags") {
        respond(
            &mut stream,
            200,
            "application/json",
            br#"{"models":[{"name":"llama3.2:latest","model":"llama3.2:latest"},{"name":"qwen3:8b","model":"qwen3:8b"}]}"#,
        );
    } else if path.contains("/api/chat") {
        handle_chat(&mut stream, &body);
    } else {
        respond(
            &mut stream,
            404,
            "application/json",
            br#"{"error":"unknown route"}"#,
        );
    }
}

fn handle_chat(stream: &mut TcpStream, body: &str) {
    if body.contains("unknown-model") {
        respond(
            stream,
            404,
            "application/json",
            br#"{"error":"model 'unknown-model' not found, try pulling it first"}"#,
        );
    } else if body.contains("server-error-please") {
        respond(
            stream,
            500,
            "application/json",
            br#"{"error":"model runner has unexpectedly stopped"}"#,
        );
    } else if body.contains("mid-stream-error-please") {
        // 200 OK, then an error object mid-stream (`plan 04 §4`).
        write_headers(stream, 200, "application/x-ndjson");
        let _ = writeln!(
            stream,
            r#"{{"message":{{"role":"assistant","content":"partial "}},"done":false}}"#
        );
        let _ = stream.flush();
        let _ = writeln!(stream, r#"{{"error":"model requires more system memory"}}"#);
        let _ = stream.flush();
    } else if body.contains("drop-please") {
        // Headers, one truncated JSON line (no newline), then an abrupt
        // socket close mid-line — the reader must not mistake this for the
        // end of the stream.
        write_headers(stream, 200, "application/x-ndjson");
        let _ = write!(
            stream,
            r#"{{"message":{{"role":"assistant","content":"partial "}},"done":fal"#
        );
        let _ = stream.flush();
    } else if body.contains("slow-stream-please") {
        // Several lines with pauses — used for the cancellation test.
        write_headers(stream, 200, "application/x-ndjson");
        for index in 0..100 {
            let _ = writeln!(
                stream,
                r#"{{"message":{{"role":"assistant","content":"word{index} "}},"done":false}}"#
            );
            let _ = stream.flush();
            thread::sleep(std::time::Duration::from_millis(10));
        }
    } else {
        // Two deltas then a done line with stats.
        write_headers(stream, 200, "application/x-ndjson");
        let _ = writeln!(
            stream,
            r#"{{"message":{{"role":"assistant","content":"use "}},"done":false}}"#
        );
        let _ = stream.flush();
        thread::sleep(std::time::Duration::from_millis(10));
        let _ = writeln!(
            stream,
            r#"{{"message":{{"role":"assistant","content":"v.reverse();"}},"done":false}}"#
        );
        let _ = stream.flush();
        thread::sleep(std::time::Duration::from_millis(10));
        let _ = writeln!(
            stream,
            r#"{{"message":{{"role":"assistant","content":""}},"done":true,"total_duration":123,"eval_count":2,"eval_duration":456,"prompt_eval_count":7}}"#
        );
        let _ = stream.flush();
    }
}

fn write_headers(stream: &mut TcpStream, status: u16, content_type: &str) {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n"
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.flush();
}

fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        match status {
            200 => "OK",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "OK",
        },
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}
