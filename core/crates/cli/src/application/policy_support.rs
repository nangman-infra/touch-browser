use std::{collections::BTreeSet, path::Path};

use serde::Serialize;

use crate::*;

pub(crate) fn current_policy_with_allowlist(
    session: &ReadOnlySession,
    kernel: &PolicyKernel,
    allowlisted_domains: &[String],
) -> Option<PolicyReport> {
    session.current_snapshot_record().map(|record| {
        kernel.evaluate_snapshot_with_allowlist(
            &record.snapshot,
            record.source_risk.clone(),
            allowlisted_domains,
        )
    })
}

pub(crate) fn succeed_action<T: Serialize>(
    action: ActionName,
    payload_type: &str,
    output: T,
    message: &str,
    policy: Option<PolicyReport>,
) -> Result<ActionResult, CliError> {
    Ok(ActionResult {
        version: CONTRACT_VERSION.to_string(),
        action,
        status: ActionStatus::Succeeded,
        payload_type: payload_type.to_string(),
        output: Some(serde_json::to_value(output)?),
        policy,
        failure_kind: None,
        message: message.to_string(),
    })
}

pub(crate) fn fail_action(
    action: ActionName,
    failure_kind: ActionFailureKind,
    message: &str,
    policy: Option<PolicyReport>,
) -> ActionResult {
    ActionResult {
        version: CONTRACT_VERSION.to_string(),
        action,
        status: ActionStatus::Failed,
        payload_type: "none".to_string(),
        output: None,
        policy,
        failure_kind: Some(failure_kind),
        message: message.to_string(),
    }
}

pub(crate) fn reject_action(
    action: ActionName,
    failure_kind: ActionFailureKind,
    message: &str,
    policy: Option<PolicyReport>,
) -> ActionResult {
    ActionResult {
        version: CONTRACT_VERSION.to_string(),
        action,
        status: ActionStatus::Rejected,
        payload_type: "none".to_string(),
        output: None,
        policy,
        failure_kind: Some(failure_kind),
        message: message.to_string(),
    }
}

pub(crate) fn ack_risk_label(ack_risk: AckRisk) -> &'static str {
    match ack_risk {
        AckRisk::Challenge => "challenge",
        AckRisk::Mfa => "mfa",
        AckRisk::Auth => "auth",
        AckRisk::HighRiskWrite => "high-risk-write",
    }
}

pub(crate) fn approved_risk_labels(approved_risks: &BTreeSet<AckRisk>) -> Vec<String> {
    approved_risks
        .iter()
        .map(|risk| ack_risk_label(*risk).to_string())
        .collect()
}

pub(crate) fn has_ack_risk(
    ack_risks: &[AckRisk],
    approved_risks: &BTreeSet<AckRisk>,
    expected: AckRisk,
) -> bool {
    ack_risks.contains(&expected) || approved_risks.contains(&expected)
}

pub(crate) fn merge_ack_risks(
    ack_risks: &[AckRisk],
    approved_risks: &BTreeSet<AckRisk>,
) -> Vec<AckRisk> {
    let mut merged = approved_risks.iter().copied().collect::<Vec<_>>();
    for ack_risk in ack_risks {
        if !merged.contains(ack_risk) {
            merged.push(*ack_risk);
        }
    }
    merged
}

pub(crate) fn policy_profile_label(profile: PolicyProfile) -> &'static str {
    match profile {
        PolicyProfile::ResearchReadOnly => "research-read-only",
        PolicyProfile::ResearchRestricted => "research-restricted",
        PolicyProfile::InteractiveReview => "interactive-review",
        PolicyProfile::InteractiveSupervisedAuth => "interactive-supervised-auth",
        PolicyProfile::InteractiveSupervisedWrite => "interactive-supervised-write",
    }
}

pub(crate) fn recommended_policy_profile(policy: &PolicyReport) -> PolicyProfile {
    if policy
        .signals
        .iter()
        .any(|signal| signal.kind == touch_browser_contracts::PolicySignalKind::HighRiskWrite)
    {
        return PolicyProfile::InteractiveSupervisedWrite;
    }

    if policy.signals.iter().any(|signal| {
        matches!(
            signal.kind,
            touch_browser_contracts::PolicySignalKind::BotChallenge
                | touch_browser_contracts::PolicySignalKind::MfaChallenge
                | touch_browser_contracts::PolicySignalKind::SensitiveAuthFlow
        )
    }) {
        return PolicyProfile::InteractiveSupervisedAuth;
    }

    PolicyProfile::InteractiveReview
}

