use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::{fs, path::PathBuf};

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
    let capabilities = &initialize["result"]["capabilities"];
    assert_eq!(capabilities["positionEncoding"], "utf-16");
    assert_eq!(
        capabilities["textDocumentSync"],
        json!({"openClose": true, "change": 1, "save": true})
    );
    assert_eq!(capabilities["hoverProvider"], true);
    assert_eq!(capabilities["documentSymbolProvider"], true);
    assert_eq!(capabilities["foldingRangeProvider"], true);
    assert!(capabilities["semanticTokensProvider"].is_object());

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

#[test]
fn publishes_and_clears_versioned_lexical_diagnostics() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stan-language-server"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let uri = "file:///tmp/diagnostic-model.stan";

    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"processId": null, "capabilities": {}}
        })))
        .unwrap();
    stdin.flush().unwrap();
    let _ = read_frame(&mut stdout);
    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })))
        .unwrap();
    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": {"textDocument": {
                "uri": uri, "languageId": "stan", "version": 1,
                "text": "model {\n  😀@\n}"
            }}
        })))
        .unwrap();
    stdin.flush().unwrap();

    let published = read_frame(&mut stdout);
    assert_eq!(published["method"], "textDocument/publishDiagnostics");
    assert_eq!(published["params"]["uri"], uri);
    assert_eq!(published["params"]["version"], 1);
    assert_eq!(
        published["params"]["diagnostics"][0]["code"],
        "lex.invalid-character"
    );
    assert_eq!(published["params"]["diagnostics"][0]["source"], "stan-lsp");
    assert_eq!(
        published["params"]["diagnostics"][0]["range"],
        json!({"start": {"line": 1, "character": 2}, "end": {"line": 1, "character": 4}})
    );

    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": {
                "textDocument": {"uri": uri, "version": 2},
                "contentChanges": [{"text": "model {\n}"}]
            }
        })))
        .unwrap();
    stdin.flush().unwrap();
    let cleared = read_frame(&mut stdout);
    assert_eq!(cleared["params"]["version"], 2);
    assert_eq!(cleared["params"]["diagnostics"], json!([]));

    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": {
                "textDocument": {"uri": uri, "version": 1},
                "contentChanges": [{"text": "model { @ }"}]
            }
        })))
        .unwrap();
    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": {
                "textDocument": {"uri": uri, "version": 3},
                "contentChanges": [{"text": "model {\n  @\n}"}]
            }
        })))
        .unwrap();
    stdin.flush().unwrap();
    let newest = read_frame(&mut stdout);
    assert_eq!(newest["params"]["version"], 3);
    assert_eq!(
        newest["params"]["diagnostics"][0]["code"],
        "lex.invalid-character"
    );

    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "method": "textDocument/didClose",
            "params": {"textDocument": {"uri": uri}}
        })))
        .unwrap();
    stdin.flush().unwrap();
    let closed = read_frame(&mut stdout);
    assert_eq!(closed["params"]["diagnostics"], json!([]));

    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": null
        })))
        .unwrap();
    stdin.flush().unwrap();
    let _ = read_frame(&mut stdout);
    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "method": "exit", "params": null
        })))
        .unwrap();
    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn advertised_editor_features_respond_over_stdio() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stan-language-server"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let uri = "file:///tmp/features-model.stan";
    let source = "parameters { real theta; }\nmodel { theta ~ normal(0, 1); }\n";
    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"processId": null, "capabilities": {}}
        })))
        .unwrap();
    stdin.flush().unwrap();
    let _ = read_frame(&mut stdout);
    stdin
        .write_all(&frame(
            json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        ))
        .unwrap();
    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": {"textDocument": {"uri": uri, "languageId": "stan", "version": 1, "text": source}}
        })))
        .unwrap();
    stdin.flush().unwrap();
    let _diagnostics = read_frame(&mut stdout);

    let requests = [
        (
            2,
            "textDocument/documentSymbol",
            json!({"textDocument": {"uri": uri}}),
        ),
        (
            3,
            "textDocument/foldingRange",
            json!({"textDocument": {"uri": uri}}),
        ),
        (
            4,
            "textDocument/semanticTokens/full",
            json!({"textDocument": {"uri": uri}}),
        ),
        (
            5,
            "textDocument/hover",
            json!({"textDocument": {"uri": uri}, "position": {"line": 1, "character": 18}}),
        ),
        (
            6,
            "textDocument/completion",
            json!({"textDocument": {"uri": uri}, "position": {"line": 1, "character": 22}}),
        ),
        (
            7,
            "textDocument/signatureHelp",
            json!({"textDocument": {"uri": uri}, "position": {"line": 1, "character": 24}}),
        ),
        (
            8,
            "textDocument/definition",
            json!({"textDocument": {"uri": uri}, "position": {"line": 1, "character": 9}}),
        ),
        (
            9,
            "textDocument/references",
            json!({"textDocument": {"uri": uri}, "position": {"line": 1, "character": 9}, "context": {"includeDeclaration": true}}),
        ),
        (
            10,
            "textDocument/documentHighlight",
            json!({"textDocument": {"uri": uri}, "position": {"line": 1, "character": 9}}),
        ),
        (
            11,
            "textDocument/formatting",
            json!({"textDocument": {"uri": uri}, "options": {"tabSize": 2, "insertSpaces": true}}),
        ),
        (
            12,
            "textDocument/rangeFormatting",
            json!({"textDocument": {"uri": uri}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 1, "character": 31}}, "options": {"tabSize": 2, "insertSpaces": true}}),
        ),
        (
            13,
            "textDocument/inlayHint",
            json!({"textDocument": {"uri": uri}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 1, "character": 31}}}),
        ),
        (
            14,
            "textDocument/rename",
            json!({"textDocument": {"uri": uri}, "position": {"line": 1, "character": 9}, "newName": "renamed"}),
        ),
        (
            15,
            "textDocument/codeAction",
            json!({"textDocument": {"uri": uri}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 1, "character": 31}}, "context": {"diagnostics": []}}),
        ),
    ];
    for (id, method, params) in requests {
        stdin
            .write_all(&frame(
                json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
            ))
            .unwrap();
        stdin.flush().unwrap();
        let response = read_frame(&mut stdout);
        assert_eq!(response["id"], id, "{method}: {response}");
        assert!(response.get("error").is_none(), "{method}: {response}");
        assert!(!response["result"].is_null(), "{method}: {response}");
        match method {
            "textDocument/documentSymbol" => {
                let symbols = response["result"].as_array().unwrap();
                let theta = symbols
                    .iter()
                    .find(|symbol| symbol["name"] == "theta")
                    .unwrap();
                assert_eq!(
                    theta["selectionRange"],
                    json!({"start": {"line": 0, "character": 18}, "end": {"line": 0, "character": 23}})
                );
            }
            "textDocument/foldingRange" => {
                assert_eq!(response["result"], json!([]));
            }
            "textDocument/semanticTokens/full" => {
                assert_eq!(response["result"]["resultId"], "1");
                assert!(!response["result"]["data"].as_array().unwrap().is_empty());
            }
            "textDocument/hover" => {
                assert_eq!(
                    response["result"]["range"],
                    json!({"start": {"line": 1, "character": 16}, "end": {"line": 1, "character": 22}})
                );
                assert!(
                    response["result"]["contents"]["value"]
                        .as_str()
                        .unwrap()
                        .contains("normal")
                );
            }
            "textDocument/completion" => {
                let labels = response["result"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|item| item["label"].as_str())
                    .collect::<Vec<_>>();
                assert!(labels.contains(&"normal_lpdf"));
                assert!(labels.contains(&"theta"));
                assert!(!labels.contains(&"get_lp"));
            }
            "textDocument/signatureHelp" => {
                assert_eq!(response["result"]["activeParameter"], 0);
                assert!(
                    response["result"]["signatures"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|signature| signature["label"].as_str().unwrap().contains("normal")),
                    "{response}"
                );
            }
            "textDocument/definition" => assert_eq!(
                response["result"],
                json!({
                    "uri": uri,
                    "range": {"start": {"line": 0, "character": 18}, "end": {"line": 0, "character": 23}}
                })
            ),
            "textDocument/references" => assert_eq!(
                response["result"],
                json!([
                    {"uri": uri, "range": {"start": {"line": 0, "character": 18}, "end": {"line": 0, "character": 23}}},
                    {"uri": uri, "range": {"start": {"line": 1, "character": 8}, "end": {"line": 1, "character": 13}}}
                ])
            ),
            "textDocument/documentHighlight" => assert_eq!(
                response["result"],
                json!([
                    {"range": {"start": {"line": 0, "character": 18}, "end": {"line": 0, "character": 23}}, "kind": 1},
                    {"range": {"start": {"line": 1, "character": 8}, "end": {"line": 1, "character": 13}}, "kind": 1}
                ])
            ),
            "textDocument/formatting" | "textDocument/rangeFormatting" => {
                let edits = response["result"].as_array().unwrap();
                assert_eq!(edits.len(), 1);
                assert_eq!(
                    edits[0]["newText"],
                    "parameters {\n  real theta;\n}\nmodel {\n  theta ~ normal(0, 1);\n}\n"
                );
            }
            "textDocument/inlayHint" | "textDocument/codeAction" => {
                assert_eq!(response["result"], json!([]));
            }
            "textDocument/rename" => assert_eq!(
                response["result"]["changes"][uri],
                json!([
                    {"range": {"start": {"line": 0, "character": 18}, "end": {"line": 0, "character": 23}}, "newText": "renamed"},
                    {"range": {"start": {"line": 1, "character": 8}, "end": {"line": 1, "character": 13}}, "newText": "renamed"}
                ])
            ),
            _ => unreachable!(),
        }
    }

    stdin
        .write_all(&frame(
            json!({"jsonrpc": "2.0", "id": 20, "method": "shutdown", "params": null}),
        ))
        .unwrap();
    stdin.flush().unwrap();
    let _ = read_frame(&mut stdout);
    stdin
        .write_all(&frame(
            json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
        ))
        .unwrap();
    drop(stdin);
    assert!(child.wait().unwrap().success());
}

