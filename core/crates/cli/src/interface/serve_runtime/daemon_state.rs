use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    browser_context_dir_for_session_file, dispatch, load_browser_cli_session, AckRisk, CliCommand,
    CliError, SessionFileOptions,
};

use super::presenters;

#[derive(Debug)]
pub(crate) struct ServeDaemonState {
    pub(crate) root_dir: PathBuf,
    pub(crate) next_session_seq: usize,
    pub(crate) next_tab_seq: usize,
    pub(crate) sessions: BTreeMap<String, ServeRuntimeSession>,
}

#[derive(Debug)]
pub(crate) struct ServeRuntimeSession {
    pub(crate) headless: bool,
    pub(crate) allowlisted_domains: Vec<String>,
    pub(crate) secret_prefills: BTreeMap<String, String>,
    pub(crate) approved_risks: BTreeSet<AckRisk>,
    pub(crate) tabs: BTreeMap<String, ServeTabRecord>,
    pub(crate) active_tab_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ServeTabRecord {
    pub(crate) session_file: PathBuf,
}

impl ServeDaemonState {
    pub(crate) fn new() -> Result<Self, CliError> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();
        let root_dir = env::temp_dir().join(format!(
            "touch-browser-serve-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root_dir)?;

        Ok(Self {
            root_dir,
            next_session_seq: 0,
            next_tab_seq: 0,
            sessions: BTreeMap::new(),
        })
    }

    pub(crate) fn cleanup(&self) -> Result<(), CliError> {
        if self.root_dir.exists() {
            fs::remove_dir_all(&self.root_dir)?;
        }
        Ok(())
    }

    pub(crate) fn create_session(
        &mut self,
        headless: bool,
        allowlisted_domains: Vec<String>,
    ) -> Result<(String, String), CliError> {
        self.next_session_seq += 1;
        let session_id = format!("srvsess-{:04}", self.next_session_seq);
        self.sessions.insert(
            session_id.clone(),
            ServeRuntimeSession {
                headless,
                allowlisted_domains,
                secret_prefills: BTreeMap::new(),
                approved_risks: BTreeSet::new(),
                tabs: BTreeMap::new(),
                active_tab_id: None,
            },
        );
        let tab_id = self.create_tab_for_session(&session_id)?;
        self.select_tab(&session_id, &tab_id)?;
        Ok((session_id, tab_id))
    }

    pub(crate) fn create_tab_for_session(&mut self, session_id: &str) -> Result<String, CliError> {
        self.session(session_id)?;
        self.next_tab_seq += 1;
        let tab_id = format!("tab-{:04}", self.next_tab_seq);
        let session_dir = self.root_dir.join(session_id);
        fs::create_dir_all(&session_dir)?;
        let session_file = session_dir.join(format!("{tab_id}.json"));
        let session = self.session_mut(session_id)?;
        session
            .tabs
            .insert(tab_id.clone(), ServeTabRecord { session_file });
        if session.active_tab_id.is_none() {
            session.active_tab_id = Some(tab_id.clone());
        }
        Ok(tab_id)
    }

    pub(crate) fn ensure_active_tab(&mut self, session_id: &str) -> Result<String, CliError> {
        match self.session(session_id)?.active_tab_id.clone() {
            Some(tab_id) => Ok(tab_id),
            None => {
                let tab_id = self.create_tab_for_session(session_id)?;
                self.select_tab(session_id, &tab_id)?;
                Ok(tab_id)
            }
        }
    }

    pub(crate) fn session(&self, session_id: &str) -> Result<&ServeRuntimeSession, CliError> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| CliError::Usage(format!("Unknown serve session `{session_id}`.")))
    }

    pub(crate) fn session_mut(
        &mut self,
        session_id: &str,
    ) -> Result<&mut ServeRuntimeSession, CliError> {
        self.sessions
            .get_mut(session_id)
            .ok_or_else(|| CliError::Usage(format!("Unknown serve session `{session_id}`.")))
    }

    pub(crate) fn ensure_tab(&self, session_id: &str, tab_id: &str) -> Result<(), CliError> {
        let session = self.session(session_id)?;
        if session.tabs.contains_key(tab_id) {
            Ok(())
        } else {
            Err(CliError::Usage(format!(
                "Serve session `{session_id}` does not contain tab `{tab_id}`."
            )))
        }
    }

    pub(crate) fn select_tab(&mut self, session_id: &str, tab_id: &str) -> Result<(), CliError> {
        self.ensure_tab(session_id, tab_id)?;
        let session = self.session_mut(session_id)?;
        session.active_tab_id = Some(tab_id.to_string());
        Ok(())
    }

