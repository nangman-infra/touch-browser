use serde_json::{json, Value};
use touch_browser_contracts::{CONTRACT_VERSION, STABLE_REF_VERSION};

use super::cli_models::CliCommand;

const AGENT_CONTRACT_VERSION: &str = "1.1.0";

pub(crate) fn capabilities_payload() -> Value {
    json!({
        "status": "ready",
        "version": env!("CARGO_PKG_VERSION"),
        "contractVersion": CONTRACT_VERSION,
        "stableRefVersion": STABLE_REF_VERSION,
        "agentContract": agent_contract_value("capabilities", "full-json"),
        "intendedCaller": "ai-agent",
        "runtimeBoundary": {
            "factFinal": false,
            "evidenceFirst": true,
            "truthDecisionOwner": "higher-level-model-or-human",
            "autoReuseRule": "Only reuse evidence-supported claims when confidenceBand=high and reviewRecommended=false."
        },
        "surfaces": [
            {
                "name": "search",
                "purpose": "discover candidate public web sources",
                "primaryOutput": "SearchReport",
                "nextActionField": "nextActions"
            },
            {
                "name": "open",
                "purpose": "compile a structured page snapshot",
                "primaryOutput": "ActionResult(snapshot-document)",
                "nextActionField": "nextActions"
            },
            {
                "name": "read-view",
                "purpose": "produce readable Markdown for verifier review",
                "primaryOutput": "ReadViewOutput"
            },
            {
                "name": "compact-view",
                "purpose": "produce low-token semantic state for agent loops",
                "primaryOutput": "CompactSnapshotOutput"
            },
            {
                "name": "extract",
                "purpose": "score claims against page-local evidence",
                "primaryOutput": "EvidenceReport",
                "reuseSignal": "claimOutcomes[].reuseAllowed"
            },
            {
                "name": "policy",
                "purpose": "classify page/action risk before interaction",
                "primaryOutput": "PolicyReport"
            },
            {
                "name": "session-synthesize",
                "purpose": "combine multi-page evidence into an auditable report",
                "primaryOutput": "SessionSynthesisReport"
            }
        ],
        "commands": stable_commands(),
        "safety": {
            "headedOverMcp": false,
            "interactiveActionsRequirePolicyPreflight": true,
            "sensitiveInputRequiresExplicitSensitiveFlagOrSecretStore": true,
            "captchaMfaAuthAndHighRiskWriteRequireHumanSupervision": true
        },
        "outputContract": {
            "defaultStdout": "json-except-read-view-and-markdown-synthesis",
            "agentJsonFlag": "--agent-json",
            "jsonErrorsFlag": "--json-errors",
            "standardFields": [
                "agentContract",
                "nextActions",
                "reuseSummary",
                "claimOutcomes[].reuseAllowed"
            ]
        },
        "result": {
            "status": "ready",
            "recommendedFirstCall": "touch-browser capabilities --agent-json"
        },
        "nextActions": [
            {
                "action": "search",
                "command": "touch-browser search <query> --session-file <path>",
                "actor": "ai",
                "canAutoRun": true,
                "headedRequired": false,
                "reason": "Start with discovery when the source URL is not already known."
            },
            {
                "action": "open",
                "command": "touch-browser open <url> --browser --session-file <path>",
                "actor": "ai",
                "canAutoRun": true,
                "headedRequired": false,
                "reason": "Open a known source before extracting evidence."
            }
        ]
    })
}

pub(crate) fn enrich_output(command: &CliCommand, mut output: Value) -> Value {
    let command_name = command_name(command);
    if let Some(session_file) = command_session_file(command) {
        insert_top_level(&mut output, "sessionFile", Value::String(session_file));
    }
    decorate_extract_reports(&mut output);
    add_reuse_summary(&mut output);
    add_next_actions(command, &mut output);
    insert_top_level(
        &mut output,
        "agentContract",
        agent_contract_value(command_name, "full-json"),
    );
    output
}

