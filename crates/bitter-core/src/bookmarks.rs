use rusqlite::{Connection, Result};

pub struct Bookmarks {
    conn: Connection,
}

const MIGRATION_1: &str = "
    CREATE TABLE IF NOT EXISTS bookmarks (
        id INTEGER PRIMARY KEY,
        url TEXT NOT NULL UNIQUE,
        title TEXT,
        added_time INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS bookmarks_recent ON bookmarks(added_time DESC, id DESC);
";

impl Bookmarks {
    pub fn open(data_dir: &std::path::Path) -> Result<Self, crate::DatabaseError> {
        let conn = crate::storage::open(
            &data_dir.join("bookmarks.sqlite"),
            MIGRATION_1,
            &["SELECT id, url, title, added_time FROM bookmarks LIMIT 0"],
        )?;
        Ok(Self { conn })
    }

    pub fn open_for_search(data_dir: &std::path::Path) -> Result<Self> {
        let conn = crate::storage::open_read_only(&data_dir.join("bookmarks.sqlite"))?;
        Ok(Self { conn })
    }

    pub fn add_bookmark(&self, url: &str, title: Option<&str>) -> Result<()> {
        let now = crate::storage::unix_seconds();

        self.conn.execute(
            "INSERT INTO bookmarks (url, title, added_time)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(url) DO UPDATE SET
                title = excluded.title",
            rusqlite::params![url, title, now],
        )?;

        Ok(())
    }

    pub fn remove_bookmark(&self, url: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM bookmarks WHERE url = ?1", [url])?;

        Ok(())
    }

    pub fn is_bookmarked(&self, url: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM bookmarks WHERE url = ?1",
            [url],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }

    pub fn get_all(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT url, title FROM bookmarks ORDER BY added_time DESC, id DESC")?;

        let bookmarks_iter = stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            ))
        })?;

        let mut results = Vec::new();
        for item in bookmarks_iter {
            results.push(item?);
        }

        Ok(results)
    }

    pub fn search(&self, query: &str) -> Result<Vec<(String, String)>> {
        let query = query
            .trim()
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_");
        let pattern = format!("%{query}%");
        let mut stmt = self.conn.prepare_cached(
            "SELECT url, title
             FROM bookmarks
             WHERE url LIKE ?1 ESCAPE '!' OR title LIKE ?1 ESCAPE '!'
             ORDER BY added_time DESC, id DESC
             LIMIT 20",
        )?;

        let bookmarks_iter = stmt.query_map([pattern], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            ))
        })?;

        let mut results = Vec::new();
        for item in bookmarks_iter {
            results.push(item?);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_existing_bookmarks_and_retains_original_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bookmarks.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE bookmarks (id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE, title TEXT,
                                     added_time INTEGER NOT NULL);
             INSERT INTO bookmarks VALUES (9, 'https://bookmarked.test', NULL, 100);",
        )
        .unwrap();
        drop(conn);

        let bookmarks = Bookmarks::open(dir.path()).unwrap();
        assert_eq!(
            bookmarks.get_all().unwrap(),
            [("https://bookmarked.test".into(), String::new())]
        );
        bookmarks
            .add_bookmark("https://bookmarked.test", Some("Saved page"))
            .unwrap();
        drop(bookmarks);

        let bookmarks = Bookmarks::open(dir.path()).unwrap();
        assert_eq!(bookmarks.search("Saved").unwrap()[0].1, "Saved page");
        assert_eq!(
            bookmarks
                .conn
                .query_row("SELECT id, added_time FROM bookmarks", [], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })
                .unwrap(),
            (9, 100)
        );
        bookmarks
            .remove_bookmark("https://bookmarked.test")
            .unwrap();
        assert!(bookmarks.get_all().unwrap().is_empty());
    }
}