    pub(crate) fn opened_tab_file(
        &self,
        session_id: &str,
        requested_tab_id: Option<&str>,
    ) -> Result<(String, PathBuf), CliError> {
        let session = self.session(session_id)?;
        let tab_id = match requested_tab_id {
            Some(tab_id) => {
                self.ensure_tab(session_id, tab_id)?;
                tab_id.to_string()
            }
            None => session.active_tab_id.clone().ok_or_else(|| {
                CliError::Usage(format!(
                    "Serve session `{session_id}` does not have an active tab."
                ))
            })?,
        };
        let tab = session.tabs.get(&tab_id).ok_or_else(|| {
            CliError::Usage(format!(
                "Serve session `{session_id}` does not contain tab `{tab_id}`."
            ))
        })?;

        if !tab.session_file.is_file() {
            return Err(CliError::Usage(format!(
                "Serve session `{session_id}` tab `{tab_id}` has not been opened yet."
            )));
        }

        Ok((tab_id, tab.session_file.clone()))
    }

    pub(crate) fn extend_session_allowlist(
        &mut self,
        session_id: &str,
        values: &[String],
    ) -> Result<(), CliError> {
        let session = self.session_mut(session_id)?;
        for value in values {
            if !session
                .allowlisted_domains
                .iter()
                .any(|existing| existing == value)
            {
                session.allowlisted_domains.push(value.clone());
            }
        }
        session.allowlisted_domains.sort();
        Ok(())
    }

    pub(crate) fn tab_summary(
        &self,
        session_id: &str,
        tab_id: &str,
    ) -> Result<presenters::TabSummaryResponse, CliError> {
        let session = self.session(session_id)?;
        let tab = session.tabs.get(tab_id).ok_or_else(|| {
            CliError::Usage(format!(
                "Serve session `{session_id}` does not contain tab `{tab_id}`."
            ))
        })?;
        let persisted = if tab.session_file.is_file() {
            Some(load_browser_cli_session(&tab.session_file)?)
        } else {
            None
        };
        let current_url = persisted
            .as_ref()
            .and_then(|persisted| persisted.session.state.current_url.clone());
        let visited_url_count = persisted
            .as_ref()
            .map(|persisted| persisted.session.state.visited_urls.len())
            .unwrap_or(0);
        let snapshot_count = persisted
            .as_ref()
            .map(|persisted| persisted.session.snapshots.len())
            .unwrap_or(0);
        let latest_search_query = persisted
            .as_ref()
            .and_then(|persisted| persisted.latest_search.as_ref())
            .map(|report| report.query.clone());
        let latest_search_result_count = persisted
            .as_ref()
            .and_then(|persisted| persisted.latest_search.as_ref())
            .map(|report| report.result_count)
            .unwrap_or(0);

        Ok(presenters::TabSummaryResponse {
            tab_id: tab_id.to_string(),
            active: session.active_tab_id.as_deref() == Some(tab_id),
            session_file: tab.session_file.display().to_string(),
            has_state: persisted.is_some(),
            current_url,
            visited_url_count,
            snapshot_count,
            latest_search_query,
            latest_search_result_count,
        })
    }

    pub(crate) fn close_tab(
        &mut self,
        session_id: &str,
        tab_id: &str,
    ) -> Result<presenters::TabCloseResponse, CliError> {
        self.ensure_tab(session_id, tab_id)?;

        let (session_file, was_active) = {
            let session = self.session(session_id)?;
            let tab = session.tabs.get(tab_id).expect("tab existence checked");
            (
                tab.session_file.clone(),
                session.active_tab_id.as_deref() == Some(tab_id),
            )
        };

        let mut removed_state = false;
        if session_file.is_file() {
            dispatch(CliCommand::SessionClose(SessionFileOptions {
                session_file: session_file.clone(),
            }))?;
            removed_state = true;
        } else {
            let context_dir = browser_context_dir_for_session_file(&session_file);
            if context_dir.exists() {
                fs::remove_dir_all(context_dir)?;
            }
        }

        let session = self.session_mut(session_id)?;
        session.tabs.remove(tab_id);
        if was_active {
            session.active_tab_id = session.tabs.keys().next().cloned();
        }

        Ok(presenters::TabCloseResponse {
            session_id: session_id.to_string(),
            tab_id: tab_id.to_string(),
            removed: true,
            removed_state,
            active_tab_id: session.active_tab_id.clone(),
            remaining_tab_count: session.tabs.len(),
        })
    }

    pub(crate) fn close_session(
        &mut self,
        session_id: &str,
    ) -> Result<presenters::SessionCloseResponse, CliError> {
        let tab_ids = self
            .session(session_id)?
            .tabs
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        let mut removed_tabs = 0usize;

        for tab_id in tab_ids {
            let _ = self.close_tab(session_id, &tab_id)?;
            removed_tabs += 1;
        }

        self.sessions.remove(session_id);
        let session_dir = self.root_dir.join(session_id);
        if session_dir.exists() {
            fs::remove_dir_all(session_dir)?;
        }

        Ok(presenters::SessionCloseResponse {
            session_id: session_id.to_string(),
            removed: true,
            removed_tabs,
        })
    }
}
