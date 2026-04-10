use std::{
    collections::BTreeMap,
    io::Write,
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};
use touch_browser_contracts::{
    EvidenceReport, EvidenceVerificationOutcome, EvidenceVerificationReport,
    EvidenceVerificationVerdict, SnapshotDocument, CONTRACT_VERSION,
};
use touch_browser_runtime::ClaimInput;

use crate::interface::{cli_error::CliError, cli_support::repo_root};

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

pub(crate) fn run_verifier_command(
    verifier_command: &str,
    claims: &[ClaimInput],
    snapshot: &SnapshotDocument,
    report: &EvidenceReport,
    generated_at: &str,
) -> Result<EvidenceVerificationReport, CliError> {
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

    Ok(EvidenceVerificationReport {
        version: CONTRACT_VERSION.to_string(),
        verifier: verifier_command.to_string(),
        generated_at: generated_at.to_string(),
        outcomes,
    })
}
