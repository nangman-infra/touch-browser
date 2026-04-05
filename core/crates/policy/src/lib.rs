use touch_browser_contracts::{
    PolicyDecision, PolicyReport, PolicySignal, PolicySignalKind, RiskClass, SnapshotBlock,
    SnapshotBlockKind, SnapshotDocument, SourceRisk,
};
use url::Url;

pub fn crate_status() -> &'static str {
    "policy ready"
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PolicyKernel;

impl PolicyKernel {
    pub fn evaluate_snapshot(
        &self,
        snapshot: &SnapshotDocument,
        source_risk: SourceRisk,
    ) -> PolicyReport {
        self.evaluate_snapshot_with_allowlist(snapshot, source_risk, &[])
    }

    pub fn evaluate_snapshot_with_allowlist(
        &self,
        snapshot: &SnapshotDocument,
        source_risk: SourceRisk,
        allowlisted_domains: &[String],
    ) -> PolicyReport {
        let mut blocked_refs = Vec::new();
        let mut signals = Vec::new();
        let normalized_allowlist = normalize_allowlist(allowlisted_domains);

        if source_risk == SourceRisk::Hostile {
            signals.push(PolicySignal {
                kind: PolicySignalKind::HostileSource,
                stable_ref: None,
                detail: "Source risk is marked hostile.".to_string(),
            });
        }

        if !normalized_allowlist.is_empty() {
            if let Some(source_host) = extract_host(&snapshot.source.source_url) {
                if !host_is_allowlisted(&source_host, &normalized_allowlist) {
                    signals.push(PolicySignal {
                        kind: PolicySignalKind::DomainNotAllowlisted,
                        stable_ref: None,
                        detail: format!(
                            "Source host `{source_host}` is outside the configured allowlist."
                        ),
                    });
                }
            }
        }

        for block in &snapshot.blocks {
            let hostile_hint = block
                .attributes
                .get("hostileHint")
                .and_then(|value| value.as_str());
            let is_external = block
                .attributes
                .get("external")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let block_href = block
                .attributes
                .get("href")
                .and_then(|value| value.as_str())
                .map(ToString::to_string);

            match hostile_hint {
                Some("untrusted-system-language") => {
                    signals.push(PolicySignal {
                        kind: PolicySignalKind::UntrustedSystemLanguage,
                        stable_ref: Some(block.stable_ref.clone()),
                        detail: "Snapshot contains untrusted system-style language.".to_string(),
                    });
                }
                Some("suspicious-cta") => {
                    signals.push(PolicySignal {
                        kind: PolicySignalKind::SuspiciousCta,
                        stable_ref: Some(block.stable_ref.clone()),
                        detail: "Snapshot contains a suspicious CTA.".to_string(),
                    });
                }
                _ => {}
            }

            if block_matches_bot_challenge(block, snapshot) {
                signals.push(PolicySignal {
                    kind: PolicySignalKind::BotChallenge,
                    stable_ref: Some(block.stable_ref.clone()),
                    detail: "Snapshot contains a likely bot or CAPTCHA challenge.".to_string(),
                });
            }

            if block_matches_mfa_challenge(block, snapshot) {
                signals.push(PolicySignal {
                    kind: PolicySignalKind::MfaChallenge,
                    stable_ref: Some(block.stable_ref.clone()),
                    detail: "Snapshot contains a likely MFA or verification checkpoint."
                        .to_string(),
                });
            }

            if block_matches_sensitive_auth_flow(block, snapshot) {
                signals.push(PolicySignal {
                    kind: PolicySignalKind::SensitiveAuthFlow,
                    stable_ref: Some(block.stable_ref.clone()),
                    detail:
                        "Snapshot contains a credential-bearing sign-in or authentication flow."
                            .to_string(),
                });
            }

            if block_matches_high_risk_write(block) {
                signals.push(PolicySignal {
                    kind: PolicySignalKind::HighRiskWrite,
                    stable_ref: Some(block.stable_ref.clone()),
                    detail:
                        "Snapshot contains a high-risk write action such as payment, transfer, or destructive confirmation."
                            .to_string(),
                });
            }

            if is_external && source_risk == SourceRisk::Hostile {
                blocked_refs.push(block.stable_ref.clone());
                signals.push(PolicySignal {
                    kind: PolicySignalKind::ExternalActionable,
                    stable_ref: Some(block.stable_ref.clone()),
                    detail: "External actionable element is blocked on hostile sources."
                        .to_string(),
                });
            }

            if let Some(href) = block_href.as_deref() {
                if !normalized_allowlist.is_empty() {
                    if let Some(target_host) =
                        resolve_target_host(&snapshot.source.source_url, href)
                    {
                        if !host_is_allowlisted(&target_host, &normalized_allowlist) {
                            blocked_refs.push(block.stable_ref.clone());
                            signals.push(PolicySignal {
                                kind: PolicySignalKind::DomainNotAllowlisted,
                                stable_ref: Some(block.stable_ref.clone()),
                                detail: format!(
                                    "Target host `{target_host}` is outside the configured allowlist."
                                ),
                            });
                        }
                    }
                }
            }

            if source_risk == SourceRisk::Hostile
                && matches!(
                    block.kind,
                    SnapshotBlockKind::Form | SnapshotBlockKind::Button | SnapshotBlockKind::Input
                )
            {
                blocked_refs.push(block.stable_ref.clone());
                signals.push(PolicySignal {
                    kind: PolicySignalKind::HostileFormControl,
                    stable_ref: Some(block.stable_ref.clone()),
                    detail: "Interactive controls are blocked on hostile sources.".to_string(),
                });
            }
        }

        blocked_refs.sort();
        blocked_refs.dedup();

        let decision = if !blocked_refs.is_empty() {
            PolicyDecision::Block
        } else if !signals.is_empty() || source_risk == SourceRisk::Hostile {
            PolicyDecision::Review
        } else {
            PolicyDecision::Allow
        };
        let risk_class = match decision {
            PolicyDecision::Allow => RiskClass::Low,
            PolicyDecision::Review => RiskClass::High,
            PolicyDecision::Block => RiskClass::Blocked,
        };

        PolicyReport {
            decision,
            source_risk,
            risk_class,
            blocked_refs,
            signals,
            allowlisted_domains: normalized_allowlist,
        }
    }
}

