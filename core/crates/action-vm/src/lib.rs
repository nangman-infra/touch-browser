use serde_json::json;
use touch_browser_contracts::{
    ActionCommand, ActionFailureKind, ActionName, ActionResult, ActionStatus, PolicyReport,
    CONTRACT_VERSION,
};
use touch_browser_policy::PolicyKernel;
use touch_browser_runtime::{
    ClaimInput, CompactInput, DiffInput, FixtureCatalog, ReadOnlyRuntime, ReadOnlySession,
    RuntimeError, SessionSnapshotRecord,
};

pub fn crate_status() -> &'static str {
    "action-vm ready"
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ReadOnlyActionVm {
    runtime: ReadOnlyRuntime,
    policy: PolicyKernel,
}

impl ReadOnlyActionVm {
    pub fn execute_fixture(
        &self,
        session: &mut ReadOnlySession,
        catalog: &FixtureCatalog,
        command: ActionCommand,
        timestamp: &str,
    ) -> ActionResult {
        match command.action {
            ActionName::Open => {
                self.execute_open(session, catalog, command.target_url.as_deref(), timestamp)
            }
            ActionName::Read => self.execute_read(session, timestamp),
            ActionName::Follow => {
                self.execute_follow(session, catalog, command.target_ref.as_deref(), timestamp)
            }
            ActionName::Extract => self.execute_extract(session, command.input.as_ref(), timestamp),
            ActionName::Diff => self.execute_diff(session, command.input.as_ref(), timestamp),
            ActionName::Compact => self.execute_compact(session, command.input.as_ref(), timestamp),
            blocked_action => self.reject_interactive_action(session, blocked_action),
        }
    }

    fn evaluate_record(&self, record: &SessionSnapshotRecord) -> PolicyReport {
        self.policy
            .evaluate_snapshot(&record.snapshot, record.source_risk.clone())
    }

    fn execute_open(
        &self,
        session: &mut ReadOnlySession,
        catalog: &FixtureCatalog,
        target_url: Option<&str>,
        timestamp: &str,
    ) -> ActionResult {
        let Some(target_url) = target_url else {
            return self.missing_target(
                session,
                ActionName::Open,
                "Open action requires targetUrl.",
            );
        };
        let result = self.runtime.open(session, catalog, target_url, timestamp);

        self.runtime_action(
            session,
            ActionName::Open,
            "snapshot-document",
            "Opened document.",
            result,
        )
    }

    fn execute_read(&self, session: &mut ReadOnlySession, timestamp: &str) -> ActionResult {
        let result = self.runtime.read(session, timestamp);
        self.runtime_action(
            session,
            ActionName::Read,
            "snapshot-document",
            "Read current snapshot.",
            result,
        )
    }

    fn execute_follow(
        &self,
        session: &mut ReadOnlySession,
        catalog: &FixtureCatalog,
        target_ref: Option<&str>,
        timestamp: &str,
    ) -> ActionResult {
        let Some(target_ref) = target_ref else {
            return self.missing_target(
                session,
                ActionName::Follow,
                "Follow action requires targetRef.",
            );
        };

        if let Some(rejection) = self.follow_policy_rejection(session, target_ref) {
            return rejection;
        }
        let result = self.runtime.follow(session, catalog, target_ref, timestamp);

        self.runtime_action(
            session,
            ActionName::Follow,
            "snapshot-document",
            "Followed link target.",
            result,
        )
    }

    fn execute_extract(
        &self,
        session: &mut ReadOnlySession,
        input: Option<&serde_json::Value>,
        timestamp: &str,
    ) -> ActionResult {
        let claims = match parse_claims(input) {
            Ok(claims) if !claims.is_empty() => claims,
            Ok(_) => {
                return self.invalid_input(
                    session,
                    ActionName::Extract,
                    "Extract action requires at least one claim.".to_string(),
                );
            }
            Err(error) => return self.invalid_input(session, ActionName::Extract, error),
        };
        let result = self.runtime.extract(session, claims, timestamp);

        self.runtime_action(
            session,
            ActionName::Extract,
            "evidence-report",
            "Extracted evidence report.",
            result,
        )
    }

