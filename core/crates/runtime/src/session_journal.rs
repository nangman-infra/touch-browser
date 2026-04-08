use serde_json::{to_value, Value};
use touch_browser_contracts::{
    ActionCommand, ReplayTranscriptEntry, SessionStatus, SnapshotDocument, SourceRisk,
    TranscriptKind, TranscriptPayloadType,
};
use touch_browser_memory::compact_working_set;

use crate::{ReadOnlySession, RuntimeError, SessionSnapshotRecord};

pub(crate) fn append_snapshot(
    session: &mut ReadOnlySession,
    timestamp: &str,
    snapshot: SnapshotDocument,
    source_risk: SourceRisk,
    source_label: Option<String>,
) -> Result<(), RuntimeError> {
    let snapshot_id = format!(
        "snap_{}_{}",
        session
            .state
            .session_id
            .strip_prefix('s')
            .unwrap_or(session.state.session_id.as_str()),
        session.snapshots.len() + 1
    );
    session.snapshots.push(SessionSnapshotRecord {
        snapshot_id: snapshot_id.clone(),
        snapshot: snapshot.clone(),
        source_risk,
        source_label,
    });
    session.state.status = SessionStatus::Active;
    session.state.current_url = Some(snapshot.source.source_url.clone());
    session.state.updated_at = timestamp.to_string();
    if !session
        .state
        .visited_urls
        .iter()
        .any(|url| url == &snapshot.source.source_url)
    {
        session
            .state
            .visited_urls
            .push(snapshot.source.source_url.clone());
    }
    session.state.snapshot_ids.push(snapshot_id);
    session.state.working_set_refs =
        compact_working_set(&snapshot, None, &session.state.working_set_refs, 6).kept_refs;
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
    Ok(())
}

pub(crate) fn record_command(
    session: &mut ReadOnlySession,
    timestamp: &str,
    command: ActionCommand,
) -> Result<(), RuntimeError> {
    let payload = to_value(command)?;
    let seq = session.transcript.entries.len() + 1;
    session.transcript.entries.push(ReplayTranscriptEntry {
        seq,
        timestamp: timestamp.to_string(),
        kind: TranscriptKind::Command,
        payload_type: TranscriptPayloadType::ActionCommand,
        payload,
    });
    Ok(())
}

pub(crate) fn record_observation(
    session: &mut ReadOnlySession,
    timestamp: &str,
    payload_type: TranscriptPayloadType,
    payload: Value,
) -> Result<(), RuntimeError> {
    let seq = session.transcript.entries.len() + 1;
    session.transcript.entries.push(ReplayTranscriptEntry {
        seq,
        timestamp: timestamp.to_string(),
        kind: TranscriptKind::Observation,
        payload_type,
        payload,
    });
    Ok(())
}

pub(crate) fn record_system(
    session: &mut ReadOnlySession,
    timestamp: &str,
    payload: Value,
) -> Result<(), RuntimeError> {
    let seq = session.transcript.entries.len() + 1;
    session.transcript.entries.push(ReplayTranscriptEntry {
        seq,
        timestamp: timestamp.to_string(),
        kind: TranscriptKind::System,
        payload_type: TranscriptPayloadType::JsonRpc,
        payload,
    });
    Ok(())
}
