use rusqlite::{Connection, Result};

pub struct History {
    conn: Connection,
}

const MIGRATION_1: &str = "
    CREATE TABLE IF NOT EXISTS history (
        id INTEGER PRIMARY KEY,
        url TEXT NOT NULL UNIQUE,
        title TEXT,
        visit_count INTEGER DEFAULT 1,
        last_visit_time INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS history_recent ON history(last_visit_time DESC, id DESC);
    CREATE VIRTUAL TABLE IF NOT EXISTS history_fts USING fts5(
        url, title, content='history', content_rowid='id'
    );
    CREATE TRIGGER IF NOT EXISTS history_ai AFTER INSERT ON history BEGIN
        INSERT INTO history_fts(rowid, url, title)
        VALUES (new.id, new.url, new.title);
    END;
    CREATE TRIGGER IF NOT EXISTS history_ad AFTER DELETE ON history BEGIN
        INSERT INTO history_fts(history_fts, rowid, url, title)
        VALUES ('delete', old.id, old.url, old.title);
    END;
    CREATE TRIGGER IF NOT EXISTS history_au AFTER UPDATE ON history BEGIN
        INSERT INTO history_fts(history_fts, rowid, url, title)
        VALUES ('delete', old.id, old.url, old.title);
        INSERT INTO history_fts(rowid, url, title)
        VALUES (new.id, new.url, new.title);
    END;
    INSERT INTO history_fts(history_fts) VALUES ('rebuild');
";

impl History {
    pub fn open(data_dir: &std::path::Path) -> Result<Self, crate::DatabaseError> {
        let conn = crate::storage::open(
            &data_dir.join("history.sqlite"),
            MIGRATION_1,
            &[
                "SELECT id, url, title, visit_count, last_visit_time FROM history LIMIT 0",
                "SELECT url, title FROM history_fts WHERE history_fts MATCH '' LIMIT 0",
            ],
        )?;
        Ok(Self { conn })
    }

    pub fn open_for_search(data_dir: &std::path::Path) -> Result<Self> {
        let conn = crate::storage::open_read_only(&data_dir.join("history.sqlite"))?;
        Ok(Self { conn })
    }

    pub fn add_visit(&self, url: &str, title: Option<&str>) -> Result<()> {
        let now = crate::storage::unix_seconds();

        self.conn
            .prepare_cached(
                "INSERT INTO history (url, title, last_visit_time)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(url) DO UPDATE SET
                title = COALESCE(excluded.title, history.title),
                visit_count = history.visit_count + 1,
                last_visit_time = MAX(history.last_visit_time, excluded.last_visit_time)",
            )?
            .execute(rusqlite::params![url, title, now])?;

        Ok(())
    }

    pub fn update_title(&self, url: &str, title: &str) -> Result<()> {
        self.conn
            .prepare_cached("UPDATE history SET title = ?2 WHERE url = ?1 AND title IS NOT ?2")?
            .execute(rusqlite::params![url, title])?;
        Ok(())
    }

    pub fn search(&self, query: &str) -> Result<Vec<(String, String)>> {
        let Some(match_query) = history_fts_query(query) else {
            return self.recent(10);
        };

        let mut stmt = self.conn.prepare_cached(
            "SELECT url, title FROM history_fts
             WHERE history_fts MATCH ?1
             ORDER BY rank LIMIT 10",
        )?;

        let history_iter = stmt.query_map([match_query], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            ))
        })?;

        let mut results = Vec::new();
        for item in history_iter {
            results.push(item?);
        }

        Ok(results)
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT url, title
             FROM history
             ORDER BY last_visit_time DESC, id DESC
             LIMIT ?1",
        )?;

        let history_iter = stmt.query_map([limit as i64], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            ))
        })?;

        let mut results = Vec::new();
        for item in history_iter {
            results.push(item?);
        }

        Ok(results)
    }

    /// Addresses last visited at or after `since` (Unix seconds).
    pub fn urls_since(&self, since: i64) -> Result<Vec<String>> {
        let mut statement = self
            .conn
            .prepare("SELECT url FROM history WHERE last_visit_time >= ?1")?;
        let rows = statement.query_map([since], |row| row.get(0))?;
        rows.collect()
    }

    /// Forgets pages last visited at or after `since` (Unix seconds); `None`
    /// forgets everything. Returns how many pages were removed.
    pub fn clear_since(&self, since: Option<i64>) -> Result<usize> {
        let removed = match since {
            Some(since) => self
                .conn
                .execute("DELETE FROM history WHERE last_visit_time >= ?1", [since])?,
            None => self.conn.execute("DELETE FROM history", [])?,
        };
        Ok(removed)
    }

    /// Removes a single page from history. Returns whether a row was removed.
    pub fn remove(&self, url: &str) -> Result<bool> {
        let removed = self
            .conn
            .execute("DELETE FROM history WHERE url = ?1", [url])?;
        Ok(removed > 0)
    }
}

