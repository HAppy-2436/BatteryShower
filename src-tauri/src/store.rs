//! SQLite-backed session/sample storage.
//!
//! Schema: each `session` represents one continuous charge (or discharge) run.
//! A new session of the same state replaces the old one (FK CASCADE drops its
//! samples). This is the "always keep only the most recent of each state"
//! requirement (rule #7).

use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;

use crate::sensors::State;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    pub timestamp: i64,
    /// 0 = charging, 1 = discharging, 2 = full
    pub state: i32,
    pub power_watts: f64,
    pub percentage: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Session {
    pub state: String,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub samples: Vec<Sample>,
}

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS sessions (
                 id         INTEGER PRIMARY KEY AUTOINCREMENT,
                 state      TEXT    NOT NULL,
                 start_time INTEGER NOT NULL,
                 end_time   INTEGER
             );
             CREATE TABLE IF NOT EXISTS samples (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 session_id  INTEGER NOT NULL,
                 timestamp   INTEGER NOT NULL,
                 state       INTEGER NOT NULL,
                 power_watts REAL    NOT NULL,
                 percentage  INTEGER NOT NULL,
                 FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS idx_samples_session_ts
                 ON samples(session_id, timestamp);",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Delete the most recent session of the given state and return its id
    /// (if any). Used to enforce "always keep only the most recent of each state".
    pub fn drop_latest(&self, state_str: &str) -> rusqlite::Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        let id: Option<i64> = conn
            .query_row(
                "SELECT id FROM sessions WHERE state = ?1 ORDER BY start_time DESC LIMIT 1",
                params![state_str],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = id {
            conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        }
        Ok(id)
    }

    /// Create a new session of the given state, deleting any previous one first.
    pub fn start_new_session(&self, state_str: &str) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().unwrap();
        // Delete the previous session of the same state (CASCADE removes samples).
        conn.execute("DELETE FROM sessions WHERE state = ?1", params![state_str])?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO sessions (state, start_time) VALUES (?1, ?2)",
            params![state_str, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn end_session(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE sessions SET end_time = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn insert_sample(&self, session_id: i64, sample: &Sample) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO samples (session_id, timestamp, state, power_watts, percentage)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                sample.timestamp,
                sample.state,
                sample.power_watts,
                sample.percentage
            ],
        )?;
        Ok(())
    }

    pub fn get_latest_session(&self, state_str: &str) -> rusqlite::Result<Option<Session>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, start_time, end_time FROM sessions WHERE state = ?1
             ORDER BY start_time DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![state_str])?;
        let row = match rows.next()? {
            Some(r) => r,
            None => return Ok(None),
        };
        let id: i64 = row.get(0)?;
        let start_time: i64 = row.get(1)?;
        let end_time: Option<i64> = row.get(2)?;

        let mut stmt2 = conn.prepare(
            "SELECT timestamp, state, power_watts, percentage
             FROM samples WHERE session_id = ?1 ORDER BY timestamp",
        )?;
        let samples: Vec<Sample> = stmt2
            .query_map(params![id], |r| {
                Ok(Sample {
                    timestamp: r.get(0)?,
                    state: r.get(1)?,
                    power_watts: r.get(2)?,
                    percentage: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(Some(Session {
            state: state_str.to_string(),
            start_time,
            end_time,
            samples,
        }))
    }

    /// Convenience: turn the sensor state into the persisted string key.
    pub fn state_key(state: State) -> &'static str {
        match state {
            State::Charging => "charging",
            State::Discharging => "discharging",
            State::Full => "full",
        }
    }
}
