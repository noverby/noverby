//! Pipeline event log queries.
//!
//! Manages the `events` table, which stores pipeline status events as JSON
//! blobs for WebSocket `/events` backfill. Clients connect with a cursor
//! (event ID) and receive all events after that cursor.
//!
//! Also manages the `last_time_us` singleton table for Jetstream cursor
//! persistence, and the `knots` table for knot cursor tracking.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// A stored pipeline event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Auto-incrementing event ID (used as the cursor).
    pub id: i64,
    /// Event kind (e.g. `"pipeline_status"`).
    pub kind: String,
    /// JSON-encoded event payload.
    pub payload: String,
    /// When the event was created (ISO 8601).
    pub created_at: String,
}

/// A knot cursor record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnotCursor {
    /// Knot server hostname.
    pub knot: String,
    /// Last processed event cursor for this knot (may be `None`).
    pub cursor: Option<String>,
    /// When the knot was added.
    pub added_at: String,
}

// ---------------------------------------------------------------------------
// events table
// ---------------------------------------------------------------------------

/// Insert a new pipeline event.
///
/// Returns the auto-generated event ID, which serves as the cursor for
/// WebSocket clients.
pub fn insert_event(conn: &Connection, kind: &str, payload: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO events (kind, payload) VALUES (?1, ?2)",
        params![kind, payload],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Get all events with an ID greater than the given cursor.
///
/// This is the primary query for WebSocket `/events` backfill: a client
/// provides its last-seen event ID and receives all newer events.
///
/// Results are ordered by ID ascending (oldest first).
pub fn get_events_after(conn: &Connection, cursor: i64) -> rusqlite::Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, payload, created_at FROM events WHERE id > ?1 ORDER BY id ASC",
    )?;

    let events = stmt
        .query_map(params![cursor], |row| {
            Ok(Event {
                id: row.get(0)?,
                kind: row.get(1)?,
                payload: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(events)
}

/// Get all events (no cursor filter).
///
/// Results are ordered by ID ascending.
pub fn get_all_events(conn: &Connection) -> rusqlite::Result<Vec<Event>> {
    get_events_after(conn, 0)
}

/// Get the latest event ID (the current cursor head).
///
/// Returns `None` if no events exist.
pub fn get_latest_event_id(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    conn.query_row("SELECT MAX(id) FROM events", [], |row| row.get(0))
}

/// Get a single event by ID.
pub fn get_event(conn: &Connection, id: i64) -> rusqlite::Result<Option<Event>> {
    conn.query_row(
        "SELECT id, kind, payload, created_at FROM events WHERE id = ?1",
        params![id],
        |row| {
            Ok(Event {
                id: row.get(0)?,
                kind: row.get(1)?,
                payload: row.get(2)?,
                created_at: row.get(3)?,
            })
        },
    )
    .optional()
}

/// Get the total number of stored events.
pub fn event_count(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
}

// ---------------------------------------------------------------------------
// last_time_us table (Jetstream cursor)
// ---------------------------------------------------------------------------

/// Save the Jetstream cursor (last processed event timestamp in microseconds).
///
/// The `last_time_us` table is a singleton — there's always exactly one row
/// with `id = 1`, initialized to `0` by the migration.
pub fn save_last_time_us(conn: &Connection, time_us: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE last_time_us SET time_us = ?1 WHERE id = 1",
        params![time_us],
    )?;
    Ok(())
}

/// Get the Jetstream cursor (last processed event timestamp in microseconds).
///
/// Returns `0` if no cursor has been saved yet (the default from the migration).
pub fn get_last_time_us(conn: &Connection) -> rusqlite::Result<i64> {
    let time_us: i64 =
        conn.query_row("SELECT time_us FROM last_time_us WHERE id = 1", [], |row| {
            row.get(0)
        })?;
    Ok(time_us)
}

// ---------------------------------------------------------------------------
// knots table
// ---------------------------------------------------------------------------

/// Add a knot to the tracking table.
///
/// If the knot already exists, this is a no-op (`INSERT OR IGNORE`).
pub fn add_knot(conn: &Connection, knot: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO knots (knot) VALUES (?1)",
        params![knot],
    )?;
    Ok(())
}

/// Remove a knot from the tracking table.
///
/// Returns `true` if a row was deleted, `false` if the knot wasn't found.
pub fn remove_knot(conn: &Connection, knot: &str) -> rusqlite::Result<bool> {
    let deleted = conn.execute("DELETE FROM knots WHERE knot = ?1", params![knot])?;
    Ok(deleted > 0)
}

/// Update the cursor for a knot.
pub fn update_knot_cursor(conn: &Connection, knot: &str, cursor: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE knots SET cursor = ?1 WHERE knot = ?2",
        params![cursor, knot],
    )?;
    Ok(())
}

