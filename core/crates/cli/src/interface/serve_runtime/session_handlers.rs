use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::{
    approved_risk_labels, dispatch, load_browser_cli_session, parse_output_format,
    parse_policy_profile, parse_source_risk, policy_profile_label,
    promoted_policy_profile_for_risks, render_session_synthesis_markdown, save_browser_cli_session,
    CliCommand, CliError, EvidenceCitation, OutputFormat, PolicyProfile, ReadOnlyRuntime,
    SessionExtractOptions, SessionFileOptions, SessionProfileSetOptions, SessionReadOptions,
    SessionRefreshOptions, SessionSynthesisClaim, SessionSynthesisClaimStatus,
    SessionSynthesisReport, SourceRisk, TargetOptions, CONTRACT_VERSION, DEFAULT_OPENED_AT,
    DEFAULT_REQUESTED_TOKENS,
};

use super::{
    daemon_state::ServeDaemonState,
    params::{
        json_ack_risks, json_bool, json_string_array, json_usize, optional_json_string,
        required_json_string,
    },
    presenters,
};

#[derive(Debug, Clone)]
pub(crate) struct ServeSessionOpenRequest {
    pub(crate) session_id: String,
    pub(crate) requested_tab_id: Option<String>,
    pub(crate) target: String,
    pub(crate) budget: usize,
    pub(crate) source_risk: Option<SourceRisk>,
    pub(crate) source_label: Option<String>,
    pub(crate) new_allowlisted_domains: Vec<String>,
    pub(crate) headed: Option<bool>,
    pub(crate) browser: bool,
}

pub(crate) fn serve_session_create(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let headless = !json_bool(params, "headed").unwrap_or(false);
    let allowlisted_domains = json_string_array(params, "allowDomains")?;
    let (session_id, active_tab_id) =
        daemon_state.create_session(headless, allowlisted_domains.clone())?;
    presenters::present_session_created(session_id, active_tab_id, headless, allowlisted_domains, 1)
}

pub(crate) fn serve_session_open(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let target = required_json_string(params, "target")?;
    let source_risk = optional_json_string(params, "sourceRisk")
        .map(|value| parse_source_risk(&value))
        .transpose()?;
    let source_label = optional_json_string(params, "sourceLabel");
    let allowlisted_domains = json_string_array(params, "allowDomains")?;
    let headed = json_bool(params, "headed");
    let browser = json_bool(params, "browser").unwrap_or(true);
    let budget = json_usize(params, "budget").unwrap_or(DEFAULT_REQUESTED_TOKENS);

    serve_session_open_internal(
        daemon_state,
        ServeSessionOpenRequest {
            session_id,
            requested_tab_id: tab_id,
            target,
            budget,
            source_risk,
            source_label,
            new_allowlisted_domains: allowlisted_domains,
            headed,
            browser,
        },
    )
}

pub(crate) fn serve_session_snapshot(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionSnapshot(SessionFileOptions {
        session_file,
    }))?;
    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
}

pub(crate) fn serve_session_compact_view(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionCompact(SessionFileOptions {
        session_file,
    }))?;
    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
}

pub(crate) fn serve_session_read_view(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let main_only = json_bool(params, "mainOnly").unwrap_or(false);
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionRead(SessionReadOptions {
        session_file,
        main_only,
    }))?;
    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
}

pub(crate) fn serve_session_refresh(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let headed = json_bool(params, "headed").unwrap_or(false);
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionRefresh(SessionRefreshOptions {
        session_file,
        headed,
    }))?;
    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
}

pub(crate) fn serve_session_extract(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let claims = json_string_array(params, "claims")?;
    if claims.is_empty() {
        return Err(CliError::Usage(
            "serve params `claims` must include at least one statement.".to_string(),
        ));
    }
    let verifier_command = optional_json_string(params, "verifierCommand");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionExtract(SessionExtractOptions {
        session_file: Some(session_file),
        engine: None,
        claims,
        verifier_command,
    }))?;
    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
}

pub(crate) fn serve_session_policy(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionPolicy(SessionFileOptions {
        session_file,
    }))?;
    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
}

pub(crate) fn serve_session_profile_get(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionProfile(SessionFileOptions {
        session_file,
    }))?;
    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
}

pub(crate) fn serve_session_profile_set(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let profile_value = required_json_string(params, "profile")?;
    let profile = parse_policy_profile(&profile_value)?;
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SetProfile(SessionProfileSetOptions {
        session_file,
        profile,
    }))?;
    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
}