pub(crate) fn compact_agent_output(command: &CliCommand, output: Value) -> Value {
    let command_name = command_name(command);
    let mut compact = json!({
        "agentContract": agent_contract_value(command_name, "agent-json"),
        "status": infer_status(&output),
        "policy": extract_policy(&output),
        "nextActions": output.get("nextActions").cloned().unwrap_or_else(|| json!([])),
        "reuseSummary": output.get("reuseSummary").cloned(),
        "sessionState": output.get("sessionState").cloned()
            .or_else(|| output.get("session_state").cloned()),
        "sessionFile": output.get("sessionFile").cloned(),
    });

    if matches!(command, CliCommand::Capabilities) {
        return compact_capabilities(output);
    }

    if let Some(search) = output
        .get("result")
        .filter(|value| value.get("results").is_some() && value.get("status").is_some())
        .or_else(|| output.get("search"))
    {
        compact["search"] = compact_search(search);
    }

    add_compact_telemetry(command, &mut compact, &output);
    add_compact_evidence_report(&mut compact, &output);
    add_compact_snapshot(&mut compact, &output);
    add_compact_markdown(&mut compact, &output);
    add_compact_report(&mut compact, &output);
    add_compact_opened_sessions(&mut compact, &output);
    add_compact_command_artifacts(command, &mut compact, &output);

    compact
}

fn add_compact_telemetry(command: &CliCommand, compact: &mut Value, output: &Value) {
    match command {
        CliCommand::TelemetrySummary => {
            if let Some(summary) = output.get("summary").or_else(|| output.get("result")) {
                compact["summary"] = summary.clone();
            }
        }
        CliCommand::TelemetryRecent(_) => {
            if let Some(limit) = output.get("limit") {
                compact["limit"] = limit.clone();
            }
            if let Some(events) = output.get("events").or_else(|| output.get("result")) {
                compact["events"] = events.clone();
            }
        }
        _ => {}
    }
}

fn add_compact_evidence_report(compact: &mut Value, output: &Value) {
    let Some(report) = primary_evidence_report(output) else {
        return;
    };

    compact["source"] = report.get("source").cloned().unwrap_or(Value::Null);
    compact["claimOutcomes"] = report
        .get("claimOutcomes")
        .cloned()
        .unwrap_or_else(|| json!([]));
    compact["citations"] = extract_citations(report);
}

fn add_compact_snapshot(compact: &mut Value, output: &Value) {
    let Some(snapshot) = primary_snapshot(output) else {
        return;
    };

    compact["source"] = snapshot.get("source").cloned().unwrap_or(Value::Null);
    compact["workingSetRefs"] = compact_snapshot_refs(snapshot);
}

fn add_compact_markdown(compact: &mut Value, output: &Value) {
    if let Some(markdown) = output
        .get("markdownText")
        .or_else(|| output.get("markdown"))
    {
        compact["markdown"] = markdown.clone();
    }
}

fn add_compact_report(compact: &mut Value, output: &Value) {
    let Some(report) = output.get("report").or_else(|| output.get("result")) else {
        return;
    };
    if report.get("claims").is_some() || report.get("notes").is_some() {
        compact["report"] = report.clone();
    }
}

fn add_compact_opened_sessions(compact: &mut Value, output: &Value) {
    let Some(opened_sessions) = compact_opened_sessions(output) else {
        return;
    };

    compact["openedCount"] = Value::Number(serde_json::Number::from(opened_sessions.len() as u64));
    if compact.get("source").is_none_or(Value::is_null) {
        if let Some(source) = opened_sessions
            .first()
            .and_then(|opened| opened.get("source"))
            .cloned()
        {
            compact["source"] = source;
        }
    }
    compact["openedSessions"] = Value::Array(opened_sessions);
}

fn add_compact_command_artifacts(command: &CliCommand, compact: &mut Value, output: &Value) {
    match command {
        CliCommand::MemorySummary { .. } => {
            copy_field(output, compact, "requestedActions");
            copy_field(output, compact, "actionCount");
            copy_field(output, compact, "memorySummary");
        }
        CliCommand::Replay { .. } => {
            copy_field(output, compact, "snapshotCount");
            copy_field(output, compact, "evidenceReportCount");
            copy_field(output, compact, "replayTranscript");
        }
        CliCommand::SessionProfile(_) | CliCommand::SetProfile(_) => {
            copy_field(output, compact, "policyProfile");
            if compact.get("policyProfile").is_none() {
                if let Some(profile) = output.pointer("/result/policyProfile").cloned() {
                    compact["policyProfile"] = profile;
                }
            }
        }
        _ => {}
    }
}

fn copy_field(source: &Value, target: &mut Value, field: &str) {
    if let Some(value) = source.get(field).cloned() {
        target[field] = value;
    }
}

