use std::io::{self, BufRead, Write};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    dispatch, log_telemetry_error, log_telemetry_success, telemetry_surface_label, CliCommand,
    CliError, TelemetryRecentOptions,
};

#[path = "serve_runtime/daemon_state.rs"]
mod daemon_state;
#[path = "serve_runtime/interaction_handlers.rs"]
mod interaction_handlers;
#[path = "serve_runtime/params.rs"]
mod params;
#[path = "serve_runtime/presenters.rs"]
mod presenters;
#[path = "serve_runtime/search_handlers.rs"]
mod search_handlers;
#[path = "serve_runtime/session_handlers.rs"]
mod session_handlers;
#[path = "serve_runtime/tab_handlers.rs"]
mod tab_handlers;

use daemon_state::ServeDaemonState;

const SERVE_METHODS: &[&str] = &[
    "runtime.status",
    "runtime.open",
    "runtime.readView",
    "runtime.extract",
    "runtime.policy",
    "runtime.compactView",
    "runtime.search",
    "runtime.search.openResult",
    "runtime.search.openTop",
    "runtime.session.create",
    "runtime.session.open",
    "runtime.session.snapshot",
    "runtime.session.compactView",
    "runtime.session.readView",
    "runtime.session.refresh",
    "runtime.session.extract",
    "runtime.session.checkpoint",
    "runtime.session.policy",
    "runtime.session.profile.get",
    "runtime.session.profile.set",
    "runtime.session.synthesize",
    "runtime.session.approve",
    "runtime.session.follow",
    "runtime.session.click",
    "runtime.session.type",
    "runtime.session.typeSecret",
    "runtime.session.submit",
    "runtime.session.secret.store",
    "runtime.session.secret.clear",
    "runtime.session.paginate",
    "runtime.session.expand",
    "runtime.session.replay",
    "runtime.session.close",
    "runtime.telemetry.summary",
    "runtime.telemetry.recent",
    "runtime.tab.open",
    "runtime.tab.list",
    "runtime.tab.select",
    "runtime.tab.close",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServeJsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

pub(crate) fn handle_serve() -> Result<(), CliError> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut daemon_state = ServeDaemonState::new()?;

    let serve_result = (|| -> Result<(), CliError> {
        for line in stdin.lock().lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<ServeJsonRpcRequest>(trimmed) {
                Ok(request) => serve_dispatch(request, &mut daemon_state),
                Err(error) => presenters::present_json_rpc_error(
                    Value::Null,
                    -32700,
                    format!("Invalid JSON-RPC request: {error}"),
                ),
            };

            writeln!(
                stdout,
                "{}",
                serde_json::to_string(&response).expect("serve response should serialize")
            )?;
            stdout.flush()?;
        }

        Ok(())
    })();

    let cleanup_result = daemon_state.cleanup();

    match (serve_result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
    }
}

