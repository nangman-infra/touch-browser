use serde_json::{to_value, Value};
use touch_browser_acquisition::AcquisitionEngine;
use touch_browser_contracts::{
    ActionCommand, ActionName, RiskClass, SnapshotDocument, SourceRisk, TranscriptPayloadType,
    CONTRACT_VERSION,
};
use touch_browser_observation::{recommend_requested_tokens, ObservationInput};

use crate::{
    session_journal::{append_snapshot, record_command, record_observation},
    FixtureCatalog, ReadOnlyRuntime, ReadOnlySession, RuntimeError,
};

impl ReadOnlyRuntime {
    pub fn open(
        &self,
        session: &mut ReadOnlySession,
        catalog: &FixtureCatalog,
        target_url: &str,
        timestamp: &str,
    ) -> Result<SnapshotDocument, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Open,
                target_ref: None,
                target_url: Some(target_url.to_string()),
                risk_class: RiskClass::Low,
                reason: "Open a read-only research document.".to_string(),
                input: None,
            },
        )?;

        let (snapshot, source_risk, source_label) = self.load_snapshot(catalog, target_url)?;
        append_snapshot(
            session,
            timestamp,
            snapshot.clone(),
            source_risk,
            source_label,
        )?;
        Ok(snapshot)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn open_live(
        &self,
        session: &mut ReadOnlySession,
        acquisition: &mut AcquisitionEngine,
        target_url: &str,
        requested_tokens: usize,
        source_risk: SourceRisk,
        source_label: Option<String>,
        timestamp: &str,
    ) -> Result<SnapshotDocument, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Open,
                target_ref: None,
                target_url: Some(target_url.to_string()),
                risk_class: RiskClass::Low,
                reason: "Open a live read-only research document.".to_string(),
                input: None,
            },
        )?;

        let acquired = acquisition
            .fetch(target_url)
            .map_err(RuntimeError::Acquisition)?;
        record_observation(
            session,
            timestamp,
            TranscriptPayloadType::AcquisitionRecord,
            to_value(&acquired.record)?,
        )?;

        let effective_budget = recommend_requested_tokens(&acquired.body, requested_tokens);
        let snapshot = self
            .observation
            .compile(&ObservationInput::new(
                acquired.record.final_url.clone(),
                acquired.record.source_type.clone(),
                acquired.body,
                effective_budget,
            ))
            .map_err(RuntimeError::Observation)?;

        append_snapshot(
            session,
            timestamp,
            snapshot.clone(),
            source_risk,
            source_label,
        )?;
        Ok(snapshot)
    }

    pub fn open_snapshot(
        &self,
        session: &mut ReadOnlySession,
        target_url: &str,
        snapshot: SnapshotDocument,
        source_risk: SourceRisk,
        source_label: Option<String>,
        timestamp: &str,
    ) -> Result<SnapshotDocument, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Open,
                target_ref: None,
                target_url: Some(target_url.to_string()),
                risk_class: RiskClass::Low,
                reason: "Open a browser-backed read-only research document.".to_string(),
                input: None,
            },
        )?;

        append_snapshot(
            session,
            timestamp,
            snapshot.clone(),
            source_risk,
            source_label,
        )?;
        Ok(snapshot)
    }

    pub fn read(
        &self,
        session: &mut ReadOnlySession,
        timestamp: &str,
    ) -> Result<SnapshotDocument, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Read,
                target_ref: None,
                target_url: session.state.current_url.clone(),
                risk_class: RiskClass::Low,
                reason: "Read the current semantic snapshot.".to_string(),
                input: None,
            },
        )?;

        let snapshot = session
            .current_snapshot()
            .ok_or(RuntimeError::NoCurrentSnapshot)?
            .snapshot
            .clone();
        record_observation(
            session,
            timestamp,
            TranscriptPayloadType::SnapshotDocument,
            to_value(&snapshot)?,
        )?;
        record_observation(
            session,
            timestamp,
            TranscriptPayloadType::SessionState,
            to_value(&session.state)?,
        )?;
        Ok(snapshot)
    }

    pub fn follow(
        &self,
        session: &mut ReadOnlySession,
        catalog: &FixtureCatalog,
        target_ref: &str,
        timestamp: &str,
    ) -> Result<SnapshotDocument, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Follow,
                target_ref: Some(target_ref.to_string()),
                target_url: None,
                risk_class: RiskClass::Low,
                reason: "Follow a link from the current snapshot.".to_string(),
                input: None,
            },
        )?;

        let current_snapshot = session
            .current_snapshot()
            .ok_or(RuntimeError::NoCurrentSnapshot)?;
        let href = current_snapshot
            .snapshot
            .blocks
            .iter()
            .find(|block| block.stable_ref == target_ref)
            .and_then(|block| block.attributes.get("href"))
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::MissingHref(target_ref.to_string()))?;
        let current_url = session
            .state
            .current_url
            .clone()
            .ok_or(RuntimeError::MissingCurrentUrl)?;
        let resolved = catalog
            .resolve_link(&current_url, href)
            .ok_or_else(|| RuntimeError::UnresolvedLink(href.to_string()))?;
        let (snapshot, source_risk, source_label) =
            self.load_snapshot(catalog, &resolved.source_url)?;
        append_snapshot(
            session,
            timestamp,
            snapshot.clone(),
            source_risk,
            source_label,
        )?;
        Ok(snapshot)
    }
}