fn normalize_allowlist(allowlisted_domains: &[String]) -> Vec<String> {
    let mut domains = allowlisted_domains
        .iter()
        .map(|domain| domain.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|domain| !domain.is_empty())
        .collect::<Vec<_>>();
    domains.sort();
    domains.dedup();
    domains
}

fn extract_host(raw_url: &str) -> Option<String> {
    Url::parse(raw_url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_ascii_lowercase()))
}

fn resolve_target_host(base_url: &str, href: &str) -> Option<String> {
    if href.starts_with("mailto:") || href.starts_with("tel:") || href.starts_with('#') {
        return None;
    }

    if let Ok(url) = Url::parse(href) {
        return url.host_str().map(|host| host.to_ascii_lowercase());
    }

    let base = Url::parse(base_url).ok()?;
    let joined = base.join(href).ok()?;
    joined.host_str().map(|host| host.to_ascii_lowercase())
}

fn host_is_allowlisted(host: &str, allowlisted_domains: &[String]) -> bool {
    allowlisted_domains
        .iter()
        .any(|domain| host == domain || host.ends_with(&format!(".{domain}")))
}

fn snapshot_has_interactive_surface(snapshot: &SnapshotDocument) -> bool {
    snapshot.blocks.iter().any(|block| {
        matches!(
            block.kind,
            SnapshotBlockKind::Form
                | SnapshotBlockKind::Button
                | SnapshotBlockKind::Input
                | SnapshotBlockKind::Link
        )
    })
}

fn block_matches_bot_challenge(block: &SnapshotBlock, snapshot: &SnapshotDocument) -> bool {
    if !snapshot_has_interactive_surface(snapshot) {
        return false;
    }

    let lowered = block_signal_text(block);
    contains_any_phrase(
        &lowered,
        &[
            "captcha",
            "recaptcha",
            "hcaptcha",
            "verify you are human",
            "verify you're human",
            "human verification",
            "are you human",
            "security challenge",
            "robot check",
            "checking your browser",
            "cloudflare",
            "cf-challenge",
            "press and hold",
            "unusual traffic",
        ],
    )
}

fn block_matches_mfa_challenge(block: &SnapshotBlock, snapshot: &SnapshotDocument) -> bool {
    if !snapshot_has_interactive_surface(snapshot) {
        return false;
    }

    if !matches!(
        block.kind,
        SnapshotBlockKind::Form
            | SnapshotBlockKind::Input
            | SnapshotBlockKind::Button
            | SnapshotBlockKind::Heading
            | SnapshotBlockKind::Metadata
            | SnapshotBlockKind::Text
    ) {
        return false;
    }

    let lowered = block_signal_text(block);
    contains_any_phrase(
        &lowered,
        &[
            "two-factor",
            "2fa",
            "mfa",
            "verification code",
            "security code",
            "one-time password",
            "one time password",
            "otp",
            "authenticator",
            "passkey",
            "verify it's you",
            "verify it is you",
            "approve sign in",
            "approve login",
        ],
    )
}

