use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PilotTelemetryEvent {
    pub recorded_at_ms: i64,
    pub surface: String,
    pub operation: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_decision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_class: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub provider_hints: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub approved_risks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

impl PilotTelemetryEvent {
    pub fn now(
        surface: impl Into<String>,
        operation: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            recorded_at_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("current time should be after unix epoch")
                .as_millis() as i64,
            surface: surface.into(),
            operation: operation.into(),
            status: status.into(),
            session_id: None,
            tab_id: None,
            current_url: None,
            policy_profile: None,
            policy_decision: None,
            risk_class: None,
            provider_hints: Vec::new(),
            approved_risks: Vec::new(),
            note: None,
            payload: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PilotTelemetrySummary {
    pub db_path: String,
    pub total_events: usize,
    pub distinct_session_count: usize,
    pub latest_recorded_at_ms: Option<i64>,
    pub status_counts: BTreeMap<String, usize>,
    pub surface_counts: BTreeMap<String, usize>,
    pub operation_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct PilotTelemetryStore {
    path: PathBuf,
}

impl PilotTelemetryStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, TelemetryError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(&path)?;
        initialize_schema(&connection)?;
        drop(connection);
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, event: &PilotTelemetryEvent) -> Result<(), TelemetryError> {
        let connection = Connection::open(&self.path)?;
        initialize_schema(&connection)?;
        connection.execute(
            "INSERT INTO pilot_telemetry_events (
                recorded_at_ms,
                surface,
                operation,
                status,
                session_id,
                tab_id,
                current_url,
                policy_profile,
                policy_decision,
                risk_class,
                provider_hints_json,
                approved_risks_json,
                note,
                payload_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                event.recorded_at_ms,
                event.surface,
                event.operation,
                event.status,
                event.session_id,
                event.tab_id,
                event.current_url,
                event.policy_profile,
                event.policy_decision,
                event.risk_class,
                serde_json::to_string(&event.provider_hints)?,
                serde_json::to_string(&event.approved_risks)?,
                event.note,
                event
                    .payload
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
            ],
        )?;
        Ok(())
    }

    pub fn summary(&self) -> Result<PilotTelemetrySummary, TelemetryError> {
        let connection = Connection::open(&self.path)?;
        initialize_schema(&connection)?;
        let total_events = query_count(
            &connection,
            "SELECT COUNT(*) FROM pilot_telemetry_events",
            [],
        )?;
        let distinct_session_count = query_count(
            &connection,
            "SELECT COUNT(DISTINCT session_id) FROM pilot_telemetry_events WHERE session_id IS NOT NULL",
            [],
        )?;
        let latest_recorded_at_ms = connection
            .query_row(
                "SELECT MAX(recorded_at_ms) FROM pilot_telemetry_events",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();

        Ok(PilotTelemetrySummary {
            db_path: self.path.display().to_string(),
            total_events,
            distinct_session_count,
            latest_recorded_at_ms,
            status_counts: query_group_counts(
                &connection,
                "SELECT status, COUNT(*) FROM pilot_telemetry_events GROUP BY status",
            )?,
            surface_counts: query_group_counts(
                &connection,
                "SELECT surface, COUNT(*) FROM pilot_telemetry_events GROUP BY surface",
            )?,
            operation_counts: query_group_counts(
                &connection,
                "SELECT operation, COUNT(*) FROM pilot_telemetry_events GROUP BY operation",
            )?,
        })
    }

    pub fn recent_events(&self, limit: usize) -> Result<Vec<PilotTelemetryEvent>, TelemetryError> {
        let connection = Connection::open(&self.path)?;
        initialize_schema(&connection)?;
        let mut statement = connection.prepare(
            "SELECT
                recorded_at_ms,
                surface,
                operation,
                status,
                session_id,
                tab_id,
                current_url,
                policy_profile,
                policy_decision,
                risk_class,
                provider_hints_json,
                approved_risks_json,
                note,
                payload_json
            FROM pilot_telemetry_events
            ORDER BY id DESC
            LIMIT ?1",
        )?;
        let rows = statement.query_map([limit as i64], |row| {
            let provider_hints_json: String = row.get(10)?;
            let approved_risks_json: String = row.get(11)?;
            let payload_json: Option<String> = row.get(13)?;
            Ok(PilotTelemetryEvent {
                recorded_at_ms: row.get(0)?,
                surface: row.get(1)?,
                operation: row.get(2)?,
                status: row.get(3)?,
                session_id: row.get(4)?,
                tab_id: row.get(5)?,
                current_url: row.get(6)?,
                policy_profile: row.get(7)?,
                policy_decision: row.get(8)?,
                risk_class: row.get(9)?,
                provider_hints: serde_json::from_str(&provider_hints_json)
                    .map_err(to_sqlite_error)?,
                approved_risks: serde_json::from_str(&approved_risks_json)
                    .map_err(to_sqlite_error)?,
                note: row.get(12)?,
                payload: payload_json
                    .map(|value| serde_json::from_str(&value).map_err(to_sqlite_error))
                    .transpose()?,
            })
        })?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }
}

fn initialize_schema(connection: &Connection) -> Result<(), TelemetryError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS pilot_telemetry_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recorded_at_ms INTEGER NOT NULL,
            surface TEXT NOT NULL,
            operation TEXT NOT NULL,
            status TEXT NOT NULL,
            session_id TEXT,
            tab_id TEXT,
            current_url TEXT,
            policy_profile TEXT,
            policy_decision TEXT,
            risk_class TEXT,
            provider_hints_json TEXT NOT NULL,
            approved_risks_json TEXT NOT NULL,
            note TEXT,
            payload_json TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_pilot_telemetry_recorded_at
        ON pilot_telemetry_events (recorded_at_ms);
        CREATE INDEX IF NOT EXISTS idx_pilot_telemetry_session
        ON pilot_telemetry_events (session_id);",
    )?;
    Ok(())
}