fn serve_dispatch(request: ServeJsonRpcRequest, daemon_state: &mut ServeDaemonState) -> Value {
    let ServeJsonRpcRequest {
        id, method, params, ..
    } = request;

    let result = match method.as_str() {
        "runtime.status" => presenters::present_runtime_status(SERVE_METHODS),
        "runtime.open" => params::json_target_options(&params)
            .and_then(|options| dispatch(CliCommand::Open(options))),
        "runtime.readView" => params::json_target_options(&params)
            .and_then(|options| dispatch(CliCommand::ReadView(options))),
        "runtime.extract" => params::json_extract_options(&params)
            .and_then(|options| dispatch(CliCommand::Extract(options))),
        "runtime.policy" => params::json_target_options(&params)
            .and_then(|options| dispatch(CliCommand::Policy(options))),
        "runtime.compactView" => params::json_target_options(&params)
            .and_then(|options| dispatch(CliCommand::CompactView(options))),
        "runtime.search" => search_handlers::serve_search(&params, daemon_state),
        "runtime.search.openResult" => {
            search_handlers::serve_search_open_result(&params, daemon_state)
        }
        "runtime.search.openTop" => search_handlers::serve_search_open_top(&params, daemon_state),
        "runtime.session.create" => session_handlers::serve_session_create(&params, daemon_state),
        "runtime.session.open" => session_handlers::serve_session_open(&params, daemon_state),
        "runtime.session.snapshot" => {
            session_handlers::serve_session_snapshot(&params, daemon_state)
        }
        "runtime.session.compactView" => {
            session_handlers::serve_session_compact_view(&params, daemon_state)
        }
        "runtime.session.readView" => {
            session_handlers::serve_session_read_view(&params, daemon_state)
        }
        "runtime.session.refresh" => session_handlers::serve_session_refresh(&params, daemon_state),
        "runtime.session.extract" => session_handlers::serve_session_extract(&params, daemon_state),
        "runtime.session.checkpoint" => {
            session_handlers::serve_session_checkpoint(&params, daemon_state)
        }
        "runtime.session.policy" => session_handlers::serve_session_policy(&params, daemon_state),
        "runtime.session.profile.get" => {
            session_handlers::serve_session_profile_get(&params, daemon_state)
        }
        "runtime.session.profile.set" => {
            session_handlers::serve_session_profile_set(&params, daemon_state)
        }
        "runtime.session.synthesize" => {
            if params.get("sessionId").is_some() {
                session_handlers::serve_session_synthesize(&params, daemon_state)
            } else {
                params::json_session_synthesize_options(&params)
                    .and_then(|options| dispatch(CliCommand::SessionSynthesize(options)))
            }
        }
        "runtime.session.approve" => session_handlers::serve_session_approve(&params, daemon_state),
        "runtime.session.follow" => {
            interaction_handlers::serve_session_follow(&params, daemon_state)
        }
        "runtime.session.click" => interaction_handlers::serve_session_click(&params, daemon_state),
        "runtime.session.type" => interaction_handlers::serve_session_type(&params, daemon_state),
        "runtime.session.typeSecret" => {
            interaction_handlers::serve_session_type_secret(&params, daemon_state)
        }
        "runtime.session.submit" => {
            interaction_handlers::serve_session_submit(&params, daemon_state)
        }
        "runtime.session.secret.store" => {
            interaction_handlers::serve_session_secret_store(&params, daemon_state)
        }
        "runtime.session.secret.clear" => {
            interaction_handlers::serve_session_secret_clear(&params, daemon_state)
        }
        "runtime.session.paginate" => {
            interaction_handlers::serve_session_paginate(&params, daemon_state)
        }
        "runtime.session.expand" => {
            interaction_handlers::serve_session_expand(&params, daemon_state)
        }
        "runtime.session.replay" => session_handlers::serve_session_replay(&params, daemon_state),
        "runtime.session.close" => {
            if params.get("sessionId").is_some() {
                session_handlers::serve_session_close(&params, daemon_state)
            } else {
                params::json_session_file_options(&params)
                    .and_then(|options| dispatch(CliCommand::SessionClose(options)))
            }
        }
        "runtime.tab.open" => tab_handlers::serve_tab_open(&params, daemon_state),
        "runtime.tab.list" => tab_handlers::serve_tab_list(&params, daemon_state),
        "runtime.tab.select" => tab_handlers::serve_tab_select(&params, daemon_state),
        "runtime.tab.close" => tab_handlers::serve_tab_close(&params, daemon_state),
        "runtime.telemetry.summary" => dispatch(CliCommand::TelemetrySummary),
        "runtime.telemetry.recent" => {
            let limit = params::json_usize(&params, "limit").unwrap_or(10);
            dispatch(CliCommand::TelemetryRecent(TelemetryRecentOptions {
                limit,
            }))
        }
        _ => Err(CliError::Usage(format!(
            "Unsupported serve method `{method}`."
        ))),
    };

    match result {
        Ok(result) => {
            let _ =
                log_telemetry_success(&telemetry_surface_label("serve"), &method, &result, &params);
            presenters::present_json_rpc_result(id, result)
        }
        Err(error) => {
            let _ = log_telemetry_error(
                &telemetry_surface_label("serve"),
                &method,
                &error.to_string(),
                params.get("sessionId").and_then(Value::as_str),
                &params,
            );
            presenters::present_json_rpc_error(id, -32602, error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{serve_dispatch, ServeDaemonState, ServeJsonRpcRequest};

    #[test]
    fn runtime_status_returns_ready_envelope() {
        let mut daemon_state = ServeDaemonState::new().expect("daemon state");
        let response = serve_dispatch(
            ServeJsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: json!(1),
                method: "runtime.status".to_string(),
                params: json!({}),
            },
            &mut daemon_state,
        );

        assert_eq!(response["jsonrpc"], json!("2.0"));
        assert_eq!(response["id"], json!(1));
        assert_eq!(response["result"]["status"], json!("ready"));
        assert_eq!(response["result"]["transport"], json!("stdio-json-rpc"));
        assert_eq!(response["result"]["daemon"], json!(true));
        assert!(response["result"]["methods"]
            .as_array()
            .expect("methods array")
            .iter()
            .any(|method| method == "runtime.search"));

        daemon_state.cleanup().expect("cleanup");
    }

    #[test]
    fn unsupported_method_returns_json_rpc_error() {
        let mut daemon_state = ServeDaemonState::new().expect("daemon state");
        let response = serve_dispatch(
            ServeJsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: json!("req-1"),
                method: "runtime.unknown".to_string(),
                params: json!({}),
            },
            &mut daemon_state,
        );

        assert_eq!(response["jsonrpc"], json!("2.0"));
        assert_eq!(response["id"], json!("req-1"));
        assert_eq!(response["error"]["code"], json!(-32602));
        assert!(response["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("Unsupported serve method"));

        daemon_state.cleanup().expect("cleanup");
    }
}
