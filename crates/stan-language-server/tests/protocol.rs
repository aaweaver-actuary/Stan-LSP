use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

fn frame(message: Value) -> Vec<u8> {
    let body = serde_json::to_vec(&message).unwrap();
    let mut framed = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    framed.extend(body);
    framed
}

fn read_frame(reader: &mut impl BufRead) -> Value {
    let mut length = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            length = Some(value.trim().parse::<usize>().unwrap());
        }
    }
    let mut body = vec![0; length.expect("response has Content-Length")];
    reader.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[test]
fn initializes_and_shuts_down_over_stdio() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stan-language-server"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"processId": null, "capabilities": {}}
        })))
        .unwrap();
    stdin.flush().unwrap();
    let initialize = read_frame(&mut stdout);
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "stan-language-server"
    );
    assert_eq!(initialize["result"]["capabilities"], json!({}));

    stdin
        .write_all(&frame(
            json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        ))
        .unwrap();
    stdin
        .write_all(&frame(
            json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": null}),
        ))
        .unwrap();
    stdin.flush().unwrap();
    let shutdown = read_frame(&mut stdout);
    assert!(shutdown["result"].is_null());

    stdin
        .write_all(&frame(
            json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
        ))
        .unwrap();
    drop(stdin);

    let mut trailing_stdout = Vec::new();
    stdout.read_to_end(&mut trailing_stdout).unwrap();
    assert!(trailing_stdout.is_empty(), "unexpected stdout after exit");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    let status = child.wait().unwrap();
    assert!(status.success(), "server failed: {stderr}");
}