fn query_count<P>(connection: &Connection, sql: &str, params: P) -> Result<usize, TelemetryError>
where
    P: rusqlite::Params,
{
    connection
        .query_row(sql, params, |row| row.get::<_, i64>(0))
        .map(|value| value as usize)
        .map_err(TelemetryError::Sqlite)
}

fn query_group_counts(
    connection: &Connection,
    sql: &str,
) -> Result<BTreeMap<String, usize>, TelemetryError> {
    let mut statement = connection.prepare(sql)?;
    let rows = statement.query_map([], |row| {
        let key: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((key, count as usize))
    })?;
    let mut counts = BTreeMap::new();
    for row in rows {
        let (key, count) = row?;
        counts.insert(key, count);
    }
    Ok(counts)
}

fn to_sqlite_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::{PilotTelemetryEvent, PilotTelemetryStore};

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();
        env::temp_dir().join(format!("touch-browser-telemetry-{name}-{nonce}.sqlite"))
    }

    #[test]
    fn appends_and_summarizes_pilot_events() {
        let path = temp_db_path("summary");
        let store = PilotTelemetryStore::open(&path).expect("telemetry store should open");
        let mut event = PilotTelemetryEvent::now("cli", "open", "succeeded");
        event.session_id = Some("scliopen001".to_string());
        event.policy_profile = Some("research-read-only".to_string());
        store.append(&event).expect("first event should append");

        let mut second = PilotTelemetryEvent::now("serve", "runtime.session.submit", "review");
        second.session_id = Some("srvsess-0001".to_string());
        second.provider_hints = vec!["github-auth".to_string()];
        second.approved_risks = vec!["auth".to_string()];
        store.append(&second).expect("second event should append");

        let summary = store.summary().expect("summary should succeed");
        assert_eq!(summary.total_events, 2);
        assert_eq!(summary.distinct_session_count, 2);
        assert_eq!(summary.surface_counts.get("cli"), Some(&1));
        assert_eq!(summary.surface_counts.get("serve"), Some(&1));

        let events = store.recent_events(4).expect("recent events should load");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].provider_hints, vec!["github-auth".to_string()]);
    }
}