fn history_fts_query(query: &str) -> Option<String> {
    let terms = query
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .take(8)
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>();

    let mut prefix = terms.join(" ");
    if prefix.is_empty() {
        return None;
    }
    if query.chars().last().is_some_and(char::is_alphanumeric) {
        prefix.push('*');
    }
    Some(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_removes_recent_pages_and_their_search_entries() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::open(dir.path()).unwrap();
        history
            .add_visit("https://old.test/", Some("Old page"))
            .unwrap();
        history
            .add_visit("https://new.test/", Some("New page"))
            .unwrap();
        history
            .conn
            .execute(
                "UPDATE history SET last_visit_time = 100 WHERE url = 'https://old.test/'",
                [],
            )
            .unwrap();
        assert_eq!(history.urls_since(1_000).unwrap(), ["https://new.test/"]);
        assert_eq!(history.clear_since(Some(1_000)).unwrap(), 1);
        assert_eq!(
            history.recent(10).unwrap(),
            [("https://old.test/".to_owned(), "Old page".to_owned())]
        );
        assert!(history.search("New").unwrap().is_empty());
        assert_eq!(history.clear_since(None).unwrap(), 1);
        assert!(history.recent(10).unwrap().is_empty());
    }

    #[test]
    fn search_accepts_url_like_input() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::open(dir.path()).expect("history database");
        history
            .add_visit(
                "https://duckduckgo.com/?q=rust+browser",
                Some("DuckDuckGo Search"),
            )
            .expect("add visit");

        let results = history
            .search("https://duckduckgo.com")
            .expect("search URL-like query");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "https://duckduckgo.com/?q=rust+browser");
    }

    #[test]
    fn imports_legacy_history_and_keeps_search_index_current_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE history (id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE, title TEXT,
                                   visit_count INTEGER DEFAULT 1, last_visit_time INTEGER NOT NULL);
             INSERT INTO history VALUES (17, 'https://example.test', 'Legacy title', 4, 100);",
        )
        .unwrap();
        drop(conn);

        let history = History::open(dir.path()).unwrap();
        assert_eq!(history.search("Legacy").unwrap().len(), 1);
        history
            .add_visit("https://example.test", Some("Updated title"))
            .unwrap();
        drop(history);

        let history = History::open(dir.path()).unwrap();
        assert!(history.search("Legacy").unwrap().is_empty());
        assert_eq!(
            history.search("Updated").unwrap()[0].0,
            "https://example.test"
        );
        let record: (i64, i64, i64) = history
            .conn
            .query_row(
                "SELECT id, visit_count, last_visit_time FROM history",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!((record.0, record.1), (17, 5));
        assert!(record.2 >= 100);
    }

    #[test]
    fn matches_a_partial_last_word_and_updates_title_without_another_visit() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::open(dir.path()).unwrap();
        history.add_visit("https://example.test", None).unwrap();
        history
            .update_title("https://example.test", "Local Landing")
            .unwrap();
        assert_eq!(history.search("Local Lan").unwrap()[0].1, "Local Landing");
        assert!(history.search("Local Lan ").unwrap().is_empty());
        assert_eq!(
            history
                .conn
                .query_row("SELECT visit_count FROM history", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn fts_query_escapes_punctuation_syntax() {
        assert_eq!(
            history_fts_query("https://example.com/path?q=rust-browser"),
            Some("\"https\" \"example\" \"com\" \"path\" \"q\" \"rust\" \"browser\"*".to_string())
        );
    }

    #[test]
    fn add_visit_accumulates_titles_and_counts() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::open(dir.path()).unwrap();
        history.add_visit("https://a.test/", Some("First")).unwrap();
        history
            .add_visit("https://a.test/", Some("Second"))
            .unwrap();
        history.add_visit("https://a.test/", None).unwrap();
        let (count, title): (i64, String) = history
            .conn
            .query_row(
                "SELECT visit_count, title FROM history WHERE url = 'https://a.test/'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 3);
        assert_eq!(title, "Second");
        history.update_title("https://a.test/", "Renamed").unwrap();
        let title: String = history
            .conn
            .query_row(
                "SELECT title FROM history WHERE url = 'https://a.test/'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(title, "Renamed");
    }

    #[test]
    fn recent_orders_newest_first_and_honors_limits() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::open(dir.path()).unwrap();
        assert!(history.recent(10).unwrap().is_empty());
        history.add_visit("https://a.test/", None).unwrap();
        history
            .conn
            .execute(
                "UPDATE history SET last_visit_time = 100 WHERE url = 'https://a.test/'",
                [],
            )
            .unwrap();
        history
            .add_visit("https://b.test/", Some("B page"))
            .unwrap();
        assert!(history.recent(0).unwrap().is_empty());
        assert_eq!(
            history.recent(1).unwrap(),
            [("https://b.test/".to_owned(), "B page".to_owned())]
        );
        let recent = history.recent(10).unwrap();
        assert_eq!(recent[0].0, "https://b.test/");
        assert_eq!(recent[1].0, "https://a.test/");
    }

    #[test]
    fn search_without_terms_falls_back_to_recent() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::open(dir.path()).unwrap();
        history
            .add_visit("https://a.test/", Some("A page"))
            .unwrap();
        assert_eq!(history.search("!!!").unwrap(), history.recent(10).unwrap());
    }

    #[test]
    fn clear_since_reports_how_many_rows_it_removed() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::open(dir.path()).unwrap();
        assert_eq!(history.clear_since(Some(1_000)).unwrap(), 0);
        history.add_visit("https://a.test/", None).unwrap();
        history
            .conn
            .execute(
                "UPDATE history SET last_visit_time = 5 WHERE url = 'https://a.test/'",
                [],
            )
            .unwrap();
        assert_eq!(history.clear_since(Some(10)).unwrap(), 0);
        assert_eq!(history.clear_since(Some(5)).unwrap(), 1);
        assert!(history.recent(10).unwrap().is_empty());
    }

    #[test]
    fn update_title_of_an_unknown_url_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::open(dir.path()).unwrap();
        history
            .update_title("https://missing.test/", "Nope")
            .unwrap();
        assert!(history.recent(10).unwrap().is_empty());
    }

    #[test]
    fn open_for_search_reads_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::open(dir.path()).unwrap();
        history
            .add_visit("https://a.test/", Some("A page"))
            .unwrap();
        let read_only = History::open_for_search(dir.path()).unwrap();
        assert_eq!(read_only.search("a.test").unwrap()[0].0, "https://a.test/");
        // A missing database is an error, not a silent empty one.
        let missing = tempfile::tempdir().unwrap();
        assert!(History::open_for_search(missing.path()).is_err());
    }

    #[test]
    fn remove_deletes_a_single_entry_and_its_search_record() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::open(dir.path()).unwrap();
        history
            .add_visit("https://keep.test/", Some("Keep page"))
            .unwrap();
        history
            .add_visit("https://drop.test/", Some("Drop page"))
            .unwrap();

        assert!(history.remove("https://drop.test/").unwrap());
        assert!(!history.remove("https://drop.test/").unwrap());

        assert_eq!(
            history.recent(10).unwrap(),
            [("https://keep.test/".to_owned(), "Keep page".to_owned())]
        );
        assert!(history.search("Drop").unwrap().is_empty());
        assert_eq!(history.search("Keep").unwrap().len(), 1);
    }
}
