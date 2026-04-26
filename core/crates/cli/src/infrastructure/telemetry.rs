use std::{env, path::PathBuf};

use serde_json::{json, Value};
use touch_browser_storage_sqlite::{PilotTelemetryEvent, PilotTelemetryStore};
use url::Url;

use crate::interface::{cli_error::CliError, cli_support::data_root};

pub(crate) fn default_telemetry_db_path() -> PathBuf {
    default_telemetry_db_path_from(
        env::var_os("TOUCH_BROWSER_TELEMETRY_DB").map(PathBuf::from),
        data_root(),
    )
}

fn default_telemetry_db_path_from(
    explicit_telemetry_db: Option<PathBuf>,
    resolved_data_root: PathBuf,
) -> PathBuf {
    explicit_telemetry_db.unwrap_or_else(|| resolved_data_root.join("pilot/telemetry.sqlite"))
}

pub(crate) fn telemetry_surface_label(default_surface: &str) -> String {
    env::var("TOUCH_BROWSER_TELEMETRY_SURFACE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_surface.to_string())
}

pub(crate) fn telemetry_store() -> Result<PilotTelemetryStore, CliError> {
    Ok(PilotTelemetryStore::open(default_telemetry_db_path())?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TelemetryMode {
    Off,
    Redacted,
    Full,
}

pub(crate) fn log_telemetry_success(
    surface: &str,
    operation: &str,
    output: &Value,
    params: &Value,
) -> Result<(), CliError> {
    let Some(event) = build_success_telemetry_event(surface, operation, output, params) else {
        return Ok(());
    };
    telemetry_store()?.append(&event)?;
    Ok(())
}

pub(crate) fn log_telemetry_error(
    surface: &str,
    operation: &str,
    error: &str,
    session_id: Option<&str>,
    params: &Value,
) -> Result<(), CliError> {
    let Some(event) = build_error_telemetry_event(surface, operation, error, session_id, params)
    else {
        return Ok(());
    };
    telemetry_store()?.append(&event)?;
    Ok(())
}

fn build_success_telemetry_event(
    surface: &str,
    operation: &str,
    output: &Value,
    params: &Value,
) -> Option<PilotTelemetryEvent> {
    build_success_telemetry_event_for_mode(surface, operation, output, params, telemetry_mode())
}

fn build_error_telemetry_event(
    surface: &str,
    operation: &str,
    error: &str,
    session_id: Option<&str>,
    params: &Value,
) -> Option<PilotTelemetryEvent> {
    build_error_telemetry_event_for_mode(
        surface,
        operation,
        error,
        session_id,
        params,
        telemetry_mode(),
    )
}

fn build_success_telemetry_event_for_mode(
    surface: &str,
    operation: &str,
    output: &Value,
    params: &Value,
    mode: TelemetryMode,
) -> Option<PilotTelemetryEvent> {
    if matches!(mode, TelemetryMode::Off) {
        return None;
    }

    let mut event = PilotTelemetryEvent::now(surface, operation, "succeeded");
    populate_telemetry_event(&mut event, output, params, mode);
    event.payload = compact_telemetry_payload(output, params, mode);
    Some(event)
}

fn build_error_telemetry_event_for_mode(
    surface: &str,
    operation: &str,
    error: &str,
    session_id: Option<&str>,
    params: &Value,
    mode: TelemetryMode,
) -> Option<PilotTelemetryEvent> {
    if matches!(mode, TelemetryMode::Off) {
        return None;
    }

    let mut event = PilotTelemetryEvent::now(surface, operation, "failed");
    event.note = Some(redact_note(error, mode));
    event.session_id = session_id.map(ToString::to_string);
    if matches!(mode, TelemetryMode::Full) {
        if let Some(session_id) = session_id {
            event.payload = Some(json!({
                "sessionId": session_id,
            }));
        } else {
            event.payload = compact_telemetry_payload(&Value::Null, params, mode);
        }
    }
    Some(event)
}

fn telemetry_mode() -> TelemetryMode {
    telemetry_mode_from_env(env::var("TOUCH_BROWSER_TELEMETRY_MODE").ok())
}

fn telemetry_mode_from_env(raw_value: Option<String>) -> TelemetryMode {
    match raw_value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("redacted")
    {
        "off" => TelemetryMode::Off,
        "full" => TelemetryMode::Full,
        _ => TelemetryMode::Redacted,
    }
}

fn populate_telemetry_event(
    event: &mut PilotTelemetryEvent,
    output: &Value,
    params: &Value,
    mode: TelemetryMode,
) {
    event.session_id = telemetry_string(output, &["sessionState", "sessionId"])
        .or_else(|| telemetry_string(output, &["result", "sessionState", "sessionId"]))
        .or_else(|| telemetry_string(output, &["sessionId"]))
        .or_else(|| telemetry_string(params, &["sessionId"]));
    event.tab_id =
        telemetry_string(output, &["tabId"]).or_else(|| telemetry_string(params, &["tabId"]));
    event.current_url = telemetry_string(output, &["sessionState", "currentUrl"])
        .or_else(|| telemetry_string(output, &["result", "sessionState", "currentUrl"]))
        .or_else(|| telemetry_string(output, &["action", "output", "source", "sourceUrl"]))
        .or_else(|| telemetry_string(output, &["output", "source", "sourceUrl"]))
        .or_else(|| telemetry_string(params, &["target"]))
        .and_then(|url| sanitize_telemetry_url(&url, mode));
    event.policy_profile = telemetry_string(output, &["sessionState", "policyProfile"])
        .or_else(|| telemetry_string(output, &["result", "sessionState", "policyProfile"]))
        .or_else(|| telemetry_string(output, &["policyProfile"]))
        .or_else(|| telemetry_string(output, &["checkpoint", "activePolicyProfile"]))
        .or_else(|| telemetry_string(output, &["result", "checkpoint", "activePolicyProfile"]));
    event.policy_decision = telemetry_string(output, &["policy", "decision"])
        .or_else(|| telemetry_string(output, &["result", "policy", "decision"]))
        .or_else(|| telemetry_string(output, &["action", "policy", "decision"]))
        .or_else(|| telemetry_string(output, &["checkpoint", "approvalPanel", "severity"]))
        .or_else(|| {
            telemetry_string(
                output,
                &["result", "checkpoint", "approvalPanel", "severity"],
            )
        });
    event.risk_class = telemetry_string(output, &["policy", "riskClass"])
        .or_else(|| telemetry_string(output, &["result", "policy", "riskClass"]))
        .or_else(|| telemetry_string(output, &["action", "policy", "riskClass"]));
    event.provider_hints = telemetry_string_array(output, &["checkpoint", "providerHints"]);
    if event.provider_hints.is_empty() {
        event.provider_hints =
            telemetry_string_array(output, &["result", "checkpoint", "providerHints"]);
    }
    event.approved_risks = telemetry_string_array(output, &["approvedRisks"]);
    if event.approved_risks.is_empty() {
        event.approved_risks = telemetry_string_array(output, &["result", "approvedRisks"]);
    }
    if event.approved_risks.is_empty() {
        event.approved_risks = telemetry_string_array(output, &["checkpoint", "approvedRisks"]);
    }
    if event.approved_risks.is_empty() {
        event.approved_risks =
            telemetry_string_array(output, &["result", "checkpoint", "approvedRisks"]);
    }
}

fn compact_telemetry_payload(output: &Value, params: &Value, mode: TelemetryMode) -> Option<Value> {
    match mode {
        TelemetryMode::Off | TelemetryMode::Redacted => None,
        TelemetryMode::Full => Some(json!({
            "params": compact_value_summary(params),
            "result": compact_value_summary(output),
        })),
    }
}

fn compact_value_summary(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut compact = serde_json::Map::new();
            for key in [
                "status",
                "action",
                "sessionId",
                "tabId",
                "sessionState",
                "policy",
                "policyProfile",
                "approvedRisks",
                "checkpoint",
                "target",
                "claims",
            ] {
                if let Some(entry) = map.get(key) {
                    compact.insert(key.to_string(), compact_value_summary(entry));
                }
            }
            Value::Object(compact)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .take(6)
                .map(compact_value_summary)
                .collect::<Vec<_>>(),
        ),
        Value::String(text) => {
            if text.len() > 180 {
                Value::String(format!("{}...", &text[..180]))
            } else {
                Value::String(text.clone())
            }
        }
        _ => value.clone(),
    }
}

fn telemetry_string(value: &Value, path: &[&str]) -> Option<String> {
    telemetry_value(value, path)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn telemetry_string_array(value: &Value, path: &[&str]) -> Vec<String> {
    telemetry_value(value, path)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn telemetry_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn sanitize_telemetry_url(url: &str, mode: TelemetryMode) -> Option<String> {
    match mode {
        TelemetryMode::Off | TelemetryMode::Redacted => sanitize_telemetry_url_redacted(url),
        TelemetryMode::Full => Some(url.to_string()),
    }
}

fn sanitize_telemetry_url_redacted(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    Some(format!("{}://{}", parsed.scheme(), host))
}

fn redact_note(note: &str, mode: TelemetryMode) -> String {
    if matches!(mode, TelemetryMode::Full) {
        return note.to_string();
    }

    note.split_whitespace()
        .map(redact_note_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_note_token(token: &str) -> String {
    let trimmed = token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
        )
    });
    if sanitize_telemetry_url_redacted(trimmed).is_some() {
        token.replace(trimmed, "<redacted-url>")
    } else {
        token.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::{json, Value};

    use super::{
        build_error_telemetry_event_for_mode, build_success_telemetry_event_for_mode,
        default_telemetry_db_path_from, redact_note, telemetry_mode_from_env, TelemetryMode,
    };

    fn temporary_directory(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("touch-browser-{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("temporary directory should exist");
        path
    }

    #[test]
    fn telemetry_db_path_respects_explicit_data_root() {
        let data_root = temporary_directory("telemetry-data-root");
        let canonical_data_root = data_root.canonicalize().unwrap_or(data_root.clone());

        assert_eq!(
            default_telemetry_db_path_from(None, canonical_data_root.clone()),
            canonical_data_root.join("pilot/telemetry.sqlite")
        );
    }

    #[test]
    fn telemetry_mode_defaults_to_redacted() {
        assert_eq!(telemetry_mode_from_env(None), TelemetryMode::Redacted);
        assert_eq!(
            telemetry_mode_from_env(Some(String::new())),
            TelemetryMode::Redacted
        );
        assert_eq!(
            telemetry_mode_from_env(Some("full".to_string())),
            TelemetryMode::Full
        );
        assert_eq!(
            telemetry_mode_from_env(Some("off".to_string())),
            TelemetryMode::Off
        );
    }

    #[test]
    fn redacted_success_telemetry_drops_payload_claims_and_full_url() {
        let output = json!({
            "sessionState": {
                "sessionId": "s123",
                "currentUrl": "https://customer.example.internal/orders/42?token=secret"
            },
            "policy": {
                "decision": "review",
                "riskClass": "high"
            }
        });
        let params = json!({
            "target": "https://customer.example.internal/orders/42?token=secret",
            "claims": ["Customer account 42 is delinquent."]
        });

        let event = build_success_telemetry_event_for_mode(
            "cli",
            "extract",
            &output,
            &params,
            TelemetryMode::Redacted,
        )
        .expect("redacted telemetry should still record summary event");

        assert_eq!(event.session_id.as_deref(), Some("s123"));
        assert_eq!(
            event.current_url.as_deref(),
            Some("https://customer.example.internal")
        );
        assert_eq!(event.policy_decision.as_deref(), Some("review"));
        assert!(event.payload.is_none());
    }

    #[test]
    fn full_success_telemetry_preserves_payload() {
        let output = json!({
            "sessionState": {
                "sessionId": "s123",
                "currentUrl": "https://example.com/docs?q=1"
            }
        });
        let params = json!({
            "target": "https://example.com/docs?q=1",
            "claims": ["Docs exist."]
        });

        let event = build_success_telemetry_event_for_mode(
            "cli",
            "extract",
            &output,
            &params,
            TelemetryMode::Full,
        )
        .expect("full telemetry should record event");

        assert_eq!(
            event.current_url.as_deref(),
            Some("https://example.com/docs?q=1")
        );
        assert_eq!(
            event
                .payload
                .as_ref()
                .and_then(|payload| payload.pointer("/params/claims/0"))
                .and_then(Value::as_str),
            Some("Docs exist.")
        );
    }

    #[test]
    fn off_mode_disables_telemetry_events() {
        assert!(build_success_telemetry_event_for_mode(
            "cli",
            "open",
            &json!({}),
            &json!({}),
            TelemetryMode::Off,
        )
        .is_none());
        assert!(build_error_telemetry_event_for_mode(
            "cli",
            "open",
            "failed",
            None,
            &json!({}),
            TelemetryMode::Off,
        )
        .is_none());
    }

    #[test]
    fn redacted_error_notes_scrub_urls() {
        let note = redact_note(
            "follow target https://customer.example.internal/orders/42?token=secret failed",
            TelemetryMode::Redacted,
        );
        assert!(!note.contains("customer.example.internal/orders/42"));
        assert!(note.contains("<redacted-url>"));
    }
}
