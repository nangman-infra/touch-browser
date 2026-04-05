use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("repo root should resolve from cli crate")
        .to_path_buf()
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_touch-browser"))
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("touch-browser process should launch")
}

fn temp_missing_session_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "touch-browser-missing-session-{}.json",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be monotonic")
            .as_nanos()
    ))
}

#[test]
fn parse_errors_write_only_to_stderr() {
    let output = run_cli(&["search", "lambda timeout", "--engine", "invalid-engine"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unknown search engine"));
}

#[test]
fn rank_validation_errors_write_only_to_stderr() {
    let output = run_cli(&["search-open-result", "--engine", "google", "--rank", "0"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--rank requires a positive number."));
}

#[test]
fn runtime_errors_write_only_to_stderr() {
    let missing_session = temp_missing_session_path();
    let output = run_cli(&[
        "session-read",
        "--session-file",
        missing_session
            .to_str()
            .expect("missing session path should be valid UTF-8"),
    ]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains(
        missing_session
            .to_str()
            .expect("missing session path should be valid UTF-8")
    ));
}

#[test]
fn successful_json_output_stays_on_stdout() {
    let output = run_cli(&["open", "fixture://research/static-docs/getting-started"]);

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).trim().is_empty());
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"payloadType\": \"snapshot-document\""));
}