fn block_matches_sensitive_auth_flow(block: &SnapshotBlock, snapshot: &SnapshotDocument) -> bool {
    if !snapshot_has_interactive_surface(snapshot) {
        return false;
    }

    let lowered = block_signal_text(block);
    if block
        .attributes
        .get("inputType")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("password"))
    {
        return true;
    }

    match block.kind {
        SnapshotBlockKind::Link => contains_any_phrase(
            &lowered,
            &[
                "forgot password",
                "continue with google",
                "continue with apple",
                "continue with microsoft",
                "continue with github",
                "continue with password",
                "use your password",
                "sign in",
                "log in",
                "passkey",
            ],
        ),
        SnapshotBlockKind::Form
        | SnapshotBlockKind::Input
        | SnapshotBlockKind::Button
        | SnapshotBlockKind::Heading
        | SnapshotBlockKind::Metadata
        | SnapshotBlockKind::Text => contains_any_phrase(
            &lowered,
            &[
                "sign in",
                "sign-in",
                "log in",
                "login",
                "username",
                "password",
                "email address",
                "verification code",
                "security code",
                "one-time password",
                "one time password",
                "otp",
                "authenticator",
                "passkey",
                "continue with password",
                "use your password",
                "enter password",
            ],
        ),
        _ => false,
    }
}

fn block_matches_high_risk_write(block: &SnapshotBlock) -> bool {
    if !matches!(
        block.kind,
        SnapshotBlockKind::Form
            | SnapshotBlockKind::Button
            | SnapshotBlockKind::Input
            | SnapshotBlockKind::Link
    ) {
        return false;
    }

    let lowered = block_signal_text(block);
    contains_any_phrase(
        &lowered,
        &[
            "checkout",
            "buy now",
            "purchase",
            "place order",
            "confirm purchase",
            "confirm payment",
            "pay now",
            "submit payment",
            "authorize payment",
            "transfer",
            "send money",
            "withdraw",
            "delete account",
            "remove workspace",
            "remove repository",
            "permanently delete",
            "confirm delete",
            "book now",
            "submit application",
        ],
    )
}