    fn execute_diff(
        &self,
        session: &mut ReadOnlySession,
        input: Option<&serde_json::Value>,
        timestamp: &str,
    ) -> ActionResult {
        let input = match parse_diff(input) {
            Ok(input) => input,
            Err(error) => return self.invalid_input(session, ActionName::Diff, error),
        };
        let result = self.runtime.diff(session, input, timestamp);

        self.runtime_action(
            session,
            ActionName::Diff,
            "snapshot-diff",
            "Computed snapshot diff.",
            result,
        )
    }

    fn execute_compact(
        &self,
        session: &mut ReadOnlySession,
        input: Option<&serde_json::Value>,
        timestamp: &str,
    ) -> ActionResult {
        let input = match parse_compact(input) {
            Ok(input) => input,
            Err(error) => return self.invalid_input(session, ActionName::Compact, error),
        };
        let result = self.runtime.compact(session, input, timestamp);

        self.runtime_action(
            session,
            ActionName::Compact,
            "compaction-result",
            "Compacted working set.",
            result,
        )
    }

    fn runtime_action<T: serde::Serialize>(
        &self,
        session: &mut ReadOnlySession,
        action: ActionName,
        payload_type: &str,
        message: &str,
        result: Result<T, RuntimeError>,
    ) -> ActionResult {
        result
            .map(|output| {
                succeed(
                    action.clone(),
                    payload_type,
                    json!(output),
                    message,
                    self.current_action_policy(session),
                )
            })
            .unwrap_or_else(|error| {
                fail(
                    action.clone(),
                    classify_runtime_error(&error),
                    error.to_string(),
                    current_policy_report(session, &self.policy),
                )
            })
    }

    fn current_action_policy(&self, session: &ReadOnlySession) -> Option<PolicyReport> {
        session
            .current_snapshot_record()
            .map(|record| self.evaluate_record(record))
    }