pub(crate) fn promoted_policy_profile_for_risks(
    current: PolicyProfile,
    approved_risks: &BTreeSet<AckRisk>,
) -> PolicyProfile {
    if approved_risks.contains(&AckRisk::HighRiskWrite) {
        return PolicyProfile::InteractiveSupervisedWrite;
    }

    if approved_risks.contains(&AckRisk::Challenge)
        || approved_risks.contains(&AckRisk::Mfa)
        || approved_risks.contains(&AckRisk::Auth)
    {
        return match current {
            PolicyProfile::InteractiveSupervisedWrite => PolicyProfile::InteractiveSupervisedWrite,
            _ => PolicyProfile::InteractiveSupervisedAuth,
        };
    }

    current
}

pub(crate) fn required_ack_risks(policy: &PolicyReport) -> Vec<String> {
    let mut risks = BTreeSet::new();
    for signal in &policy.signals {
        match signal.kind {
            touch_browser_contracts::PolicySignalKind::BotChallenge => {
                risks.insert(AckRisk::Challenge);
            }
            touch_browser_contracts::PolicySignalKind::MfaChallenge => {
                risks.insert(AckRisk::Mfa);
            }
            touch_browser_contracts::PolicySignalKind::SensitiveAuthFlow => {
                risks.insert(AckRisk::Auth);
            }
            touch_browser_contracts::PolicySignalKind::HighRiskWrite => {
                risks.insert(AckRisk::HighRiskWrite);
            }
            _ => {}
        }
    }

    risks
        .into_iter()
        .map(|risk| ack_risk_label(risk).to_string())
        .collect()
}

pub(crate) fn checkpoint_provider_hints(
    snapshot: &SnapshotDocument,
    policy: &PolicyReport,
) -> Vec<String> {
    let source_url = snapshot.source.source_url.to_ascii_lowercase();
    let mut hints = BTreeSet::new();

    if policy
        .signals
        .iter()
        .any(|signal| signal.kind == touch_browser_contracts::PolicySignalKind::SensitiveAuthFlow)
    {
        if source_url.contains("github.com") {
            hints.insert("github-auth".to_string());
        } else if source_url.contains("accounts.google.com") || source_url.contains("google.com") {
            hints.insert("google-auth".to_string());
        } else if source_url.contains("login.microsoftonline.com")
            || source_url.contains("microsoft.com")
        {
            hints.insert("microsoft-auth".to_string());
        } else if source_url.contains("okta.") {
            hints.insert("okta-auth".to_string());
        } else if source_url.contains("auth0.") {
            hints.insert("auth0-auth".to_string());
        } else {
            hints.insert("generic-auth".to_string());
        }
    }

    if policy
        .signals
        .iter()
        .any(|signal| signal.kind == touch_browser_contracts::PolicySignalKind::BotChallenge)
    {
        if source_url.contains("google.com")
            && snapshot
                .source
                .title
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .contains("recaptcha")
        {
            hints.insert("google-recaptcha".to_string());
        } else {
            hints.insert("generic-bot-challenge".to_string());
        }
    }

    if policy
        .signals
        .iter()
        .any(|signal| signal.kind == touch_browser_contracts::PolicySignalKind::HighRiskWrite)
    {
        if source_url.contains("stripe") {
            hints.insert("stripe-checkout-like".to_string());
        } else if source_url.contains("paypal") {
            hints.insert("paypal-checkout-like".to_string());
        } else if source_url.contains("shopify") {
            hints.insert("shopify-checkout-like".to_string());
        } else {
            hints.insert("generic-high-risk-write".to_string());
        }
    }

    hints.into_iter().collect()
}