pub(crate) fn serve_session_checkpoint(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let mut result = dispatch(CliCommand::SessionCheckpoint(SessionFileOptions {
        session_file,
    }))?;
    let approved_risks = {
        let session = daemon_state.session(&session_id)?;
        approved_risk_labels(&session.approved_risks)
    };
    result["checkpoint"]["approvedRisks"] = Value::from(approved_risks);
    result["checkpoint"]["approvalPanel"]["approvedRisks"] =
        result["checkpoint"]["approvedRisks"].clone();
    result["checkpoint"]["playbook"]["approvedRisks"] =
        result["checkpoint"]["approvedRisks"].clone();
    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
}

pub(crate) fn serve_session_synthesize(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let note_limit = json_usize(params, "noteLimit").unwrap_or(12);
    let format = optional_json_string(params, "format")
        .map(|value| parse_output_format(&value))
        .transpose()?
        .unwrap_or(OutputFormat::Json);
    let session = daemon_state.session(&session_id)?;

    let runtime = ReadOnlyRuntime::default();
    let mut tab_reports = Vec::new();

    for (tab_id, tab) in &session.tabs {
        if !tab.session_file.is_file() {
            continue;
        }
        let persisted = load_browser_cli_session(&tab.session_file)?;
        let report = runtime.synthesize_session(
            &persisted.session,
            &persisted.session.state.updated_at,
            note_limit,
        )?;
        tab_reports.push((tab_id.clone(), report));
    }

    if tab_reports.is_empty() {
        return Err(CliError::Usage(format!(
            "Serve session `{session_id}` has no opened tabs to synthesize."
        )));
    }

    let report = combine_session_synthesis_reports(&session_id, note_limit, &tab_reports);
    presenters::present_session_synthesize(
        session_id,
        session.active_tab_id.clone(),
        session.tabs.len(),
        format,
        (format == OutputFormat::Markdown).then(|| render_session_synthesis_markdown(&report)),
        report,
        tab_reports,
    )
}

pub(crate) fn serve_session_approve(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let ack_risks = json_ack_risks(params, "ackRisks")?;
    if ack_risks.is_empty() {
        return Err(CliError::Usage(
            "serve params `ackRisks` must include at least one approval risk.".to_string(),
        ));
    }

    let session = daemon_state.session_mut(&session_id)?;
    for ack_risk in ack_risks {
        session.approved_risks.insert(ack_risk);
    }
    let promoted_profile = promoted_policy_profile_for_risks(
        PolicyProfile::InteractiveReview,
        &session.approved_risks,
    );
    for tab in session.tabs.values() {
        if !tab.session_file.is_file() {
            continue;
        }
        let mut persisted = load_browser_cli_session(&tab.session_file)?;
        persisted.session.state.policy_profile = promoted_profile;
        save_browser_cli_session(&tab.session_file, &persisted)?;
    }

    presenters::present_session_approved(
        session_id,
        approved_risk_labels(&session.approved_risks),
        policy_profile_label(promoted_profile),
    )
}

pub(crate) fn serve_session_replay(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::BrowserReplay(SessionFileOptions {
        session_file,
    }))?;
    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
}

pub(crate) fn serve_session_close(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    presenters::present_session_close(daemon_state.close_session(&session_id)?)
}

pub(crate) fn serve_session_open_internal(
    daemon_state: &mut ServeDaemonState,
    request: ServeSessionOpenRequest,
) -> Result<Value, CliError> {
    let ServeSessionOpenRequest {
        session_id,
        requested_tab_id,
        target,
        budget,
        source_risk,
        source_label,
        new_allowlisted_domains,
        headed,
        browser,
    } = request;

    if !browser {
        return Err(CliError::Usage(
            "Serve daemon sessions currently require browser-backed open.".to_string(),
        ));
    }

    let resolved_tab_id = match requested_tab_id.as_deref() {
        Some(tab_id) => {
            daemon_state.ensure_tab(&session_id, tab_id)?;
            daemon_state.select_tab(&session_id, tab_id)?;
            tab_id.to_string()
        }
        None => daemon_state.ensure_active_tab(&session_id)?,
    };

    daemon_state.extend_session_allowlist(&session_id, &new_allowlisted_domains)?;
    let (headless, allowlisted_domains, session_file) = {
        let session = daemon_state.session(&session_id)?;
        let tab = session.tabs.get(&resolved_tab_id).ok_or_else(|| {
            CliError::Usage(format!(
                "Serve session `{session_id}` does not contain tab `{resolved_tab_id}`."
            ))
        })?;
        (
            session.headless,
            session.allowlisted_domains.clone(),
            tab.session_file.clone(),
        )
    };

    let result = dispatch(CliCommand::Open(TargetOptions {
        target,
        budget,
        source_risk,
        source_label,
        allowlisted_domains,
        browser: true,
        headed: headed.unwrap_or(!headless),
        main_only: false,
        session_file: Some(session_file),
    }))?;

    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
}

