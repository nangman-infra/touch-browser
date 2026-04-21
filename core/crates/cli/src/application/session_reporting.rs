use std::collections::BTreeMap;

use super::{
    deps::{
        ActionResult, ClaimInput, CliError, EvidenceClaimOutcome, EvidenceClaimVerdict,
        EvidenceReport, EvidenceVerificationVerdict, ReadOnlySession, RuntimeError,
        UnsupportedClaimReason,
    },
    ports::EvidenceVerifierPort,
};

pub(crate) fn verify_action_result_if_requested(
    verifier: &dyn EvidenceVerifierPort,
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
    let mut report = report;
    report.verification =
        Some(verifier.run_verifier(verifier_command, claims, &snapshot, &report, generated_at)?);
    apply_verifier_adjudication(&mut report);
    replace_latest_evidence_report(session, &report)?;
    action_result.output = Some(serde_json::to_value(report)?);
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
            support_score: Some(claim.support_score),
            citation: Some(claim.citation.clone()),
            primary_support_snippet: claim
                .primary_support_snippet
                .clone()
                .or_else(|| claim.support_snippets.first().cloned()),
            support_snippets: claim.support_snippets.clone(),
            reason: None,
            confidence_band: None,
            review_recommended: false,
            verdict_explanation: None,
            match_signals: None,
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
            primary_support_snippet: None,
            support_snippets: Vec::new(),
            reason: Some(claim.reason.clone()),
            confidence_band: None,
            review_recommended: false,
            verdict_explanation: None,
            match_signals: None,
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
            primary_support_snippet: None,
            support_snippets: Vec::new(),
            reason: Some(claim.reason.clone()),
            confidence_band: None,
            review_recommended: false,
            verdict_explanation: None,
            match_signals: None,
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
            primary_support_snippet: None,
            support_snippets: Vec::new(),
            reason: Some(claim.reason.clone()),
            confidence_band: None,
            review_recommended: false,
            verdict_explanation: None,
            match_signals: None,
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