pub(crate) fn checkpoint_approval_panel(
    provider_hints: &[String],
    required_ack_risks: &[String],
    approved_risks: &[String],
    active_profile: PolicyProfile,
    recommended_profile: PolicyProfile,
    policy: &PolicyReport,
) -> CheckpointApprovalPanel {
    let severity = match policy.decision {
        touch_browser_contracts::PolicyDecision::Block => "block",
        touch_browser_contracts::PolicyDecision::Review => "review",
        touch_browser_contracts::PolicyDecision::Allow => "allow",
    };
    let mut actions = vec![CheckpointAction {
        id: "refresh".to_string(),
        label: "Refresh after manual continuation".to_string(),
        command: "refresh".to_string(),
        required_ack_risks: Vec::new(),
    }];

    if !required_ack_risks.is_empty() {
        actions.insert(
            0,
            CheckpointAction {
                id: "approve".to_string(),
                label: "Approve supervised continuation".to_string(),
                command: "approve".to_string(),
                required_ack_risks: required_ack_risks.to_vec(),
            },
        );
    }

    if required_ack_risks
        .iter()
        .any(|risk| risk == "auth" || risk == "mfa")
    {
        actions.push(CheckpointAction {
            id: "store-secret".to_string(),
            label: "Store daemon secret for supervised auth".to_string(),
            command: "secret.store".to_string(),
            required_ack_risks: Vec::new(),
        });
    }

    CheckpointApprovalPanel {
        title: "Supervised continuation required".to_string(),
        severity: severity.to_string(),
        provider: provider_hints
            .first()
            .cloned()
            .unwrap_or_else(|| "generic".to_string()),
        active_policy_profile: policy_profile_label(active_profile).to_string(),
        recommended_policy_profile: policy_profile_label(recommended_profile).to_string(),
        required_ack_risks: required_ack_risks.to_vec(),
        approved_risks: approved_risks.to_vec(),
        actions,
    }
}

pub(crate) fn checkpoint_playbook(
    provider_hints: &[String],
    required_ack_risks: &[String],
    approved_risks: &[String],
    snapshot: &SnapshotDocument,
    recommended_profile: PolicyProfile,
) -> CheckpointPlaybook {
    let provider = provider_hints
        .first()
        .cloned()
        .unwrap_or_else(|| "generic".to_string());
    let mut steps = match provider.as_str() {
        "github-auth" => vec![
            "Open the session in headed mode and continue on the GitHub login surface.".to_string(),
            "Type username/password or store the password with the daemon secret store.".to_string(),
            "Approve auth supervision before submit, then refresh after the page advances.".to_string(),
        ],
        "google-auth" => vec![
            "Keep the browser headed and complete the Google account challenge on the live page.".to_string(),
            "If prompted for OTP or device approval, complete the checkpoint manually and refresh.".to_string(),
            "Approve auth or MFA supervision before retrying submit.".to_string(),
        ],
        "microsoft-auth" | "okta-auth" | "auth0-auth" | "generic-auth" => vec![
            "Continue only in headed mode on the live authentication provider.".to_string(),
            "Store sensitive secrets in daemon memory rather than plain CLI arguments.".to_string(),
            "Approve auth or MFA supervision before submit, then refresh once the provider advances.".to_string(),
        ],
        "google-recaptcha" | "generic-bot-challenge" => vec![
            "Switch to headed mode and complete the visible human verification checkpoint.".to_string(),
            "Do not retry automated clicks until the challenge is cleared.".to_string(),
            "Approve challenge supervision and refresh the captured state after the page advances.".to_string(),
        ],
        "stripe-checkout-like" | "paypal-checkout-like" | "shopify-checkout-like" | "generic-high-risk-write" => vec![
            "Treat the next step as a supervised write boundary.".to_string(),
            "Review the target form/button and confirm the exact side effect before approval.".to_string(),
            "Approve high-risk write only for the intended action, then refresh immediately after completion.".to_string(),
        ],
        _ => vec![
            "Continue in headed mode when the page requires manual supervision.".to_string(),
            "Use approve to persist supervised risk acknowledgements for this session.".to_string(),
            "Refresh the browser-backed snapshot after the live page changes.".to_string(),
        ],
    };

    if required_ack_risks.iter().any(|risk| risk == "mfa") {
        steps.push("If an OTP or verification code is required, store it through the daemon secret store and use typeSecret.".to_string());
    }

    let sensitive_targets = snapshot
        .blocks
        .iter()
        .filter(|block| currentish_block_is_sensitive(block))
        .take(6)
        .map(|block| CheckpointSensitiveTarget {
            r#ref: block.stable_ref.clone(),
            text: block.text.clone(),
        })
        .collect::<Vec<_>>();

    CheckpointPlaybook {
        provider,
        recommended_policy_profile: policy_profile_label(recommended_profile).to_string(),
        required_ack_risks: required_ack_risks.to_vec(),
        approved_risks: approved_risks.to_vec(),
        steps,
        sensitive_targets,
    }
}

