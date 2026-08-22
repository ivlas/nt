use std::fs;
use std::io;
use std::path::Path;

use rusqlite::Connection;

use crate::error::{NtError, Result};
mod connection;
pub(crate) mod schema_engine;

use schema_engine::SchemaManifest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitOutcome {
    Initialized,
    AlreadyInitialized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenMode {
    ReadOnly,
    ReadWrite,
}

struct RemoveEmptyDirectory<'a>(Option<&'a Path>);

impl RemoveEmptyDirectory<'_> {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for RemoveEmptyDirectory<'_> {
    fn drop(&mut self) {
        if let Some(path) = self.0 {
            let _ = fs::remove_dir(path);
        }
    }
}

pub(crate) fn initialize_at(path: &Path, manifest: &SchemaManifest) -> Result<InitOutcome> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => initialize_existing(path, true, manifest),
        Ok(_) => Err(NtError::NotNtDatabase),
        Err(error) if error.kind() == io::ErrorKind::NotFound => initialize_missing_with(
            path,
            manifest,
            |connection| schema_engine::initialize(connection, manifest),
            connection::configure_wal,
        ),
        Err(error) => Err(NtError::path_io("inspect database path", path, error)),
    }
}

fn initialize_existing(
    path: &Path,
    establish_wal: bool,
    manifest: &SchemaManifest,
) -> Result<InitOutcome> {
    let permission_target = database_permission_target(path)
        .map_err(|error| NtError::path_io("open database permissions", path, error))?;
    let mut connection = connection::open_existing(path, OpenMode::ReadWrite)?;
    verify_database_permission_target(&permission_target, path)
        .map_err(|error| NtError::path_io("verify database permissions", path, error))?;
    connection::configure(&connection)?;
    let initialized = match schema_engine::inspect(&connection, manifest)? {
        schema_engine::Identity::Nt => false,
        schema_engine::Identity::Empty => {
            set_private_database_permissions(&permission_target)
                .map_err(|error| NtError::path_io("set database permissions", path, error))?;
            schema_engine::initialize(&mut connection, manifest)?
        }
    };
    let outcome = if initialized {
        InitOutcome::Initialized
    } else {
        InitOutcome::AlreadyInitialized
    };
    if establish_wal || initialized {
        connection::configure_wal(&connection)?;
    }
    Ok(outcome)
}

fn initialize_missing_with(
    path: &Path,
    manifest: &SchemaManifest,
    initialize: impl FnOnce(&mut Connection) -> Result<bool>,
    prepare_for_publish: impl FnOnce(&Connection) -> Result<()>,
) -> Result<InitOutcome> {
    let parent = path.parent().ok_or(NtError::InvalidDatabasePath)?;
    let parent_existed = parent.exists();
    create_database_directory(parent)
        .map_err(|error| NtError::path_io("create database directory", parent, error))?;
    let mut parent_cleanup = RemoveEmptyDirectory((!parent_existed).then_some(parent));
    let candidate = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| NtError::path_io("create temporary database in", parent, error))?;
    let permission_target = database_permission_target(candidate.path()).map_err(|error| {
        NtError::path_io(
            "open temporary database permissions",
            candidate.path(),
            error,
        )
    })?;
    set_private_database_permissions(&permission_target).map_err(|error| {
        NtError::path_io(
            "set temporary database permissions",
            candidate.path(),
            error,
        )
    })?;
    let mut connection = connection::open_existing(candidate.path(), OpenMode::ReadWrite)?;
    connection::configure(&connection)?;
    if !initialize(&mut connection)? {
        return Err(NtError::NotNtDatabase);
    }
    prepare_for_publish(&connection)?;
    drop(connection);

    match candidate.persist_noclobber(path) {
        Ok(file) => {
            drop(file);
            parent_cleanup.disarm();
            Ok(InitOutcome::Initialized)
        }
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            drop(error.file);
            parent_cleanup.disarm();
            initialize_existing(path, false, manifest)
        }
        Err(error) => Err(NtError::path_io("publish database", path, error.error)),
    }
}

fn create_database_directory(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;

        builder.mode(0o700);
    }
    builder.create(path)
}

#[cfg(unix)]
type DatabasePermissionTarget = fs::File;

#[cfg(not(unix))]
type DatabasePermissionTarget = ();

#[cfg(unix)]
fn database_permission_target(path: &Path) -> io::Result<DatabasePermissionTarget> {
    fs::OpenOptions::new().read(true).write(true).open(path)
}

#[cfg(not(unix))]
fn database_permission_target(_path: &Path) -> io::Result<DatabasePermissionTarget> {
    Ok(())
}

