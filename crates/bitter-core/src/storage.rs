use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, thiserror::Error)]
#[error("Could not open database {path}: {source}")]
pub struct DatabaseError {
    path: PathBuf,
    source: SchemaError,
}

#[derive(Debug, thiserror::Error)]
enum SchemaError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("schema version {0} is unsupported by this version of Bitter Browser")]
    Version(i64),
}

pub(crate) fn open(
    path: &Path,
    migration: &str,
    probes: &[&str],
) -> Result<Connection, DatabaseError> {
    let result = (|| {
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NOFOLLOW
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        if conn.is_readonly("main")? {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_READONLY),
                None,
            )
            .into());
        }
        upgrade(&mut conn, migration, probes)?;
        Ok(conn)
    })();
    result.map_err(|source| DatabaseError {
        path: path.to_owned(),
        source,
    })
}

fn upgrade(conn: &mut Connection, migration: &str, probes: &[&str]) -> Result<(), SchemaError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        0 => tx.execute_batch(migration)?,
        SCHEMA_VERSION => {}
        version => return Err(SchemaError::Version(version)),
    }
    for probe in probes {
        tx.prepare(probe)?;
    }
    if version == 0 {
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub(crate) fn open_read_only(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_millis(100))?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn preserves_corrupt_and_future_database_files() {
        let profile = tempfile::tempdir().unwrap();
        let corrupt = profile.path().join("corrupt.sqlite");
        let content = b"not a SQLite database".repeat(100);
        fs::write(&corrupt, &content).unwrap();
        assert!(open(&corrupt, "", &[]).is_err());
        assert_eq!(fs::read(corrupt).unwrap(), content);

        let future = profile.path().join("future.sqlite");
        let conn = Connection::open(&future).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        drop(conn);
        let before = fs::read(&future).unwrap();
        assert!(matches!(
            open(&future, "", &[]),
            Err(DatabaseError {
                source: SchemaError::Version(2),
                ..
            })
        ));
        assert_eq!(fs::read(future).unwrap(), before);
    }

    #[test]
    fn read_only_connection_cannot_migrate() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("read-only.sqlite");
        drop(Connection::open(&path).unwrap());
        let before = fs::read(&path).unwrap();
        let mut conn =
            Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let result = upgrade(&mut conn, "CREATE TABLE example (id INTEGER);", &[]);
        assert!(
            matches!(result, Err(SchemaError::Sqlite(error)) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ReadOnly))
        );
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn read_only_main_database_is_rejected() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("readonly.sqlite");
        drop(Connection::open(&path).unwrap());
        let mut perms = fs::metadata(&path).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o444);
        fs::set_permissions(&path, perms.clone()).unwrap();
        let result = open(&path, "CREATE TABLE example (id INTEGER);", &[]);
        perms.set_mode(0o644);
        fs::set_permissions(&path, perms).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn disk_full_rolls_back_schema_and_version_and_allows_retry() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("full.sqlite");
        let mut conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE legacy (value TEXT); INSERT INTO legacy VALUES ('keep me');",
        )
        .unwrap();
        let before = fs::read(&path).unwrap();
        conn.pragma_update(None, "max_page_count", 3).unwrap();
        let sql = "CREATE TABLE first (value TEXT); CREATE TABLE second (value TEXT);";
        let result = upgrade(&mut conn, sql, &[]);
        assert!(
            matches!(result, Err(SchemaError::Sqlite(error)) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DiskFull))
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 0);
        let value: String = conn
            .query_row("SELECT value FROM legacy", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "keep me");
        conn.pragma_update(None, "max_page_count", 100).unwrap();
        upgrade(&mut conn, sql, &[]).unwrap();
    }
}