pub(crate) fn checkpoint_candidates(snapshot: &SnapshotDocument) -> Vec<CheckpointCandidate> {
    snapshot
        .blocks
        .iter()
        .filter(|block| {
            matches!(
                block.kind,
                touch_browser_contracts::SnapshotBlockKind::Form
                    | touch_browser_contracts::SnapshotBlockKind::Input
                    | touch_browser_contracts::SnapshotBlockKind::Button
                    | touch_browser_contracts::SnapshotBlockKind::Link
            )
        })
        .take(12)
        .map(|block| CheckpointCandidate {
            kind: block.kind.clone(),
            r#ref: block.stable_ref.clone(),
            text: block.text.clone(),
        })
        .collect()
}

fn currentish_block_is_sensitive(block: &SnapshotBlock) -> bool {
    let text = block.text.to_ascii_lowercase();
    let name = block
        .attributes
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let input_type = block
        .attributes
        .get("inputType")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    input_type == "password"
        || name.contains("pass")
        || name == "otp"
        || name.ends_with("_otp")
        || name.contains("token")
        || name.contains("verification")
        || text.contains("password")
        || text == "otp"
        || text.contains("verification")
}

fn policy_requires_ack(
    session: &ReadOnlySession,
    policy: &PolicyReport,
    action: ActionName,
    target_ref: Option<&str>,
) -> Option<(AckRisk, &'static str)> {
    for signal in &policy.signals {
        let matches_target = target_ref.is_none()
            || signal.stable_ref.as_deref() == target_ref
            || signal.stable_ref.is_none();

        if !matches_target {
            continue;
        }

        match signal.kind {
            touch_browser_contracts::PolicySignalKind::BotChallenge => {
                return Some((
                    AckRisk::Challenge,
                    "Detected a likely bot or CAPTCHA checkpoint. Re-open in headed mode, complete the human checkpoint manually, then retry with `--ack-risk challenge` or run `refresh` after the page advances.",
                ));
            }
            touch_browser_contracts::PolicySignalKind::MfaChallenge => {
                return Some((
                    AckRisk::Mfa,
                    "Detected a likely MFA or verification checkpoint. Use headed mode, complete the challenge manually or provide a daemon secret, then retry with `--ack-risk mfa`.",
                ));
            }
            touch_browser_contracts::PolicySignalKind::SensitiveAuthFlow => {
                let requires_ack = action != ActionName::Type
                    || target_ref.is_some_and(|target_ref| {
                        current_snapshot_ref_is_sensitive(session, target_ref)
                    });
                if requires_ack {
                    return Some((
                        AckRisk::Auth,
                        "Detected a credential-bearing authentication flow. Continue only in headed mode with explicit `--ack-risk auth`.",
                    ));
                }
            }
            touch_browser_contracts::PolicySignalKind::HighRiskWrite => {
                if matches!(action, ActionName::Click | ActionName::Submit) {
                    return Some((
                        AckRisk::HighRiskWrite,
                        "Detected a high-risk write action. Continue only in headed mode with explicit `--ack-risk high-risk-write`.",
                    ));
                }
            }
            _ => {}
        }
    }

    None
}

pub(crate) fn preflight_ref_action(
    persisted: &BrowserCliSession,
    kernel: &PolicyKernel,
    action: ActionName,
    target_ref: &str,
    message: &str,
    session_file: &Path,
) -> Option<SessionCommandOutput> {
    let policy =
        current_policy_with_allowlist(&persisted.session, kernel, &persisted.allowlisted_domains)?;
    if !policy
        .blocked_refs
        .iter()
        .any(|blocked| blocked == target_ref)
    {
        return None;
    }

    let action_result = reject_action(
        action,
        ActionFailureKind::PolicyBlocked,
        message,
        Some(policy),
    );

    Some(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state.clone(),
        session_file: session_file.display().to_string(),
    })
}

pub(crate) fn preflight_session_block(
    persisted: &BrowserCliSession,
    kernel: &PolicyKernel,
    action: ActionName,
    message: &str,
    session_file: &Path,
) -> Option<SessionCommandOutput> {
    let policy =
        current_policy_with_allowlist(&persisted.session, kernel, &persisted.allowlisted_domains)?;
    if policy.decision != touch_browser_contracts::PolicyDecision::Block {
        return None;
    }

    if action == ActionName::Paginate
        && policy.source_risk == SourceRisk::Low
        && !policy.signals.is_empty()
        && policy.signals.iter().all(|signal| {
            signal.kind == touch_browser_contracts::PolicySignalKind::DomainNotAllowlisted
        })
    {
        return None;
    }

    let action_result = reject_action(
        action,
        ActionFailureKind::PolicyBlocked,
        message,
        Some(policy),
    );

    Some(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state.clone(),
        session_file: session_file.display().to_string(),
    })
}