#[cfg(unix)]
fn verify_database_permission_target(
    target: &DatabasePermissionTarget,
    path: &Path,
) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let target = target.metadata()?;
    let path = fs::metadata(path)?;
    if target.dev() == path.dev() && target.ino() == path.ino() {
        Ok(())
    } else {
        Err(io::Error::other(
            "database path changed during initialization",
        ))
    }
}

#[cfg(not(unix))]
fn verify_database_permission_target(
    _target: &DatabasePermissionTarget,
    _path: &Path,
) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_database_permissions(target: &DatabasePermissionTarget) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    target.set_permissions(fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_database_permissions(_target: &DatabasePermissionTarget) -> io::Result<()> {
    Ok(())
}

pub(crate) fn open_at(
    path: &Path,
    mode: OpenMode,
    manifest: &SchemaManifest,
) -> Result<Connection> {
    let connection = connection::open_existing(path, mode)?;
    match schema_engine::inspect(&connection, manifest)? {
        schema_engine::Identity::Nt => {}
        schema_engine::Identity::Empty => return Err(NtError::NotNtDatabase),
    }
    match mode {
        OpenMode::ReadOnly => connection::configure(&connection)?,
        OpenMode::ReadWrite => connection::configure_wal(&connection)?,
    }
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use rusqlite::Connection;

    use super::*;
    use crate::schema::{self, MANIFEST};

    struct TestStorage {
        connection: Connection,
    }

    fn initialize_at(path: &Path) -> Result<InitOutcome> {
        super::initialize_at(path, &MANIFEST)
    }

    fn open_at(path: &Path, mode: OpenMode) -> Result<TestStorage> {
        super::open_at(path, mode, &MANIFEST).map(|connection| TestStorage { connection })
    }

    #[cfg(unix)]
    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;

        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    fn set_mode(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn initializes_and_reopens_a_clean_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".nt/nt.sqlite3");
        assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
        assert_eq!(
            initialize_at(&path).unwrap(),
            InitOutcome::AlreadyInitialized
        );
        let repository = open_at(&path, OpenMode::ReadWrite).unwrap();
        let foreign_keys: i64 = repository
            .connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        let journal: String = repository
            .connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        let busy_timeout: i64 = repository
            .connection
            .pragma_query_value(None, "busy_timeout", |row| row.get(0))
            .unwrap();
        assert_eq!(
            (foreign_keys, journal.as_str(), busy_timeout),
            (1, "wal", 5000)
        );
    }

    #[cfg(unix)]
    #[test]
    fn new_storage_directories_database_and_sidecars_are_private() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("private");
        let parent = first.join(".nt");
        let path = parent.join("nt.sqlite3");

        assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
        assert_eq!(mode(&first), 0o700);
        assert_eq!(mode(&parent), 0o700);
        assert_eq!(mode(&path), 0o600);

        let storage = open_at(&path, OpenMode::ReadWrite).unwrap();
        storage
            .connection
            .execute(
                "INSERT INTO notes(id, collection, body, title, created, updated)
                 VALUES ('018fbe0a-6c00-7000-8000-000000000001', 'inbox', '# Private',
                         'Private', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .unwrap();
        let file_name = path.file_name().unwrap().to_string_lossy();
        let wal = path.with_file_name(format!("{file_name}-wal"));
        let shm = path.with_file_name(format!("{file_name}-shm"));
        assert_eq!(mode(&wal), 0o600);
        assert_eq!(mode(&shm), 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn initialization_preserves_existing_directory_and_initialized_database_modes() {
        let directory = tempfile::tempdir().unwrap();
        let parent = directory.path().join(".nt");
        fs::create_dir(&parent).unwrap();
        set_mode(&parent, 0o750);
        let path = parent.join("nt.sqlite3");

        assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
        assert_eq!(mode(&parent), 0o750);
        set_mode(&path, 0o640);

        assert_eq!(
            initialize_at(&path).unwrap(),
            InitOutcome::AlreadyInitialized
        );
        assert_eq!(mode(&parent), 0o750);
        assert_eq!(mode(&path), 0o640);
    }

    #[cfg(unix)]
    #[test]
    fn initialization_makes_adopted_empty_databases_private() {
        for name in ["zero.sqlite3", "empty.sqlite3"] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join(name);
            if name == "zero.sqlite3" {
                fs::write(&path, []).unwrap();
            } else {
                drop(Connection::open(&path).unwrap());
            }
            set_mode(&path, 0o666);

            assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
            assert_eq!(mode(&path), 0o600);
        }
    }

    #[test]
    fn read_write_open_reestablishes_wal() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nt.sqlite3");
        initialize_at(&path).unwrap();
        let connection = Connection::open(&path).unwrap();
        let journal: String = connection
            .query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal, "delete");
        drop(connection);

        let repository = open_at(&path, OpenMode::ReadWrite).unwrap();
        let journal: String = repository
            .connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(journal, "wal");
    }

    #[test]
    fn wal_configuration_rejects_other_resulting_modes() {
        let connection = Connection::open_in_memory().unwrap();
        assert!(matches!(
            connection::configure_wal(&connection),
            Err(NtError::WalUnavailable)
        ));
    }

    #[test]
    fn ordinary_open_does_not_create_storage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".nt/nt.sqlite3");
        assert!(matches!(
            open_at(&path, OpenMode::ReadWrite),
            Err(NtError::MissingDatabase)
        ));
        assert!(!path.exists());
    }

    #[test]
    fn failed_new_initialization_leaves_no_database_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".nt/nt.sqlite3");

        let result = initialize_missing_with(
            &path,
            &MANIFEST,
            |connection| {
                schema::initialize_with(connection, |step| {
                    if step == 3 {
                        return Err(NtError::Io(io::Error::other(
                            "injected initialization failure",
                        )));
                    }
                    Ok(())
                })?;
                Ok(true)
            },
            connection::configure_wal,
        );

        assert!(result.is_err());
        assert!(!path.exists());
        assert!(!path.parent().unwrap().exists());
        assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
    }

    #[test]
    fn failed_wal_setup_does_not_publish_new_storage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".nt/nt.sqlite3");

        let result = initialize_missing_with(&path, &MANIFEST, schema::initialize, |_| {
            Err(NtError::WalUnavailable)
        });

        assert!(matches!(result, Err(NtError::WalUnavailable)));
        assert!(!path.exists());
        assert!(!path.parent().unwrap().exists());
        assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
    }

    #[test]
    fn initialization_rejects_paths_without_a_parent() {
        assert!(matches!(
            initialize_at(Path::new("")),
            Err(NtError::InvalidDatabasePath)
        ));
    }

    #[test]
    fn initialization_accepts_zero_length_and_empty_sqlite_files() {
        for name in ["zero.sqlite3", "empty.sqlite3"] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join(name);
            if name == "zero.sqlite3" {
                fs::write(&path, []).unwrap();
            } else {
                drop(Connection::open(&path).unwrap());
            }
            assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
            assert!(open_at(&path, OpenMode::ReadWrite).is_ok());
        }
    }

    #[test]
    fn unrelated_databases_are_rejected_without_modification() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("other.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("CREATE TABLE other(value TEXT)")
            .unwrap();
        drop(connection);
        let before = fs::read(&path).unwrap();
        assert!(matches!(initialize_at(&path), Err(NtError::NotNtDatabase)));
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn corrupt_database_errors_are_not_reported_as_foreign_schema() {
        for operation in [initialize_at as fn(&Path) -> Result<InitOutcome>, |path| {
            open_at(path, OpenMode::ReadWrite).map(|_| InitOutcome::AlreadyInitialized)
        }] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("corrupt.sqlite3");
            fs::write(&path, b"not a sqlite database").unwrap();

            assert!(matches!(operation(&path), Err(NtError::CorruptDatabase(_))));
        }
    }

    #[test]
    fn incompatible_nt_versions_are_reported() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nt.sqlite3");
        assert_eq!(initialize_at(&path).unwrap(), InitOutcome::Initialized);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "PRAGMA ignore_check_constraints = ON; UPDATE schema_version SET version = 3",
            )
            .unwrap();
        drop(connection);
        assert!(matches!(
            open_at(&path, OpenMode::ReadWrite),
            Err(NtError::UnsupportedSchema(3))
        ));
    }

    #[test]
    fn another_application_id_is_rejected_without_schema_changes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("other.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("PRAGMA application_id = 1234; CREATE TABLE other(value TEXT)")
            .unwrap();
        drop(connection);
        assert!(matches!(initialize_at(&path), Err(NtError::NotNtDatabase)));
        let connection = Connection::open(path).unwrap();
        let application_id: i64 = connection
            .pragma_query_value(None, "application_id", |row| row.get(0))
            .unwrap();
        assert_eq!(application_id, 1234);
        let table_exists: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'other')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_exists, 1);
    }

    #[test]
    fn concurrent_initializers_preserve_the_winning_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".nt/nt.sqlite3");
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    initialize_at(&path)
                })
            })
            .collect::<Vec<_>>();
        let outcomes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| **outcome == InitOutcome::Initialized)
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| **outcome == InitOutcome::AlreadyInitialized)
                .count(),
            7
        );
        assert!(open_at(&path, OpenMode::ReadWrite).is_ok());
    }
}