/// Get the cursor for a knot.
///
/// Returns `None` if the knot doesn't exist or has no cursor set.
pub fn get_knot_cursor(conn: &Connection, knot: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT cursor FROM knots WHERE knot = ?1",
        params![knot],
        |row| row.get(0),
    )
    .optional()
    // Flatten: if the row exists but cursor is NULL, we still want None
    .map(|opt| opt.flatten())
}

/// Get all tracked knots with their cursors.
pub fn get_all_knots(conn: &Connection) -> rusqlite::Result<Vec<KnotCursor>> {
    let mut stmt = conn.prepare("SELECT knot, cursor, added_at FROM knots ORDER BY knot")?;

    let knots = stmt
        .query_map([], |row| {
            Ok(KnotCursor {
                knot: row.get(0)?,
                cursor: row.get(1)?,
                added_at: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(knots)
}

/// Get all tracked knot hostnames (without cursor data).
pub fn get_knot_names(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT knot FROM knots ORDER BY knot")?;

    let names = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;

    fn setup_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        migrations::migrate(&mut conn).unwrap();
        conn
    }

    // -----------------------------------------------------------------------
    // events table tests
    // -----------------------------------------------------------------------

    #[test]
    fn insert_and_get_event() {
        let conn = setup_db();

        let id = insert_event(&conn, "pipeline_status", r#"{"status":"running"}"#).unwrap();
        assert!(id > 0);

        let event = get_event(&conn, id).unwrap().expect("event should exist");
        assert_eq!(event.id, id);
        assert_eq!(event.kind, "pipeline_status");
        assert_eq!(event.payload, r#"{"status":"running"}"#);
    }

    #[test]
    fn get_event_not_found() {
        let conn = setup_db();
        let event = get_event(&conn, 9999).unwrap();
        assert!(event.is_none());
    }

    #[test]
    fn get_events_after_cursor() {
        let conn = setup_db();

        let id1 = insert_event(&conn, "a", "payload1").unwrap();
        let id2 = insert_event(&conn, "b", "payload2").unwrap();
        let _id3 = insert_event(&conn, "c", "payload3").unwrap();

        // Get events after the first one
        let events = get_events_after(&conn, id1).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, id2);
        assert_eq!(events[0].kind, "b");
        assert_eq!(events[1].kind, "c");
    }

    #[test]
    fn get_events_after_cursor_zero_returns_all() {
        let conn = setup_db();

        insert_event(&conn, "a", "p1").unwrap();
        insert_event(&conn, "b", "p2").unwrap();

        let events = get_events_after(&conn, 0).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn get_events_after_cursor_beyond_latest() {
        let conn = setup_db();

        insert_event(&conn, "a", "p1").unwrap();

        let events = get_events_after(&conn, 9999).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn get_all_events_empty() {
        let conn = setup_db();
        let events = get_all_events(&conn).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn get_latest_event_id_empty() {
        let conn = setup_db();
        let latest = get_latest_event_id(&conn).unwrap();
        assert!(latest.is_none());
    }

    #[test]
    fn get_latest_event_id_returns_max() {
        let conn = setup_db();

        insert_event(&conn, "a", "p1").unwrap();
        let id2 = insert_event(&conn, "b", "p2").unwrap();

        let latest = get_latest_event_id(&conn).unwrap();
        assert_eq!(latest, Some(id2));
    }

    #[test]
    fn event_count_empty() {
        let conn = setup_db();
        assert_eq!(event_count(&conn).unwrap(), 0);
    }

    #[test]
    fn event_count_after_inserts() {
        let conn = setup_db();

        insert_event(&conn, "a", "p1").unwrap();
        insert_event(&conn, "b", "p2").unwrap();
        insert_event(&conn, "c", "p3").unwrap();

        assert_eq!(event_count(&conn).unwrap(), 3);
    }

    #[test]
    fn events_ordered_by_id_ascending() {
        let conn = setup_db();

        let id1 = insert_event(&conn, "first", "p1").unwrap();
        let id2 = insert_event(&conn, "second", "p2").unwrap();
        let id3 = insert_event(&conn, "third", "p3").unwrap();

        let events = get_all_events(&conn).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].id, id1);
        assert_eq!(events[1].id, id2);
        assert_eq!(events[2].id, id3);
    }

    // -----------------------------------------------------------------------
    // last_time_us tests
    // -----------------------------------------------------------------------

    #[test]
    fn last_time_us_default_is_zero() {
        let conn = setup_db();
        assert_eq!(get_last_time_us(&conn).unwrap(), 0);
    }

    #[test]
    fn save_and_get_last_time_us() {
        let conn = setup_db();

        save_last_time_us(&conn, 1_700_000_000_000_000).unwrap();
        assert_eq!(get_last_time_us(&conn).unwrap(), 1_700_000_000_000_000);

        save_last_time_us(&conn, 1_800_000_000_000_000).unwrap();
        assert_eq!(get_last_time_us(&conn).unwrap(), 1_800_000_000_000_000);
    }

    // -----------------------------------------------------------------------
    // knots table tests
    // -----------------------------------------------------------------------

    #[test]
    fn add_and_get_knots() {
        let conn = setup_db();

        add_knot(&conn, "knot-a.example.com").unwrap();
        add_knot(&conn, "knot-b.example.com").unwrap();

        let names = get_knot_names(&conn).unwrap();
        assert_eq!(names, vec!["knot-a.example.com", "knot-b.example.com"]);
    }

    #[test]
    fn add_knot_is_idempotent() {
        let conn = setup_db();

        add_knot(&conn, "knot.example.com").unwrap();
        add_knot(&conn, "knot.example.com").unwrap();

        let names = get_knot_names(&conn).unwrap();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn remove_knot_existing() {
        let conn = setup_db();

        add_knot(&conn, "knot.example.com").unwrap();
        assert!(remove_knot(&conn, "knot.example.com").unwrap());

        let names = get_knot_names(&conn).unwrap();
        assert!(names.is_empty());
    }

    #[test]
    fn remove_knot_not_found() {
        let conn = setup_db();
        assert!(!remove_knot(&conn, "nonexistent.example.com").unwrap());
    }

    #[test]
    fn knot_cursor_initially_none() {
        let conn = setup_db();

        add_knot(&conn, "knot.example.com").unwrap();
        let cursor = get_knot_cursor(&conn, "knot.example.com").unwrap();
        assert!(cursor.is_none());
    }

    #[test]
    fn update_and_get_knot_cursor() {
        let conn = setup_db();

        add_knot(&conn, "knot.example.com").unwrap();
        update_knot_cursor(&conn, "knot.example.com", "cursor-abc-123").unwrap();

        let cursor = get_knot_cursor(&conn, "knot.example.com").unwrap();
        assert_eq!(cursor.as_deref(), Some("cursor-abc-123"));
    }

    #[test]
    fn update_knot_cursor_overwrites() {
        let conn = setup_db();

        add_knot(&conn, "knot.example.com").unwrap();
        update_knot_cursor(&conn, "knot.example.com", "cursor-1").unwrap();
        update_knot_cursor(&conn, "knot.example.com", "cursor-2").unwrap();

        let cursor = get_knot_cursor(&conn, "knot.example.com").unwrap();
        assert_eq!(cursor.as_deref(), Some("cursor-2"));
    }

    #[test]
    fn get_knot_cursor_nonexistent_knot() {
        let conn = setup_db();
        let cursor = get_knot_cursor(&conn, "nonexistent.example.com").unwrap();
        assert!(cursor.is_none());
    }

    #[test]
    fn get_all_knots_with_cursors() {
        let conn = setup_db();

        add_knot(&conn, "knot-a.example.com").unwrap();
        add_knot(&conn, "knot-b.example.com").unwrap();
        update_knot_cursor(&conn, "knot-a.example.com", "cursor-a").unwrap();

        let knots = get_all_knots(&conn).unwrap();
        assert_eq!(knots.len(), 2);
        assert_eq!(knots[0].knot, "knot-a.example.com");
        assert_eq!(knots[0].cursor.as_deref(), Some("cursor-a"));
        assert_eq!(knots[1].knot, "knot-b.example.com");
        assert!(knots[1].cursor.is_none());
    }

    #[test]
    fn get_all_knots_empty() {
        let conn = setup_db();
        let knots = get_all_knots(&conn).unwrap();
        assert!(knots.is_empty());
    }

    #[test]
    fn get_knot_names_sorted() {
        let conn = setup_db();

        add_knot(&conn, "charlie.example.com").unwrap();
        add_knot(&conn, "alpha.example.com").unwrap();
        add_knot(&conn, "bravo.example.com").unwrap();

        let names = get_knot_names(&conn).unwrap();
        assert_eq!(
            names,
            vec![
                "alpha.example.com",
                "bravo.example.com",
                "charlie.example.com"
            ]
        );
    }
}