pub(crate) struct InteractivePreflightOptions<'a> {
    pub(crate) action: ActionName,
    pub(crate) target_ref: Option<&'a str>,
    pub(crate) headed: bool,
    pub(crate) ack_risks: &'a [AckRisk],
    pub(crate) message: &'a str,
    pub(crate) session_file: &'a Path,
}

pub(crate) fn preflight_interactive_action(
    persisted: &BrowserCliSession,
    kernel: &PolicyKernel,
    options: InteractivePreflightOptions<'_>,
) -> Option<SessionCommandOutput> {
    let InteractivePreflightOptions {
        action,
        target_ref,
        headed,
        ack_risks,
        message,
        session_file,
    } = options;
    let policy =
        current_policy_with_allowlist(&persisted.session, kernel, &persisted.allowlisted_domains)?;

    if persisted.allowlisted_domains.is_empty() {
        let action_result = reject_action(
            action,
            ActionFailureKind::PolicyBlocked,
            "Interactive browser actions require at least one `--allow-domain` boundary.",
            Some(policy),
        );
        return Some(SessionCommandOutput {
            action: action_result.clone(),
            result: action_result,
            session_state: persisted.session.state.clone(),
            session_file: session_file.display().to_string(),
        });
    }

    if policy.signals.iter().any(|signal| {
        signal.kind == touch_browser_contracts::PolicySignalKind::DomainNotAllowlisted
            && signal.stable_ref.is_none()
    }) {
        let action_result = reject_action(
            action,
            ActionFailureKind::PolicyBlocked,
            "Interactive browser actions require the current page host to be inside the allowlist.",
            Some(policy),
        );
        return Some(SessionCommandOutput {
            action: action_result.clone(),
            result: action_result,
            session_state: persisted.session.state.clone(),
            session_file: session_file.display().to_string(),
        });
    }

    if policy.source_risk == SourceRisk::Hostile {
        let action_result = reject_action(
            action,
            ActionFailureKind::PolicyBlocked,
            "Interactive browser actions are blocked on hostile sources.",
            Some(policy),
        );
        return Some(SessionCommandOutput {
            action: action_result.clone(),
            result: action_result,
            session_state: persisted.session.state.clone(),
            session_file: session_file.display().to_string(),
        });
    }

    if let Some((required_ack, detail)) =
        policy_requires_ack(&persisted.session, &policy, action.clone(), target_ref)
    {
        let requires_headed = persisted
            .session
            .current_snapshot_record()
            .map(|record| record.snapshot.source.source_url.as_str())
            .is_none_or(|source_url| !is_fixture_target(source_url));
        if requires_headed && !headed {
            let action_result = reject_action(
                action,
                ActionFailureKind::PolicyBlocked,
                &format!("{detail} Headed mode is required for supervised continuation."),
                Some(policy),
            );
            return Some(SessionCommandOutput {
                action: action_result.clone(),
                result: action_result,
                session_state: persisted.session.state.clone(),
                session_file: session_file.display().to_string(),
            });
        }

        if !has_ack_risk(ack_risks, &persisted.approved_risks, required_ack) {
            let action_result = reject_action(
                action,
                ActionFailureKind::PolicyBlocked,
                &format!(
                    "{detail} Re-run with `--ack-risk {}` once you want to cross this boundary.",
                    ack_risk_label(required_ack)
                ),
                Some(policy),
            );
            return Some(SessionCommandOutput {
                action: action_result.clone(),
                result: action_result,
                session_state: persisted.session.state.clone(),
                session_file: session_file.display().to_string(),
            });
        }
    }

    if let Some(target_ref) = target_ref {
        if policy
            .blocked_refs
            .iter()
            .any(|blocked| blocked == target_ref)
        {
            let action_result = reject_action(
                action,
                ActionFailureKind::PolicyBlocked,
                message,
                Some(policy),
            );
            return Some(SessionCommandOutput {
                action: action_result.clone(),
                result: action_result,
                session_state: persisted.session.state.clone(),
                session_file: session_file.display().to_string(),
            });
        }
    }

    None
}