fn combine_session_synthesis_reports(
    session_id: &str,
    note_limit: usize,
    reports: &[(String, SessionSynthesisReport)],
) -> SessionSynthesisReport {
    #[derive(Debug, Clone)]
    struct AggregateClaim {
        claim_id: String,
        statement: String,
        status: SessionSynthesisClaimStatus,
        snapshot_ids: BTreeSet<String>,
        support_refs: BTreeSet<String>,
        citations: Vec<EvidenceCitation>,
        citation_keys: BTreeSet<String>,
    }

    fn citation_key(citation: &EvidenceCitation) -> String {
        format!(
            "{}|{}|{:?}|{:?}|{}",
            citation.url,
            citation.retrieved_at,
            citation.source_type,
            citation.source_risk,
            citation.source_label.clone().unwrap_or_default()
        )
    }

    fn merge_claim(
        aggregates: &mut BTreeMap<(String, String), AggregateClaim>,
        claim: &SessionSynthesisClaim,
        status: SessionSynthesisClaimStatus,
    ) {
        let key = (claim.claim_id.clone(), claim.statement.clone());
        let aggregate = aggregates.entry(key).or_insert_with(|| AggregateClaim {
            claim_id: claim.claim_id.clone(),
            statement: claim.statement.clone(),
            status,
            snapshot_ids: BTreeSet::new(),
            support_refs: BTreeSet::new(),
            citations: Vec::new(),
            citation_keys: BTreeSet::new(),
        });

        aggregate
            .snapshot_ids
            .extend(claim.snapshot_ids.iter().cloned());
        aggregate
            .support_refs
            .extend(claim.support_refs.iter().cloned());
        for citation in &claim.citations {
            let key = citation_key(citation);
            if aggregate.citation_keys.insert(key) {
                aggregate.citations.push(citation.clone());
            }
        }
    }

    let mut visited_urls = BTreeSet::new();
    let mut working_set_refs = BTreeSet::new();
    let mut synthesized_notes = Vec::new();
    let mut note_keys = BTreeSet::new();
    let mut supported = BTreeMap::<(String, String), AggregateClaim>::new();
    let mut contradicted = BTreeMap::<(String, String), AggregateClaim>::new();
    let mut unsupported = BTreeMap::<(String, String), AggregateClaim>::new();
    let mut needs_more_browsing = BTreeMap::<(String, String), AggregateClaim>::new();
    let mut snapshot_count = 0usize;
    let mut evidence_report_count = 0usize;
    let mut generated_at = DEFAULT_OPENED_AT.to_string();

    for (_, report) in reports {
        snapshot_count += report.snapshot_count;
        evidence_report_count += report.evidence_report_count;
        generated_at = report.generated_at.clone();
        visited_urls.extend(report.visited_urls.iter().cloned());
        working_set_refs.extend(report.working_set_refs.iter().cloned());
        for note in &report.synthesized_notes {
            if note_keys.insert(note.clone()) && synthesized_notes.len() < note_limit {
                synthesized_notes.push(note.clone());
            }
        }
        for claim in &report.supported_claims {
            merge_claim(
                &mut supported,
                claim,
                SessionSynthesisClaimStatus::EvidenceSupported,
            );
        }
        for claim in &report.contradicted_claims {
            merge_claim(
                &mut contradicted,
                claim,
                SessionSynthesisClaimStatus::Contradicted,
            );
        }
        for claim in &report.unsupported_claims {
            merge_claim(
                &mut unsupported,
                claim,
                SessionSynthesisClaimStatus::InsufficientEvidence,
            );
        }
        for claim in &report.needs_more_browsing_claims {
            merge_claim(
                &mut needs_more_browsing,
                claim,
                SessionSynthesisClaimStatus::NeedsMoreBrowsing,
            );
        }
    }

    let into_claims = |aggregates: BTreeMap<(String, String), AggregateClaim>| {
        aggregates
            .into_values()
            .map(|aggregate| SessionSynthesisClaim {
                version: CONTRACT_VERSION.to_string(),
                claim_id: aggregate.claim_id,
                statement: aggregate.statement,
                status: aggregate.status,
                snapshot_ids: aggregate.snapshot_ids.into_iter().collect(),
                support_refs: aggregate.support_refs.into_iter().collect(),
                citations: aggregate.citations,
            })
            .collect::<Vec<_>>()
    };

    SessionSynthesisReport {
        version: CONTRACT_VERSION.to_string(),
        session_id: session_id.to_string(),
        generated_at,
        snapshot_count,
        evidence_report_count,
        visited_urls: visited_urls.into_iter().collect(),
        working_set_refs: working_set_refs.into_iter().collect(),
        synthesized_notes,
        supported_claims: into_claims(supported),
        contradicted_claims: into_claims(contradicted),
        unsupported_claims: into_claims(unsupported),
        needs_more_browsing_claims: into_claims(needs_more_browsing),
    }
}