fn compact_capabilities(output: Value) -> Value {
    json!({
        "agentContract": agent_contract_value("capabilities", "agent-json"),
        "status": output.get("status").cloned().unwrap_or_else(|| json!("ready")),
        "version": output.get("version").cloned(),
        "contractVersion": output.get("contractVersion").cloned(),
        "stableRefVersion": output.get("stableRefVersion").cloned(),
        "intendedCaller": output.get("intendedCaller").cloned(),
        "runtimeBoundary": output.get("runtimeBoundary").cloned(),
        "commands": output.get("commands").cloned(),
        "safety": output.get("safety").cloned(),
        "outputContract": output.get("outputContract").cloned(),
        "nextActions": [
            {
                "action": "search",
                "command": "touch-browser search <query> --session-file <path>",
                "actor": "ai",
                "canAutoRun": true,
                "headedRequired": false,
                "reason": "Start with discovery when the source URL is not already known."
            },
            {
                "action": "open",
                "command": "touch-browser open <url> --browser --session-file <path>",
                "actor": "ai",
                "canAutoRun": true,
                "headedRequired": false,
                "reason": "Open a known source before extracting evidence."
            }
        ]
    })
}

fn stable_commands() -> Value {
    json!([
        {
            "name": "capabilities",
            "aliases": ["status"],
            "aiPurpose": "discover runtime capabilities and output contract",
            "autoRunnable": true
        },
        {
            "name": "search",
            "aiPurpose": "discover candidate sources",
            "autoRunnable": true,
            "next": ["search-open-top", "search-open-result", "read-view", "extract"]
        },
        {
            "name": "open",
            "aiPurpose": "capture a page snapshot",
            "autoRunnable": true,
            "next": ["session-read", "session-extract", "session-synthesize"]
        },
        {
            "name": "read-view",
            "aiPurpose": "inspect readable page content",
            "autoRunnable": true,
            "next": ["extract"]
        },
        {
            "name": "compact-view",
            "aiPurpose": "inspect low-token page state",
            "autoRunnable": true,
            "next": ["extract", "follow", "expand"]
        },
        {
            "name": "extract",
            "aiPurpose": "verify claims against page-local evidence",
            "autoRunnable": true,
            "next": ["session-synthesize", "search", "open"]
        },
        {
            "name": "policy",
            "aiPurpose": "classify page or action risk",
            "autoRunnable": true,
            "next": ["open", "checkpoint", "human-handoff"]
        },
        {
            "name": "session-synthesize",
            "aiPurpose": "produce auditable multi-page evidence output",
            "autoRunnable": true,
            "next": ["session-close"]
        }
    ])
}

fn agent_contract_value(command: &str, format: &str) -> Value {
    json!({
        "version": AGENT_CONTRACT_VERSION,
        "format": format,
        "intendedCaller": "ai-agent",
        "command": command,
        "reuseRule": {
            "reuseAllowedWhen": {
                "verdict": "evidence-supported",
                "confidenceBand": "high",
                "reviewRecommended": false
            },
            "otherwise": "browse-more-or-escalate"
        }
    })
}

fn command_name(command: &CliCommand) -> &'static str {
    match command {
        CliCommand::Capabilities => "capabilities",
        CliCommand::Search(_) => "search",
        CliCommand::SearchOpenResult(_) => "search-open-result",
        CliCommand::SearchOpenTop(_) => "search-open-top",
        CliCommand::Mcp => "mcp",
        CliCommand::Update(_) => "update",
        CliCommand::Uninstall(_) => "uninstall",
        CliCommand::Open(_) => "open",
        CliCommand::Snapshot(_) => "snapshot",
        CliCommand::CompactView(_) => "compact-view",
        CliCommand::ReadView(_) => "read-view",
        CliCommand::Extract(_) => "extract",
        CliCommand::Policy(_) => "policy",
        CliCommand::SessionSnapshot(_) => "session-snapshot",
        CliCommand::SessionCompact(_) => "session-compact",
        CliCommand::SessionRead(_) => "session-read",
        CliCommand::SessionRefresh(_) => "refresh",
        CliCommand::SessionExtract(_) => "session-extract",
        CliCommand::SessionCheckpoint(_) => "checkpoint",
        CliCommand::SessionPolicy(_) => "session-policy",
        CliCommand::SessionProfile(_) => "session-profile",
        CliCommand::SetProfile(_) => "set-profile",
        CliCommand::SessionSynthesize(_) => "session-synthesize",
        CliCommand::Approve(_) => "approve",
        CliCommand::Follow(_) => "follow",
        CliCommand::Click(_) => "click",
        CliCommand::Type(_) => "type",
        CliCommand::Submit(_) => "submit",
        CliCommand::Paginate(_) => "paginate",
        CliCommand::Expand(_) => "expand",
        CliCommand::BrowserReplay(_) => "browser-replay",
        CliCommand::SessionClose(_) => "session-close",
        CliCommand::TelemetrySummary => "telemetry-summary",
        CliCommand::TelemetryRecent(_) => "telemetry-recent",
        CliCommand::Replay { .. } => "replay",
        CliCommand::MemorySummary { .. } => "memory-summary",
        CliCommand::Serve => "serve",
    }
}

