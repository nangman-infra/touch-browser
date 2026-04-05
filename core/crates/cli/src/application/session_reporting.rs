use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifierCommandRequest<'a> {
    version: &'static str,
    generated_at: &'a str,
    claims: &'a [ClaimInput],
    snapshot: &'a SnapshotDocument,
    evidence_report: &'a EvidenceReport,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerifierCommandResponse {
    #[serde(default)]
    outcomes: Vec<VerifierCommandOutcome>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerifierCommandOutcome {
    claim_id: String,
    verdict: EvidenceVerificationVerdict,
    #[serde(default)]
    verifier_score: Option<f64>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    statement: Option<String>,
}

pub(crate) fn verify_action_result_if_requested(
    mut action_result: ActionResult,
    session: &mut ReadOnlySession,
    claims: &[ClaimInput],
    verifier_command: Option<&str>,
    generated_at: &str,
) -> Result<ActionResult, CliError> {
    let Some(verifier_command) = verifier_command else {
        return Ok(action_result);
    };

    let output = action_result.output.take().ok_or_else(|| {
        CliError::Verifier(
            "Verifier requested but extract action had no output payload.".to_string(),
        )
    })?;
    let report: EvidenceReport = serde_json::from_value(output)?;
    let snapshot = session
        .current_snapshot_record()
        .ok_or(RuntimeError::NoCurrentSnapshot)?
        .snapshot
        .clone();
    let report = run_verifier_hook(verifier_command, claims, &snapshot, &report, generated_at)?;
    replace_latest_evidence_report(session, &report)?;
    action_result.output = Some(json!(report));
    Ok(action_result)
}

fn replace_latest_evidence_report(
    session: &mut ReadOnlySession,
    report: &EvidenceReport,
) -> Result<(), CliError> {
    let Some(record) = session.evidence_reports.last_mut() else {
        return Err(CliError::Verifier(
            "Verifier requested but the session has no evidence report to update.".to_string(),
        ));
    };

    record.report = report.clone();
    Ok(())
}

fn run_verifier_hook(
    verifier_command: &str,
    claims: &[ClaimInput],
    snapshot: &SnapshotDocument,
    report: &EvidenceReport,
    generated_at: &str,
) -> Result<EvidenceReport, CliError> {
    let request = VerifierCommandRequest {
        version: CONTRACT_VERSION,
        generated_at,
        claims,
        snapshot,
        evidence_report: report,
    };
    let request_body = serde_json::to_vec(&request)?;
    let mut child = Command::new("sh")
        .args(["-lc", verifier_command])
        .current_dir(repo_root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| CliError::Verifier("Failed to open verifier stdin.".to_string()))?;
        stdin.write_all(&request_body)?;
    }
    let _ = child.stdin.take();

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(CliError::Verifier(format!(
            "Verifier command failed with status {}: {detail}",
            output.status
        )));
    }

    let response: VerifierCommandResponse = serde_json::from_slice(&output.stdout)?;
    let statements = claims
        .iter()
        .map(|claim| (claim.id.as_str(), claim.statement.as_str()))
        .collect::<BTreeMap<_, _>>();
    let outcomes = response
        .outcomes
        .into_iter()
        .map(|outcome| {
            if let Some(score) = outcome.verifier_score {
                if !(0.0..=1.0).contains(&score) {
                    return Err(CliError::Verifier(format!(
                        "Verifier score for `{}` must be between 0 and 1.",
                        outcome.claim_id
                    )));
                }
            }

            let statement = outcome.statement.or_else(|| {
                statements
                    .get(outcome.claim_id.as_str())
                    .map(|statement| (*statement).to_string())
            });
            let statement = statement.ok_or_else(|| {
                CliError::Verifier(format!(
                    "Verifier returned unknown claim id `{}`.",
                    outcome.claim_id
                ))
            })?;

            Ok(EvidenceVerificationOutcome {
                version: CONTRACT_VERSION.to_string(),
                claim_id: outcome.claim_id,
                statement,
                verdict: outcome.verdict,
                verifier_score: outcome.verifier_score,
                notes: outcome.notes,
            })
        })
        .collect::<Result<Vec<_>, CliError>>()?;

    let mut verified = report.clone();
    verified.verification = Some(EvidenceVerificationReport {
        version: CONTRACT_VERSION.to_string(),
        verifier: verifier_command.to_string(),
        generated_at: generated_at.to_string(),
        outcomes,
    });
    apply_verifier_adjudication(&mut verified);
    Ok(verified)
}