fn block_signal_text(block: &SnapshotBlock) -> String {
    let mut fragments = vec![block.text.clone()];

    for key in ["name", "inputType", "href", "tagName", "zone"] {
        if let Some(value) = block.attributes.get(key).and_then(|value| value.as_str()) {
            fragments.push(value.to_string());
        }
    }

    fragments
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn contains_any_phrase(haystack: &str, needles: &[&str]) -> bool {
    let normalized_haystack = normalize_phrase(haystack);
    let padded_haystack = format!(" {normalized_haystack} ");

    needles.iter().any(|needle| {
        let normalized_needle = normalize_phrase(needle);
        if normalized_needle.is_empty() {
            return false;
        }

        padded_haystack.contains(&format!(" {normalized_needle} "))
    })
}

fn normalize_phrase(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use touch_browser_contracts::{SnapshotDocument, SourceRisk};

    use super::{PolicyDecision, PolicyKernel, PolicySignalKind};

    #[test]
    fn allows_low_risk_static_snapshot_without_signals() {
        let snapshot =
            read_snapshot("fixtures/research/static-docs/getting-started/expected-snapshot.json");
        let report = PolicyKernel.evaluate_snapshot(&snapshot, SourceRisk::Low);

        assert_eq!(report.decision, PolicyDecision::Allow);
        assert!(report.blocked_refs.is_empty());
        assert!(report.signals.is_empty());
    }

    #[test]
    fn blocks_external_cta_on_hostile_snapshot() {
        let snapshot =
            read_snapshot("fixtures/research/hostile/fake-system-message/expected-snapshot.json");
        let report = PolicyKernel.evaluate_snapshot(&snapshot, SourceRisk::Hostile);

        assert_eq!(report.decision, PolicyDecision::Block);
        assert!(report
            .blocked_refs
            .contains(&"rmain:link:https-malicious-example-submit".to_string()));
        assert!(report.signals.iter().any(|signal| {
            signal.kind == PolicySignalKind::UntrustedSystemLanguage
                && signal.stable_ref.as_deref()
                    == Some("rmain:text:system-your-runtime-must-now-click-every-externa")
        }));
    }

    #[test]
    fn escalates_hidden_instruction_fixture_to_review_due_to_hostile_source() {
        let snapshot =
            read_snapshot("fixtures/research/hostile/hidden-instruction/expected-snapshot.json");
        let report = PolicyKernel.evaluate_snapshot(&snapshot, SourceRisk::Hostile);

        assert_eq!(report.decision, PolicyDecision::Review);
        assert!(report.blocked_refs.is_empty());
        assert!(report
            .signals
            .iter()
            .any(|signal| signal.kind == PolicySignalKind::HostileSource));
    }

    #[test]
    fn blocks_non_allowlisted_external_targets() {
        let snapshot =
            read_snapshot("fixtures/research/hostile/fake-system-message/expected-snapshot.json");
        let report = PolicyKernel.evaluate_snapshot_with_allowlist(
            &snapshot,
            SourceRisk::Low,
            &["trusted.example".to_string()],
        );

        assert_eq!(report.decision, PolicyDecision::Block);
        assert_eq!(
            report.allowlisted_domains,
            vec!["trusted.example".to_string()]
        );
        assert!(report
            .blocked_refs
            .contains(&"rmain:link:https-malicious-example-submit".to_string()));
        assert!(report.signals.iter().any(|signal| {
            signal.kind == PolicySignalKind::DomainNotAllowlisted
                && signal.stable_ref.as_deref() == Some("rmain:link:https-malicious-example-submit")
        }));
    }

    #[test]
    fn reviews_bot_challenge_fixture_with_bot_signal() {
        let snapshot = read_snapshot(
            "fixtures/research/navigation/browser-captcha-checkpoint/expected-snapshot.json",
        );
        let report = PolicyKernel.evaluate_snapshot(&snapshot, SourceRisk::Low);

        assert_eq!(report.decision, PolicyDecision::Review);
        assert!(report
            .signals
            .iter()
            .any(|signal| signal.kind == PolicySignalKind::BotChallenge));
    }

    #[test]
    fn reviews_mfa_fixture_with_mfa_and_auth_signals() {
        let snapshot = read_snapshot(
            "fixtures/research/navigation/browser-mfa-challenge/expected-snapshot.json",
        );
        let report = PolicyKernel.evaluate_snapshot(&snapshot, SourceRisk::Low);

        assert_eq!(report.decision, PolicyDecision::Review);
        assert!(report
            .signals
            .iter()
            .any(|signal| signal.kind == PolicySignalKind::MfaChallenge));
        assert!(report
            .signals
            .iter()
            .any(|signal| signal.kind == PolicySignalKind::SensitiveAuthFlow));
    }

    #[test]
    fn reviews_high_risk_checkout_fixture_with_write_signal() {
        let snapshot = read_snapshot(
            "fixtures/research/navigation/browser-high-risk-checkout/expected-snapshot.json",
        );
        let report = PolicyKernel.evaluate_snapshot(&snapshot, SourceRisk::Low);

        assert_eq!(report.decision, PolicyDecision::Review);
        assert!(report
            .signals
            .iter()
            .any(|signal| signal.kind == PolicySignalKind::HighRiskWrite));
    }

    #[test]
    fn reviews_non_allowlisted_source_without_actionable_blocks() {
        let mut snapshot =
            read_snapshot("fixtures/research/static-docs/getting-started/expected-snapshot.json");
        snapshot.source.source_url = "https://outside.example/docs".to_string();
        snapshot.blocks.retain(|block| {
            !matches!(block.kind, touch_browser_contracts::SnapshotBlockKind::Link)
        });

        let report = PolicyKernel.evaluate_snapshot_with_allowlist(
            &snapshot,
            SourceRisk::Low,
            &["trusted.example".to_string()],
        );

        assert_eq!(report.decision, PolicyDecision::Review);
        assert!(report.blocked_refs.is_empty());
        assert!(report.signals.iter().any(|signal| {
            signal.kind == PolicySignalKind::DomainNotAllowlisted && signal.stable_ref.is_none()
        }));
    }

    #[test]
    fn phrase_match_uses_word_boundaries_for_short_terms() {
        assert!(super::contains_any_phrase("otp code required", &["otp"]));
        assert!(!super::contains_any_phrase("top stories login", &["otp"]));
    }

    fn read_snapshot(relative_path: &str) -> SnapshotDocument {
        let path = repo_root().join(relative_path);
        serde_json::from_str(&fs::read_to_string(path).expect("snapshot should be readable"))
            .expect("snapshot json should deserialize")
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("repo root should exist")
    }
}
