use std::{
    io::Write,
    process::{Command, Stdio},
};

fn run_with_stdin(binary: &str, args: &[&str], source: &str) -> std::process::Output {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn stanfmt_formats_stdin_and_check_reports_drift() {
    let binary = env!("CARGO_BIN_EXE_stanfmt");
    let formatted = run_with_stdin(binary, &[], "model{real x;}");
    assert!(formatted.status.success());
    assert_eq!(
        String::from_utf8(formatted.stdout).unwrap(),
        "model {\n  real x;\n}\n"
    );
    let check = run_with_stdin(binary, &["--check"], "model{real x;}");
    assert_eq!(check.status.code(), Some(1));
}

#[test]
fn stanlint_json_is_machine_readable_and_uses_ci_exit_codes() {
    let output = run_with_stdin(
        env!("CARGO_BIN_EXE_stanlint"),
        &["--json"],
        "model { missing = 1; }",
    );
    assert_eq!(output.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value[0]["code"], "correctness.unresolved-identifier");
}
