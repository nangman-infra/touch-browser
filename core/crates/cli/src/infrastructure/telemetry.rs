use std::{env, path::PathBuf};

use serde_json::{json, Value};
use touch_browser_storage_sqlite::{PilotTelemetryEvent, PilotTelemetryStore};

use crate::*;

pub(crate) fn default_telemetry_db_path() -> PathBuf {
    env::var_os("TOUCH_BROWSER_TELEMETRY_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("output/pilot/telemetry.sqlite"))
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

pub(crate) fn log_telemetry_success(
    surface: &str,
    operation: &str,
    output: &Value,
    params: &Value,
) -> Result<(), CliError> {
    let mut event = PilotTelemetryEvent::now(surface, operation, "succeeded");
    populate_telemetry_event(&mut event, output, params);
    event.payload = Some(compact_telemetry_payload(output, params));
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
    let mut event = PilotTelemetryEvent::now(surface, operation, "failed");
    event.note = Some(error.to_string());
    event.session_id = session_id.map(ToString::to_string);
    if let Some(session_id) = session_id {
        event.payload = Some(json!({
            "sessionId": session_id,
        }));
    } else if !params.is_null() {
        event.payload = Some(compact_telemetry_payload(&Value::Null, params));
    }
    telemetry_store()?.append(&event)?;
    Ok(())
}

fn populate_telemetry_event(event: &mut PilotTelemetryEvent, output: &Value, params: &Value) {
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
        .or_else(|| telemetry_string(params, &["target"]));
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

fn compact_telemetry_payload(output: &Value, params: &Value) -> Value {
    json!({
        "params": compact_value_summary(params),
        "result": compact_value_summary(output),
    })
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