#[test]
fn workspace_symbols_and_call_hierarchy_have_exact_targets() {
    let root = std::env::temp_dir().join(format!("stan-lsp-protocol-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let path: PathBuf = root.join("functions.stan");
    let source =
        "functions { real helper(real x) { return x; } real outer(real y) { return helper(y); } }";
    fs::write(&path, source).unwrap();
    let uri = format!("file://{}", path.display());
    let root_uri = format!("file://{}", root.display());

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
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "processId": null,
                "capabilities": {},
                "workspaceFolders": [{"uri": root_uri, "name": "fixture"}]
            }
        })))
        .unwrap();
    stdin.flush().unwrap();
    let _ = read_frame(&mut stdout);
    stdin
        .write_all(&frame(
            json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        ))
        .unwrap();
    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": {"textDocument": {"uri": uri, "languageId": "stan", "version": 1, "text": source}}
        })))
        .unwrap();
    stdin.flush().unwrap();
    let _ = read_frame(&mut stdout);

    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "id": 2, "method": "workspace/symbol",
            "params": {"query": "helper"}
        })))
        .unwrap();
    stdin.flush().unwrap();
    let workspace = read_frame(&mut stdout);
    assert_eq!(
        workspace["result"].as_array().unwrap().len(),
        1,
        "{workspace}"
    );
    assert_eq!(workspace["result"][0]["name"], "helper");
    assert_eq!(
        workspace["result"][0]["location"]["range"],
        json!({"start": {"line": 0, "character": 17}, "end": {"line": 0, "character": 23}})
    );

    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "id": 3, "method": "textDocument/prepareCallHierarchy",
            "params": {"textDocument": {"uri": uri}, "position": {"line": 0, "character": 18}}
        })))
        .unwrap();
    stdin.flush().unwrap();
    let helper = read_frame(&mut stdout);
    assert_eq!(helper["result"].as_array().unwrap().len(), 1);
    assert_eq!(helper["result"][0]["name"], "helper");
    let helper_item = helper["result"][0].clone();

    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "id": 4, "method": "callHierarchy/incomingCalls",
            "params": {"item": helper_item}
        })))
        .unwrap();
    stdin.flush().unwrap();
    let incoming = read_frame(&mut stdout);
    assert_eq!(incoming["result"].as_array().unwrap().len(), 1);
    assert_eq!(incoming["result"][0]["from"]["name"], "outer");
    let helper_call = source.rfind("helper").unwrap() as u64;
    assert_eq!(
        incoming["result"][0]["fromRanges"],
        json!([{"start": {"line": 0, "character": helper_call}, "end": {"line": 0, "character": helper_call + 6}}])
    );

    let outer_offset = source.find("outer").unwrap() as u64;
    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "id": 5, "method": "textDocument/prepareCallHierarchy",
            "params": {"textDocument": {"uri": uri}, "position": {"line": 0, "character": outer_offset + 1}}
        })))
        .unwrap();
    stdin.flush().unwrap();
    let outer = read_frame(&mut stdout);
    let outer_item = outer["result"][0].clone();
    stdin
        .write_all(&frame(json!({
            "jsonrpc": "2.0", "id": 6, "method": "callHierarchy/outgoingCalls",
            "params": {"item": outer_item}
        })))
        .unwrap();
    stdin.flush().unwrap();
    let outgoing = read_frame(&mut stdout);
    assert_eq!(outgoing["result"].as_array().unwrap().len(), 1);
    assert_eq!(outgoing["result"][0]["to"]["name"], "helper");

    stdin
        .write_all(&frame(
            json!({"jsonrpc": "2.0", "id": 7, "method": "shutdown", "params": null}),
        ))
        .unwrap();
    stdin.flush().unwrap();
    let _ = read_frame(&mut stdout);
    stdin
        .write_all(&frame(
            json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
        ))
        .unwrap();
    drop(stdin);
    assert!(child.wait().unwrap().success());
    fs::remove_dir_all(root).unwrap();
}
