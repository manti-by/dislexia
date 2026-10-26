//! Integration tests: `OllamaClient` against the hand-written mock Ollama
//! server (`wiki/pages/2026-10-26-09-testing.md` §2). No real network — loopback only.

use std::time::Duration;

use dislexia::ollama::{
    CancelFlag, ChatRequest, OllamaClient, OllamaError, Options, user_error_message,
};

mod common;
use common::spawn;

fn client(addr: &str) -> OllamaClient {
    // Tighter connect timeout than production so refused-connection tests
    // stay fast; the read timeout stays bigger than the mock's sleeps.
    OllamaClient::new(&format!("http://{addr}")).expect("client")
}

fn simple_request(model: &str, prompt: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_owned(),
        messages: vec![dislexia::ollama::ApiMessage {
            role: "user".to_owned(),
            content: prompt.to_owned(),
        }],
        stream: true,
        options: Some(Options::default()),
    }
}

#[test]
fn list_models_parses_names() {
    let server = spawn();
    let models = client(&server.addr).list_models().expect("list models");
    assert_eq!(models, vec!["llama3.2:latest", "qwen3:8b"]);
}

#[test]
fn test_connection_counts_models() {
    let server = spawn();
    assert_eq!(client(&server.addr).test_connection().unwrap(), 2);
}

#[test]
fn chat_yields_deltas_then_done_with_stats() {
    let server = spawn();
    let mut stream = client(&server.addr)
        .chat(&simple_request("llama3.2", "hello"), &CancelFlag::new())
        .expect("stream");

    let mut text = String::new();
    let mut done_stats = None;
    for _ in 0..3 {
        let chunk = stream.next().expect("chunk").expect("ok");
        text.push_str(chunk.text());
        if let Some(stats) = chunk.done_stats() {
            done_stats = Some(*stats);
        }
    }
    assert_eq!(text, "use v.reverse();");
    let stats = done_stats.expect("done chunk carried stats");
    assert_eq!(stats.eval_count, Some(2));
    assert_eq!(stats.prompt_eval_count, Some(7));
}

#[test]
fn mid_stream_error_maps_to_api_error() {
    let server = spawn();
    let mut stream = client(&server.addr)
        .chat(
            &simple_request("llama3.2", "mid-stream-error-please"),
            &CancelFlag::new(),
        )
        .expect("stream");

    // First chunk arrives, then the error line.
    assert!(stream.next().unwrap().is_ok());
    match stream.next() {
        Some(Err(OllamaError::Api(message))) => {
            assert_eq!(message, "model requires more system memory")
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[test]
fn model_not_found_maps_to_wording() {
    let server = spawn();
    let error = client(&server.addr)
        .chat(
            &simple_request("unknown-model", "hello"),
            &CancelFlag::new(),
        )
        .expect_err("404");
    match error {
        OllamaError::Http { status: 404, .. } => {
            assert_eq!(
                user_error_message(
                    &OllamaError::Http {
                        status: 404,
                        body: "model 'unknown-model' not found, try pulling it first".into(),
                    },
                    &server.addr,
                    "unknown-model"
                ),
                "Model \"unknown-model\" not found. Open Settings → Model."
            );
        }
        other => panic!("expected 404, got {other:?}"),
    }
}

#[test]
fn server_error_500_maps_to_wording() {
    let server = spawn();
    let error = client(&server.addr)
        .chat(
            &simple_request("m", "server-error-please"),
            &CancelFlag::new(),
        )
        .expect_err("500");
    assert!(matches!(error, OllamaError::Http { status: 500, .. }));
}

#[test]
fn abrupt_socket_close_is_an_error_not_a_hang() {
    let server = spawn();
    let started = std::time::Instant::now();
    let mut stream = client(&server.addr)
        .chat(&simple_request("m", "drop-please"), &CancelFlag::new())
        .expect("stream");

    // The truncated final line parses as garbage → the stream must end in
    // an error (Parse), quickly, rather than hanging.
    let mut saw_error = false;
    for item in stream.by_ref() {
        if let Err(error) = item {
            assert!(matches!(error, OllamaError::Parse(_)), "{error:?}");
            saw_error = true;
            break;
        }
    }
    assert!(saw_error, "expected the stream to end with an error");
    assert!(started.elapsed() < Duration::from_secs(10), "no hang");
}

#[test]
fn cancelled_stream_stops_between_lines() {
    let server = spawn();
    let cancel = CancelFlag::new();
    let mut stream = client(&server.addr)
        .chat(&simple_request("m", "slow-stream-please"), &cancel)
        .expect("stream");

    // Consume one chunk, then cancel: the very next item is `Cancelled`.
    assert!(stream.next().unwrap().is_ok());
    cancel.cancel();
    match stream.next() {
        Some(Err(error)) => assert!(error.is_cancelled()),
        other => panic!("expected Cancelled, got {other:?}"),
    }
}

#[test]
fn refused_connection_maps_to_connection_wording() {
    // Port 1 on loopback is reliably closed.
    let error = client("127.0.0.1:1").list_models().unwrap_err();
    eprintln!("debug refused error: {error:?}");
    assert!(matches!(error, OllamaError::Connection(_)));
    assert_eq!(
        user_error_message(&error, "http://127.0.0.1:1", "m"),
        "Can't reach Ollama at http://127.0.0.1:1 — is \"ollama serve\" running?"
    );
}
