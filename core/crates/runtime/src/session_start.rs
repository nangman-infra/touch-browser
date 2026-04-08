use touch_browser_contracts::{
    PolicyProfile, ReplayTranscript, SessionMode, SessionState, SessionStatus, CONTRACT_VERSION,
};

use crate::{ReadOnlyRuntime, ReadOnlySession};

impl ReadOnlyRuntime {
    pub fn start_session(
        &self,
        session_id: impl Into<String>,
        opened_at: impl Into<String>,
    ) -> ReadOnlySession {
        let session_id = session_id.into();
        let opened_at = opened_at.into();

        ReadOnlySession {
            state: SessionState {
                version: CONTRACT_VERSION.to_string(),
                session_id: session_id.clone(),
                mode: SessionMode::ReadOnly,
                status: SessionStatus::Idle,
                policy_profile: PolicyProfile::ResearchReadOnly,
                current_url: None,
                opened_at: opened_at.clone(),
                updated_at: opened_at,
                visited_urls: Vec::new(),
                snapshot_ids: Vec::new(),
                working_set_refs: Vec::new(),
            },
            transcript: ReplayTranscript {
                version: CONTRACT_VERSION.to_string(),
                session_id,
                entries: Vec::new(),
            },
            snapshots: Vec::new(),
            evidence_reports: Vec::new(),
        }
    }
}
