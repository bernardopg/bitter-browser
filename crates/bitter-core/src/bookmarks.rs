use rusqlite::{Connection, Result};
use std::path::PathBuf;

pub struct Bookmarks {
    conn: Connection,
}

impl Bookmarks {
    pub fn new() -> Result<Self> {
        let mut path = crate::paths::data_dir();
        path.push("bookmarks.sqlite");

        let conn = Connection::open(path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS bookmarks (
                id INTEGER PRIMARY KEY,
                url TEXT NOT NULL UNIQUE,
                title TEXT,
                added_time INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn add_bookmark(&self, url: &str, title: Option<&str>) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO bookmarks (url, title, added_time)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(url) DO UPDATE SET
                title = excluded.title",
            rusqlite::params![url, title, now],
        )?;

        Ok(())
    }

    pub fn get_all(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT url, title FROM bookmarks ORDER BY added_time DESC"
        )?;

        let bookmarks_iter = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1).unwrap_or_default()))
        })?;

        let mut results = Vec::new();
        for item in bookmarks_iter {
            results.push(item?);
        }

        Ok(results)
    }
}