fn apply_verifier_adjudication(report: &mut EvidenceReport) {
    if report.claim_outcomes.is_empty() {
        report.claim_outcomes = synthesize_claim_outcomes_from_report(report);
    }

    let verdicts = report
        .verification
        .as_ref()
        .map(|verification| {
            verification
                .outcomes
                .iter()
                .map(|outcome| (outcome.claim_id.as_str(), outcome.verdict.clone()))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    for claim in &mut report.claim_outcomes {
        let Some(verifier_verdict) = verdicts.get(claim.claim_id.as_str()) else {
            continue;
        };

        claim.verification_verdict = Some(verifier_verdict.clone());
        claim.verdict = map_final_claim_verdict(&claim.verdict, verifier_verdict);
        claim.reason = match claim.verdict {
            EvidenceClaimVerdict::EvidenceSupported => None,
            EvidenceClaimVerdict::Contradicted => claim
                .reason
                .clone()
                .or(Some(UnsupportedClaimReason::ContradictoryEvidence)),
            EvidenceClaimVerdict::InsufficientEvidence => claim
                .reason
                .clone()
                .filter(|reason| *reason != UnsupportedClaimReason::NeedsMoreBrowsing)
                .or(Some(UnsupportedClaimReason::InsufficientConfidence)),
            EvidenceClaimVerdict::NeedsMoreBrowsing => {
                Some(UnsupportedClaimReason::NeedsMoreBrowsing)
            }
        };

        if claim.verdict != EvidenceClaimVerdict::NeedsMoreBrowsing {
            claim.next_action_hint = None;
        }
    }

    report.rebuild_claim_buckets();
}

fn synthesize_claim_outcomes_from_report(report: &EvidenceReport) -> Vec<EvidenceClaimOutcome> {
    let mut outcomes = Vec::new();

    for claim in &report.supported_claims {
        outcomes.push(EvidenceClaimOutcome {
            version: claim.version.clone(),
            claim_id: claim.claim_id.clone(),
            statement: claim.statement.clone(),
            verdict: EvidenceClaimVerdict::EvidenceSupported,
            support: claim.support.clone(),
            support_score: Some(claim.confidence),
            citation: Some(claim.citation.clone()),
            reason: None,
            checked_block_refs: Vec::new(),
            guard_failures: Vec::new(),
            next_action_hint: None,
            verification_verdict: None,
        });
    }

    for claim in &report.contradicted_claims {
        outcomes.push(EvidenceClaimOutcome {
            version: claim.version.clone(),
            claim_id: claim.claim_id.clone(),
            statement: claim.statement.clone(),
            verdict: EvidenceClaimVerdict::Contradicted,
            support: Vec::new(),
            support_score: None,
            citation: None,
            reason: Some(claim.reason.clone()),
            checked_block_refs: claim.checked_block_refs.clone(),
            guard_failures: claim.guard_failures.clone(),
            next_action_hint: claim.next_action_hint.clone(),
            verification_verdict: None,
        });
    }

    for claim in &report.unsupported_claims {
        outcomes.push(EvidenceClaimOutcome {
            version: claim.version.clone(),
            claim_id: claim.claim_id.clone(),
            statement: claim.statement.clone(),
            verdict: EvidenceClaimVerdict::InsufficientEvidence,
            support: Vec::new(),
            support_score: None,
            citation: None,
            reason: Some(claim.reason.clone()),
            checked_block_refs: claim.checked_block_refs.clone(),
            guard_failures: claim.guard_failures.clone(),
            next_action_hint: claim.next_action_hint.clone(),
            verification_verdict: None,
        });
    }

    for claim in &report.needs_more_browsing_claims {
        outcomes.push(EvidenceClaimOutcome {
            version: claim.version.clone(),
            claim_id: claim.claim_id.clone(),
            statement: claim.statement.clone(),
            verdict: EvidenceClaimVerdict::NeedsMoreBrowsing,
            support: Vec::new(),
            support_score: None,
            citation: None,
            reason: Some(claim.reason.clone()),
            checked_block_refs: claim.checked_block_refs.clone(),
            guard_failures: claim.guard_failures.clone(),
            next_action_hint: claim.next_action_hint.clone(),
            verification_verdict: None,
        });
    }

    outcomes
}

fn map_final_claim_verdict(
    current: &EvidenceClaimVerdict,
    verifier: &EvidenceVerificationVerdict,
) -> EvidenceClaimVerdict {
    match verifier {
        EvidenceVerificationVerdict::Verified => EvidenceClaimVerdict::EvidenceSupported,
        EvidenceVerificationVerdict::Contradicted => EvidenceClaimVerdict::Contradicted,
        EvidenceVerificationVerdict::NeedsMoreBrowsing => EvidenceClaimVerdict::NeedsMoreBrowsing,
        EvidenceVerificationVerdict::InsufficientEvidence => {
            EvidenceClaimVerdict::InsufficientEvidence
        }
        EvidenceVerificationVerdict::Unresolved => {
            if *current == EvidenceClaimVerdict::EvidenceSupported {
                EvidenceClaimVerdict::NeedsMoreBrowsing
            } else {
                current.clone()
            }
        }
    }
}

pub(crate) fn render_session_synthesis_markdown(report: &SessionSynthesisReport) -> String {
    let mut sections = vec![
        "# Session Synthesis".to_string(),
        String::new(),
        format!("- Session ID: {}", report.session_id),
        format!("- Snapshots: {}", report.snapshot_count),
        format!("- Evidence Reports: {}", report.evidence_report_count),
    ];

    if !report.visited_urls.is_empty() {
        sections.push(format!(
            "- Visited URLs: {}",
            report.visited_urls.join(", ")
        ));
    }

    if !report.synthesized_notes.is_empty() {
        sections.push(String::new());
        sections.push("## Synthesized Notes".to_string());
        for note in &report.synthesized_notes {
            sections.push(format!("- {note}"));
        }
    }

    if !report.supported_claims.is_empty() {
        sections.push(String::new());
        sections.push("## Evidence-Supported Claims".to_string());
        for claim in &report.supported_claims {
            sections.push(render_session_claim_markdown(claim));
        }
    }

    if !report.contradicted_claims.is_empty() {
        sections.push(String::new());
        sections.push("## Contradicted Claims".to_string());
        for claim in &report.contradicted_claims {
            sections.push(render_session_claim_markdown(claim));
        }
    }

    if !report.unsupported_claims.is_empty() {
        sections.push(String::new());
        sections.push("## Insufficient Evidence Claims".to_string());
        for claim in &report.unsupported_claims {
            sections.push(render_session_claim_markdown(claim));
        }
    }

    if !report.needs_more_browsing_claims.is_empty() {
        sections.push(String::new());
        sections.push("## Needs More Browsing Claims".to_string());
        for claim in &report.needs_more_browsing_claims {
            sections.push(render_session_claim_markdown(claim));
        }
    }

    sections.join("\n")
}

fn render_session_claim_markdown(claim: &SessionSynthesisClaim) -> String {
    let mut lines = vec![format!("- {}", claim.statement)];

    if !claim.citations.is_empty() {
        let citations = claim
            .citations
            .iter()
            .map(|citation| citation.url.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        lines.push(format!("  Citations: {}", citations.join(", ")));
    }

    if !claim.support_refs.is_empty() {
        lines.push(format!("  Refs: {}", claim.support_refs.join(", ")));
    }

    lines.join("\n")
}