fn insert_top_level(output: &mut Value, key: &str, value: Value) {
    if let Some(object) = output.as_object_mut() {
        object.entry(key.to_string()).or_insert(value);
    }
}

fn decorate_extract_reports(output: &mut Value) {
    for path in [
        &["extract", "output"][..],
        &["result", "output"][..],
        &["output"][..],
        &["report"][..],
    ] {
        if let Some(report) = nested_mut(output, path) {
            decorate_claim_outcomes(report);
        }
    }
}

fn decorate_claim_outcomes(report: &mut Value) {
    let fallback_citation = report.get("source").map(source_to_citation);
    let Some(outcomes) = report
        .get_mut("claimOutcomes")
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    for outcome in outcomes {
        let reuse_allowed = outcome.get("verdict").and_then(Value::as_str)
            == Some("evidence-supported")
            && outcome.get("confidenceBand").and_then(Value::as_str) == Some("high")
            && outcome
                .get("reviewRecommended")
                .and_then(Value::as_bool)
                .map(|review| !review)
                .unwrap_or(false);

        if let Some(object) = outcome.as_object_mut() {
            object.insert("reuseAllowed".to_string(), Value::Bool(reuse_allowed));
            if !object.contains_key("citation")
                && object
                    .get("support")
                    .and_then(Value::as_array)
                    .is_some_and(|support| !support.is_empty())
            {
                if let Some(citation) = fallback_citation.clone() {
                    object.insert("citation".to_string(), citation);
                }
            }
        }
    }
}