    fn follow_policy_rejection(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Option<ActionResult> {
        let policy = current_policy_report(session, &self.policy)?;
        if policy
            .blocked_refs
            .iter()
            .all(|blocked| blocked != target_ref)
        {
            return None;
        }

        Some(reject(
            ActionName::Follow,
            ActionFailureKind::PolicyBlocked,
            format!("Follow target `{target_ref}` is blocked by policy."),
            Some(policy),
        ))
    }

    fn reject_interactive_action(
        &self,
        session: &ReadOnlySession,
        action: ActionName,
    ) -> ActionResult {
        reject(
            action,
            ActionFailureKind::PolicyBlocked,
            "Read-only action VM blocks interactive actions.".to_string(),
            current_policy_report(session, &self.policy),
        )
    }

    fn missing_target(
        &self,
        session: &ReadOnlySession,
        action: ActionName,
        message: &str,
    ) -> ActionResult {
        fail(
            action,
            ActionFailureKind::MissingTarget,
            message.to_string(),
            current_policy_report(session, &self.policy),
        )
    }

    fn invalid_input(
        &self,
        session: &ReadOnlySession,
        action: ActionName,
        message: String,
    ) -> ActionResult {
        fail(
            action,
            ActionFailureKind::InvalidInput,
            message,
            current_policy_report(session, &self.policy),
        )
    }
}

fn parse_claims(input: Option<&serde_json::Value>) -> Result<Vec<ClaimInput>, String> {
    let claims = input
        .and_then(|value| value.get("claims"))
        .cloned()
        .ok_or_else(|| "Extract action requires `input.claims`.".to_string())?;
    serde_json::from_value(claims).map_err(|error| error.to_string())
}

fn parse_diff(input: Option<&serde_json::Value>) -> Result<DiffInput, String> {
    let value = input
        .cloned()
        .ok_or_else(|| "Diff action requires input.".to_string())?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

fn parse_compact(input: Option<&serde_json::Value>) -> Result<CompactInput, String> {
    let value = input
        .cloned()
        .ok_or_else(|| "Compact action requires input.".to_string())?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

fn succeed(
    action: ActionName,
    payload_type: &str,
    output: serde_json::Value,
    message: &str,
    policy: Option<PolicyReport>,
) -> ActionResult {
    ActionResult {
        version: CONTRACT_VERSION.to_string(),
        action,
        status: ActionStatus::Succeeded,
        payload_type: payload_type.to_string(),
        output: Some(output),
        policy,
        failure_kind: None,
        message: message.to_string(),
    }
}

fn reject(
    action: ActionName,
    failure_kind: ActionFailureKind,
    message: String,
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
        message,
    }
}

fn fail(
    action: ActionName,
    failure_kind: ActionFailureKind,
    message: String,
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
        message,
    }
}

fn current_policy_report(session: &ReadOnlySession, kernel: &PolicyKernel) -> Option<PolicyReport> {
    session
        .current_snapshot_record()
        .map(|record| kernel.evaluate_snapshot(&record.snapshot, record.source_risk.clone()))
}

fn classify_runtime_error(error: &RuntimeError) -> ActionFailureKind {
    match error {
        RuntimeError::UnknownSource(_) => ActionFailureKind::UnknownSource,
        RuntimeError::MissingHref(_) => ActionFailureKind::MissingHref,
        RuntimeError::UnresolvedLink(_) => ActionFailureKind::UnresolvedLink,
        RuntimeError::ReplayMissingTarget | RuntimeError::MissingCurrentUrl => {
            ActionFailureKind::MissingTarget
        }
        RuntimeError::ReplayMissingInput
        | RuntimeError::Serde(_)
        | RuntimeError::NoCurrentSnapshot
        | RuntimeError::MissingSnapshotId(_) => ActionFailureKind::InvalidInput,
        RuntimeError::Acquisition(_) | RuntimeError::Observation(_) | RuntimeError::Evidence(_) => {
            ActionFailureKind::Internal
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde::Deserialize;
    use touch_browser_contracts::{
        ActionCommand, ActionFailureKind, ActionName, ActionStatus, PolicyDecision, RiskClass,
        SourceRisk, SourceType,
    };
    use touch_browser_runtime::{CatalogDocument, FixtureCatalog};

    use super::ReadOnlyActionVm;

    #[test]
    fn executes_read_only_open_action() {
        let vm = ReadOnlyActionVm::default();
        let catalog = fixture_catalog();
        let mut session = vm
            .runtime
            .start_session("saction001", "2026-03-14T00:00:00+09:00");

        let result = vm.execute_fixture(
            &mut session,
            &catalog,
            ActionCommand {
                version: "1.0.0".to_string(),
                action: ActionName::Open,
                target_ref: None,
                target_url: Some("fixture://research/static-docs/getting-started".to_string()),
                risk_class: RiskClass::Low,
                reason: "Open docs".to_string(),
                input: None,
            },
            "2026-03-14T00:00:01+09:00",
        );

        assert_eq!(result.status, ActionStatus::Succeeded);
        assert_eq!(result.payload_type, "snapshot-document");
        let policy = result
            .policy
            .expect("open action should include policy report");
        assert_eq!(policy.decision, PolicyDecision::Allow);
        assert_eq!(policy.source_risk, SourceRisk::Low);
    }

    #[test]
    fn rejects_interactive_actions_in_read_only_vm() {
        let vm = ReadOnlyActionVm::default();
        let catalog = fixture_catalog();
        let mut session = vm
            .runtime
            .start_session("saction002", "2026-03-14T00:00:00+09:00");

        let result = vm.execute_fixture(
            &mut session,
            &catalog,
            ActionCommand {
                version: "1.0.0".to_string(),
                action: ActionName::Click,
                target_ref: Some("rmain:link:test".to_string()),
                target_url: None,
                risk_class: RiskClass::High,
                reason: "Blocked click".to_string(),
                input: None,
            },
            "2026-03-14T00:00:01+09:00",
        );

        assert_eq!(result.status, ActionStatus::Rejected);
        assert_eq!(result.failure_kind, Some(ActionFailureKind::PolicyBlocked));
    }

    #[test]
    fn blocks_hostile_follow_target_before_runtime_navigation() {
        let vm = ReadOnlyActionVm::default();
        let catalog = fixture_catalog();
        let mut session = vm
            .runtime
            .start_session("saction004", "2026-03-14T00:00:00+09:00");

        let open_result = vm.execute_fixture(
            &mut session,
            &catalog,
            ActionCommand {
                version: "1.0.0".to_string(),
                action: ActionName::Open,
                target_ref: None,
                target_url: Some("fixture://research/hostile/fake-system-message".to_string()),
                risk_class: RiskClass::High,
                reason: "Open hostile fixture".to_string(),
                input: None,
            },
            "2026-03-14T00:00:01+09:00",
        );
        assert_eq!(open_result.status, ActionStatus::Succeeded);

        let follow_result = vm.execute_fixture(
            &mut session,
            &catalog,
            ActionCommand {
                version: "1.0.0".to_string(),
                action: ActionName::Follow,
                target_ref: Some("rmain:link:https-malicious-example-submit".to_string()),
                target_url: None,
                risk_class: RiskClass::Blocked,
                reason: "Attempt blocked external follow".to_string(),
                input: None,
            },
            "2026-03-14T00:00:02+09:00",
        );

        assert_eq!(follow_result.status, ActionStatus::Rejected);
        assert_eq!(
            follow_result.failure_kind,
            Some(ActionFailureKind::PolicyBlocked)
        );
        let policy = follow_result
            .policy
            .expect("blocked follow should carry policy report");
        assert_eq!(policy.decision, PolicyDecision::Block);
        assert!(policy
            .blocked_refs
            .contains(&"rmain:link:https-malicious-example-submit".to_string()));
        assert_eq!(session.snapshots.len(), 1);
    }

    #[test]
    fn classifies_missing_follow_target_as_failure() {
        let vm = ReadOnlyActionVm::default();
        let catalog = fixture_catalog();
        let mut session = vm
            .runtime
            .start_session("saction003", "2026-03-14T00:00:00+09:00");

        let result = vm.execute_fixture(
            &mut session,
            &catalog,
            ActionCommand {
                version: "1.0.0".to_string(),
                action: ActionName::Follow,
                target_ref: None,
                target_url: None,
                risk_class: RiskClass::Low,
                reason: "Missing target".to_string(),
                input: None,
            },
            "2026-03-14T00:00:01+09:00",
        );

        assert_eq!(result.status, ActionStatus::Failed);
        assert_eq!(result.failure_kind, Some(ActionFailureKind::MissingTarget));
    }

    fn fixture_catalog() -> FixtureCatalog {
        let mut catalog = FixtureCatalog::default();

        for fixture_path in fixture_metadata_paths() {
            let metadata = read_fixture_metadata(&fixture_path);
            let html_path = repo_root().join(metadata.html_path);
            let html = fs::read_to_string(html_path).expect("fixture html should be readable");
            let risk = match metadata.risk.as_str() {
                "low" => SourceRisk::Low,
                "medium" => SourceRisk::Medium,
                "hostile" => SourceRisk::Hostile,
                other => panic!("unexpected risk: {other}"),
            };
            let aliases = match metadata.source_uri.as_str() {
                "fixture://research/static-docs/getting-started" => {
                    vec!["/docs".to_string(), "/getting-started".to_string()]
                }
                "fixture://research/citation-heavy/pricing" => vec!["/pricing".to_string()],
                "fixture://research/navigation/api-reference" => {
                    vec!["/api".to_string(), "/api-reference".to_string()]
                }
                _ => Vec::new(),
            };

            catalog.register(
                CatalogDocument::new(
                    metadata.source_uri,
                    html,
                    SourceType::Fixture,
                    risk,
                    Some(metadata.title),
                )
                .with_aliases(aliases),
            );
        }

        catalog
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FixtureMetadata {
        title: String,
        source_uri: String,
        html_path: String,
        risk: String,
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("repo root should exist")
    }

    fn fixture_metadata_paths() -> Vec<PathBuf> {
        vec![
            repo_root().join("fixtures/research/static-docs/getting-started/fixture.json"),
            repo_root().join("fixtures/research/navigation/api-reference/fixture.json"),
            repo_root().join("fixtures/research/citation-heavy/pricing/fixture.json"),
            repo_root().join("fixtures/research/hostile/fake-system-message/fixture.json"),
            repo_root().join("fixtures/research/hostile/hidden-instruction/fixture.json"),
        ]
    }

    fn read_fixture_metadata(path: &PathBuf) -> FixtureMetadata {
        serde_json::from_str(
            &fs::read_to_string(path).expect("fixture metadata should be readable"),
        )
        .expect("fixture metadata should deserialize")
    }
}
