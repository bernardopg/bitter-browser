use rusqlite::{Connection, Result};
use std::path::PathBuf;

pub struct History {
    conn: Connection,
}

impl History {
    pub fn new() -> Result<Self> {
        let mut path = crate::paths::data_dir();
        path.push("history.sqlite");

        let conn = Connection::open(path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY,
                url TEXT NOT NULL UNIQUE,
                title TEXT,
                visit_count INTEGER DEFAULT 1,
                last_visit_time INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS history_fts USING fts5(
                url,
                title,
                content='history',
                content_rowid='id'
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn add_visit(&self, url: &str, title: Option<&str>) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO history (url, title, last_visit_time)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(url) DO UPDATE SET
                title = excluded.title,
                visit_count = visit_count + 1,
                last_visit_time = excluded.last_visit_time",
            rusqlite::params![url, title, now],
        )?;

        Ok(())
    }

    pub fn search(&self, query: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT url, title FROM history_fts
             WHERE history_fts MATCH ?1
             ORDER BY rank LIMIT 10"
        )?;

        let history_iter = stmt.query_map([query], |row| {
            Ok((row.get(0)?, row.get(1).unwrap_or_default()))
        })?;

        let mut results = Vec::new();
        for item in history_iter {
            results.push(item?);
        }

        Ok(results)
    }
}