fn add_reuse_summary(output: &mut Value) {
    let Some(report) = primary_evidence_report(output) else {
        return;
    };
    let outcomes = report
        .get("claimOutcomes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if outcomes.is_empty() {
        return;
    }

    let total = outcomes.len();
    let reuse_allowed = outcomes
        .iter()
        .filter(|outcome| {
            outcome
                .get("reuseAllowed")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    let review_required = total.saturating_sub(reuse_allowed);
    insert_top_level(
        output,
        "reuseSummary",
        json!({
            "totalClaims": total,
            "reuseAllowedClaims": reuse_allowed,
            "reviewRequiredClaims": review_required,
            "allClaimsReusable": total > 0 && reuse_allowed == total
        }),
    );
}

fn add_next_actions(command: &CliCommand, output: &mut Value) {
    let actions = match command {
        CliCommand::Capabilities => vec![
            next_action(
                "search",
                Some("touch-browser search <query> --session-file <path>"),
                true,
                false,
                "Use discovery when no source URL is known.",
            ),
            next_action(
                "open",
                Some("touch-browser open <url> --browser --session-file <path>"),
                true,
                false,
                "Open a known source before extracting evidence.",
            ),
        ],
        CliCommand::Search(_) => search_next_actions(output),
        CliCommand::SearchOpenResult(_) | CliCommand::SearchOpenTop(_) => {
            search_open_next_actions(output)
        }
        CliCommand::Open(_) | CliCommand::Snapshot(_) => open_next_actions(output),
        CliCommand::CompactView(_) | CliCommand::ReadView(_) => {
            read_view_next_actions(command, output)
        }
        CliCommand::SessionRead(_) | CliCommand::SessionCompact(_) => {
            session_read_next_actions(output)
        }
        CliCommand::Extract(_) | CliCommand::SessionExtract(_) => extract_next_actions(output),
        CliCommand::Policy(_) | CliCommand::SessionPolicy(_) => policy_next_actions(output),
        CliCommand::SessionSynthesize(_) => synthesize_next_actions(output),
        CliCommand::SessionCheckpoint(_) => vec![next_action(
            "human-handoff",
            None,
            false,
            true,
            "A supervised checkpoint requires human approval before interaction continues.",
        )],
        _ => Vec::new(),
    };

    if !actions.is_empty() {
        insert_top_level(output, "nextActions", Value::Array(actions));
    }
}

fn session_read_next_actions(output: &Value) -> Vec<Value> {
    output
        .get("sessionFile")
        .and_then(Value::as_str)
        .map(|session_file| {
            vec![next_action(
                "session-extract",
                Some(&format!(
                    "touch-browser session-extract --session-file {session_file} --claim <statement>"
                )),
                true,
                false,
                "Verify a concrete claim against the persisted session snapshot.",
            )]
        })
        .unwrap_or_default()
}

fn search_open_next_actions(output: &Value) -> Vec<Value> {
    let session_file = first_opened_session_file(output).or_else(|| {
        output
            .get("sessionFile")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let Some(session_file) = session_file else {
        return vec![
            next_action("session-read", Some("touch-browser session-read --session-file <opened session file> --main-only"), true, false, "Inspect the opened candidate page before extracting claims."),
            next_action("session-extract", Some("touch-browser session-extract --session-file <opened session file> --claim <statement>"), true, false, "Verify claims against the opened page."),
        ];
    };

    vec![
        next_action(
            "session-read",
            Some(&format!(
                "touch-browser session-read --session-file {} --main-only",
                shell_arg(&session_file)
            )),
            true,
            false,
            "Inspect the opened candidate page before extracting claims.",
        ),
        next_action(
            "session-extract",
            Some(&format!(
                "touch-browser session-extract --session-file {} --claim <statement>",
                shell_arg(&session_file)
            )),
            true,
            false,
            "Verify claims against the opened page.",
        ),
    ]
}

fn first_opened_session_file(output: &Value) -> Option<String> {
    output
        .get("opened")
        .and_then(Value::as_array)
        .and_then(|opened| opened.first())
        .and_then(|opened| opened.get("sessionFile"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn search_next_actions(output: &Value) -> Vec<Value> {
    output
        .get("result")
        .and_then(|result| result.get("nextActionHints"))
        .and_then(Value::as_array)
        .map(|hints| {
            hints
                .iter()
                .map(|hint| {
                    let action = hint
                        .get("action")
                        .and_then(Value::as_str)
                        .unwrap_or("open-top");
                    let command = hint
                        .get("command")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| synthesize_search_command(output, action, hint));
                    let can_auto_run = command
                        .as_ref()
                        .map(|_| Value::Bool(true))
                        .unwrap_or_else(|| {
                            hint.get("canAutoRun").cloned().unwrap_or(Value::Bool(false))
                        });
                    json!({
                        "action": action,
                        "command": command,
                        "actor": hint.get("actor").cloned().unwrap_or_else(|| json!("ai")),
                        "canAutoRun": can_auto_run,
                        "headedRequired": hint.get("headedRequired").cloned().unwrap_or(Value::Bool(false)),
                        "resultRanks": hint.get("resultRanks").cloned().unwrap_or_else(|| json!([])),
                        "reason": hint.get("detail").cloned().unwrap_or_else(|| json!("Follow the search hint."))
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn synthesize_search_command(output: &Value, action: &str, hint: &Value) -> Option<String> {
    let session_file = output.get("sessionFile").and_then(Value::as_str)?;
    let engine = search_engine(output)?;
    let ranks = hint_result_ranks(hint);

    match action {
        "open-top" => {
            let limit = ranks.len().max(1);
            Some(format!(
                "touch-browser search-open-top --engine {engine} --session-file {} --limit {limit}",
                shell_arg(session_file)
            ))
        }
        "open-result" => ranks.first().map(|rank| {
            format!(
                "touch-browser search-open-result --engine {engine} --session-file {} --rank {rank}",
                shell_arg(session_file)
            )
        }),
        "read-view" => first_ranked_result_url(output, &ranks).map(|url| {
            format!("touch-browser read-view {} --main-only", shell_arg(&url))
        }),
        _ => None,
    }
}

fn search_engine(output: &Value) -> Option<&str> {
    output
        .get("result")
        .and_then(|result| result.get("engine"))
        .and_then(Value::as_str)
        .or_else(|| output.get("engine").and_then(Value::as_str))
}

fn hint_result_ranks(hint: &Value) -> Vec<u64> {
    hint.get("resultRanks")
        .and_then(Value::as_array)
        .map(|ranks| ranks.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default()
}

fn first_ranked_result_url(output: &Value, ranks: &[u64]) -> Option<String> {
    let first_rank = ranks.first()?;
    output
        .get("result")
        .and_then(|result| result.get("results"))
        .and_then(Value::as_array)
        .and_then(|results| {
            results.iter().find_map(|result| {
                (result.get("rank").and_then(Value::as_u64) == Some(*first_rank))
                    .then(|| {
                        result
                            .get("url")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .flatten()
            })
        })
}

fn command_session_file(command: &CliCommand) -> Option<String> {
    match command {
        CliCommand::Search(options) => options.session_file.as_ref(),
        CliCommand::SearchOpenResult(options) => options.session_file.as_ref(),
        CliCommand::SearchOpenTop(options) => options.session_file.as_ref(),
        CliCommand::Open(options)
        | CliCommand::Snapshot(options)
        | CliCommand::CompactView(options)
        | CliCommand::ReadView(options)
        | CliCommand::Policy(options) => options.session_file.as_ref(),
        CliCommand::Extract(options) => options.session_file.as_ref(),
        CliCommand::SessionSnapshot(options)
        | CliCommand::SessionCompact(options)
        | CliCommand::SessionCheckpoint(options)
        | CliCommand::SessionPolicy(options)
        | CliCommand::SessionProfile(options)
        | CliCommand::BrowserReplay(options)
        | CliCommand::SessionClose(options) => Some(&options.session_file),
        CliCommand::SessionRefresh(options) => Some(&options.session_file),
        CliCommand::SessionRead(options) => Some(&options.session_file),
        CliCommand::SessionExtract(options) => options.session_file.as_ref(),
        CliCommand::SetProfile(options) => Some(&options.session_file),
        CliCommand::SessionSynthesize(options) => Some(&options.session_file),
        CliCommand::Approve(options) => Some(&options.session_file),
        CliCommand::Follow(options) => Some(&options.session_file),
        CliCommand::Click(options) => Some(&options.session_file),
        CliCommand::Type(options) => Some(&options.session_file),
        CliCommand::Submit(options) => Some(&options.session_file),
        CliCommand::Paginate(options) => Some(&options.session_file),
        CliCommand::Expand(options) => Some(&options.session_file),
        _ => None,
    }
    .map(|path| path.display().to_string())
}

fn open_next_actions(output: &Value) -> Vec<Value> {
    let session_file = output.get("sessionFile").and_then(Value::as_str);
    let mut actions = Vec::new();
    if let Some(session_file) = session_file {
        actions.push(next_action(
            "session-read",
            Some(&format!(
                "touch-browser session-read --session-file {session_file} --main-only"
            )),
            true,
            false,
            "Inspect the current page text before claim extraction.",
        ));
        actions.push(next_action(
            "session-extract",
            Some(&format!(
                "touch-browser session-extract --session-file {session_file} --claim <statement>"
            )),
            true,
            false,
            "Verify a concrete claim against the persisted snapshot.",
        ));
    } else {
        actions.push(next_action(
            "extract",
            Some("touch-browser extract <url> --claim <statement>"),
            true,
            false,
            "Verify a concrete claim against this source.",
        ));
    }
    actions
}

fn read_view_next_actions(command: &CliCommand, output: &Value) -> Vec<Value> {
    if let Some(session_file) = output.get("sessionFile").and_then(Value::as_str) {
        return vec![next_action(
            "session-extract",
            Some(&format!(
                "touch-browser session-extract --session-file {} --claim <statement>",
                shell_arg(session_file)
            )),
            true,
            false,
            "Verify a concrete claim against the persisted session snapshot.",
        )];
    }

    let target = source_url(output).or_else(|| command_target(command).map(str::to_string));
    let command = target.map(|target| {
        format!(
            "touch-browser extract {} --claim <statement>",
            shell_arg(&target)
        )
    });

    vec![next_action(
        "extract",
        command.as_deref(),
        command.is_some(),
        false,
        "Verify a concrete claim after reading the page.",
    )]
}

fn extract_next_actions(output: &Value) -> Vec<Value> {
    let summary = output.get("reuseSummary");
    if summary
        .and_then(|value| value.get("allClaimsReusable"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let command = output
            .get("sessionFile")
            .and_then(Value::as_str)
            .map(|session_file| {
                format!(
                    "touch-browser session-synthesize --session-file {session_file} --format json"
                )
            });
        if command.is_none() {
            return vec![next_action(
                "answer-with-citations",
                None,
                true,
                false,
                "All claims are reusable; answer using the returned claimOutcomes and citations.",
            )];
        }
        return vec![next_action(
            "session-synthesize",
            command.as_deref(),
            true,
            false,
            "All claims are reusable; synthesize an auditable citation artifact.",
        )];
    }

    if all_claims_contradicted(output) {
        return vec![
            next_action(
                "reject-or-revise-claim",
                None,
                true,
                false,
                "The retrieved evidence contradicts every checked claim; do not reuse the original claim.",
            ),
            next_action(
                "search",
                Some("touch-browser search <cross-check-query> --session-file <path>"),
                true,
                false,
                "Run a cross-check only if the user needs confirmation beyond the current source.",
            ),
        ];
    }

    vec![
        next_action(
            "search",
            Some("touch-browser search <more-specific-query> --session-file <path>"),
            true,
            false,
            "At least one claim is unresolved, contradicted, or review-bound.",
        ),
        next_action(
            "open",
            Some("touch-browser open <more-specific-source> --browser --session-file <path>"),
            true,
            false,
            "Open a more specific source before reusing the claim.",
        ),
    ]
}

fn policy_next_actions(output: &Value) -> Vec<Value> {
    let decision = extract_policy(output)
        .and_then(|policy| policy.get("decision").cloned())
        .and_then(|value| value.as_str().map(str::to_string));

    match decision.as_deref() {
        Some("allow") => vec![next_action(
            "open-or-extract",
            Some("touch-browser open <url> --browser --session-file <path>"),
            true,
            false,
            "Policy allows read-only evidence collection.",
        )],
        Some("review") => vec![next_action(
            "checkpoint",
            Some("touch-browser checkpoint --session-file <path>"),
            false,
            true,
            "Policy requires supervised review before continuing.",
        )],
        Some("block") => vec![next_action(
            "human-handoff",
            None,
            false,
            true,
            "Policy blocked the source or action.",
        )],
        _ => Vec::new(),
    }
}

fn synthesize_next_actions(output: &Value) -> Vec<Value> {
    let command = output
        .get("sessionFile")
        .and_then(Value::as_str)
        .map(|session_file| format!("touch-browser session-close --session-file {session_file}"))
        .unwrap_or_else(|| "touch-browser session-close --session-file <path>".to_string());

    vec![next_action(
        "session-close",
        Some(&command),
        true,
        false,
        "Close the persisted browser session after the audit artifact is no longer needed.",
    )]
}

fn all_claims_contradicted(output: &Value) -> bool {
    let Some(report) = primary_evidence_report(output) else {
        return false;
    };
    let Some(outcomes) = report.get("claimOutcomes").and_then(Value::as_array) else {
        return false;
    };

    !outcomes.is_empty()
        && outcomes
            .iter()
            .all(|outcome| outcome.get("verdict").and_then(Value::as_str) == Some("contradicted"))
}

fn next_action(
    action: &str,
    command: Option<&str>,
    can_auto_run: bool,
    headed_required: bool,
    reason: &str,
) -> Value {
    json!({
        "action": action,
        "command": command,
        "actor": if headed_required { "human" } else { "ai" },
        "canAutoRun": can_auto_run,
        "headedRequired": headed_required,
        "reason": reason
    })
}

fn primary_evidence_report(output: &Value) -> Option<&Value> {
    nested(output, &["extract", "output"])
        .filter(|value| value.get("claimOutcomes").is_some())
        .or_else(|| {
            nested(output, &["result", "output"])
                .filter(|value| value.get("claimOutcomes").is_some())
        })
        .or_else(|| {
            output
                .get("output")
                .filter(|value| value.get("claimOutcomes").is_some())
        })
        .or_else(|| {
            output
                .get("report")
                .filter(|value| value.get("claimOutcomes").is_some())
        })
}

fn primary_snapshot(output: &Value) -> Option<&Value> {
    nested(output, &["open", "output"])
        .filter(|value| value.get("blocks").is_some())
        .or_else(|| {
            nested(output, &["result", "output"]).filter(|value| value.get("blocks").is_some())
        })
        .or_else(|| {
            output
                .get("output")
                .filter(|value| value.get("blocks").is_some())
        })
}

fn extract_policy(output: &Value) -> Option<Value> {
    output
        .get("policy")
        .cloned()
        .or_else(|| nested(output, &["open", "policy"]).cloned())
        .or_else(|| nested(output, &["extract", "policy"]).cloned())
        .or_else(|| nested(output, &["result", "policy"]).cloned())
}

fn extract_citations(report: &Value) -> Value {
    let mut citations = report
        .get("claimOutcomes")
        .and_then(Value::as_array)
        .map(|outcomes| {
            outcomes
                .iter()
                .filter_map(|outcome| outcome.get("citation"))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if citations.is_empty()
        && report
            .get("claimOutcomes")
            .and_then(Value::as_array)
            .is_some_and(|outcomes| !outcomes.is_empty())
    {
        if let Some(source) = report.get("source") {
            citations.push(source_to_citation(source));
        }
    }
    Value::Array(citations)
}

fn source_to_citation(source: &Value) -> Value {
    json!({
        "url": source.get("url").or_else(|| source.get("sourceUrl")).cloned().unwrap_or(Value::Null),
        "sourceLabel": source.get("sourceLabel").or_else(|| source.get("title")).cloned().unwrap_or(Value::Null),
        "sourceType": source.get("sourceType").cloned().unwrap_or(Value::Null),
        "sourceRisk": source.get("sourceRisk").cloned().unwrap_or_else(|| json!("unknown"))
    })
}

fn source_url(output: &Value) -> Option<String> {
    output
        .get("source")
        .and_then(|source| source.get("sourceUrl").or_else(|| source.get("url")))
        .and_then(Value::as_str)
        .or_else(|| {
            output
                .get("sessionState")
                .or_else(|| output.get("session_state"))
                .and_then(|session| session.get("currentUrl"))
                .and_then(Value::as_str)
        })
        .map(str::to_string)
}

fn command_target(command: &CliCommand) -> Option<&str> {
    match command {
        CliCommand::Open(options)
        | CliCommand::Snapshot(options)
        | CliCommand::CompactView(options)
        | CliCommand::ReadView(options)
        | CliCommand::Policy(options) => Some(&options.target),
        CliCommand::Extract(options) => Some(&options.target),
        _ => None,
    }
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '-' | '_' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn compact_snapshot_refs(snapshot: &Value) -> Value {
    let refs = snapshot
        .get("blocks")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .take(12)
                .filter_map(|block| {
                    Some(json!({
                        "id": block.get("id")?,
                        "ref": block.get("ref")?,
                        "kind": block.get("kind")?,
                        "role": block.get("role")?,
                        "text": block.get("text")?
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Value::Array(refs)
}

fn compact_search(search: &Value) -> Value {
    let results = search
        .get("results")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(10)
                .map(|item| {
                    json!({
                        "rank": item.get("rank"),
                        "title": item.get("title"),
                        "url": item.get("url"),
                        "domain": item.get("domain"),
                        "officialLikely": item.get("officialLikely"),
                        "selectionScore": item.get("selectionScore"),
                        "recommendedSurface": item.get("recommendedSurface")
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({
        "status": search.get("status"),
        "query": search.get("query"),
        "engine": search.get("engine"),
        "resultCount": search.get("resultCount"),
        "recommendedResultRanks": search.get("recommendedResultRanks"),
        "results": results
    })
}

fn compact_opened_sessions(output: &Value) -> Option<Vec<Value>> {
    let opened = output.get("opened").and_then(Value::as_array)?;
    let sessions = opened
        .iter()
        .map(|opened| {
            let source = nested(opened, &["result", "output", "source"])
                .cloned()
                .or_else(|| {
                    opened.get("selectedResult").map(|selected| {
                        json!({
                            "sourceUrl": selected.get("url").cloned().unwrap_or(Value::Null),
                            "title": selected.get("title").cloned().unwrap_or(Value::Null),
                            "domain": selected.get("domain").cloned().unwrap_or(Value::Null),
                            "officialLikely": selected.get("officialLikely").cloned().unwrap_or(Value::Null)
                        })
                    })
                })
                .unwrap_or(Value::Null);
            json!({
                "rank": opened.get("rank").cloned().unwrap_or(Value::Null),
                "sessionFile": opened.get("sessionFile").cloned().unwrap_or(Value::Null),
                "source": source,
                "quality": opened.get("diagnostics").and_then(|diagnostics| diagnostics.get("qualityLabel")).cloned().unwrap_or(Value::Null)
            })
        })
        .collect::<Vec<_>>();
    Some(sessions)
}

fn infer_status(output: &Value) -> Value {
    output
        .get("status")
        .cloned()
        .or_else(|| nested(output, &["result", "status"]).cloned())
        .or_else(|| nested(output, &["open", "status"]).cloned())
        .or_else(|| nested(output, &["extract", "status"]).cloned())
        .unwrap_or_else(|| json!("succeeded"))
}

fn nested<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
}

fn nested_mut<'a>(value: &'a mut Value, path: &[&str]) -> Option<&'a mut Value> {
    let mut current = value;
    for key in path {
        current = current.get_mut(*key)?;
    }
    Some(current)
}
