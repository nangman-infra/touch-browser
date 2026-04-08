use serde_json::Value;
use touch_browser_contracts::{
    ActionCommand, ActionName, ReplayTranscript, TranscriptKind, TranscriptPayloadType,
};

use crate::{
    ClaimInput, CompactInput, DiffInput, FixtureCatalog, ReadOnlyRuntime, ReadOnlySession,
    RuntimeError,
};

impl ReadOnlyRuntime {
    pub fn replay(
        &self,
        catalog: &FixtureCatalog,
        transcript: &ReplayTranscript,
        opened_at: &str,
    ) -> Result<ReadOnlySession, RuntimeError> {
        let mut session = self.start_session(transcript.session_id.clone(), opened_at.to_string());

        for entry in &transcript.entries {
            if entry.kind != TranscriptKind::Command
                || entry.payload_type != TranscriptPayloadType::ActionCommand
            {
                continue;
            }

            let command: ActionCommand = serde_json::from_value(entry.payload.clone())?;

            match command.action {
                ActionName::Open => {
                    let target_url = command
                        .target_url
                        .as_deref()
                        .ok_or(RuntimeError::ReplayMissingTarget)?;
                    self.open(&mut session, catalog, target_url, &entry.timestamp)?;
                }
                ActionName::Read => {
                    let _ = self.read(&mut session, &entry.timestamp)?;
                }
                ActionName::Follow => {
                    let target_ref = command
                        .target_ref
                        .as_deref()
                        .ok_or(RuntimeError::ReplayMissingTarget)?;
                    let _ = self.follow(&mut session, catalog, target_ref, &entry.timestamp)?;
                }
                ActionName::Extract => {
                    let claims = parse_claim_inputs(command.input.as_ref())?;
                    let _ = self.extract(&mut session, claims, &entry.timestamp)?;
                }
                ActionName::Diff => {
                    let diff_input: DiffInput = serde_json::from_value(
                        command.input.ok_or(RuntimeError::ReplayMissingInput)?,
                    )?;
                    let _ = self.diff(&mut session, diff_input, &entry.timestamp)?;
                }
                ActionName::Compact => {
                    let compact_input: CompactInput = serde_json::from_value(
                        command.input.ok_or(RuntimeError::ReplayMissingInput)?,
                    )?;
                    let _ = self.compact(&mut session, compact_input, &entry.timestamp)?;
                }
                _ => {}
            }
        }

        Ok(session)
    }
}

fn parse_claim_inputs(input: Option<&Value>) -> Result<Vec<ClaimInput>, RuntimeError> {
    let claims_value = input
        .and_then(|value| value.get("claims"))
        .cloned()
        .ok_or(RuntimeError::ReplayMissingInput)?;
    serde_json::from_value(claims_value).map_err(RuntimeError::Serde)
}
