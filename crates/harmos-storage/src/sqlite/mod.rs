/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod columns;
mod files;
mod index;
mod projection;
mod rows;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{collections::BTreeMap, num::NonZeroUsize, path::Path, sync::Arc};

// Third Party Imports
use rusqlite::{Connection, OptionalExtension, params};
use tokio::sync::Mutex;

// Local Imports
use crate::query::columnar::{ArrowPage, ArrowScan, Cancellation, StorageCapabilities};
use crate::query::{FileSelection, ObjectSelection, RecordSelection, SchemaSelection, StreamSelection};
use crate::{
    BackendFailure, BackendSnapshot, Error, FileChunk, FileMetadata, FileReference, MutationReceipt, ObjectCatalog, ObjectEntry, ObjectKey,
    ObjectPoint, ReadLimits, Result, ScanPage, SchemaKey, StagedFile, StorageBackend, StorageBatch, StorageCursor, StorageFuture,
    StorageSchema, StoreHistory, StoredRecord, StoredRow,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy)]
pub struct SqliteSettings {
    pub chunk_rows: NonZeroUsize,
    pub chunk_bytes: NonZeroUsize,
}

pub struct SqliteBackend {
    history: StoreHistory,
    state: Arc<Mutex<State>>,
}

struct State {
    connection: Connection,
    _lease: Option<std::fs::File>,
    history: StoreHistory,
    settings: SqliteSettings,
    snapshots: BTreeMap<Vec<u8>, StorageCursor>,
    closed: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl SqliteBackend {
    pub fn new(path: impl AsRef<Path>, history: StoreHistory, settings: SqliteSettings) -> Result<Self> {
        let path = path.as_ref();
        let mut lock_path = path.as_os_str().to_os_string();
        lock_path.push(".lock");
        let lease = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(|source| Error::backend("open SQLite lease", BackendFailure::Permanent, source))?;
        lease.try_lock().map_err(|source| Error::backend("lock SQLite storage", BackendFailure::Conflict, source))?;
        Self::connect(Connection::open(path).map_err(db)?, history, settings, Some(lease))
    }

    pub fn in_memory(history: StoreHistory, settings: SqliteSettings) -> Result<Self> {
        Self::connect(Connection::open_in_memory().map_err(db)?, history, settings, None)
    }

    fn connect(mut connection: Connection, history: StoreHistory, settings: SqliteSettings, lease: Option<std::fs::File>) -> Result<Self> {
        integer(settings.chunk_rows.get() as u64)?;
        integer(settings.chunk_bytes.get() as u64)?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;").map_err(db)?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0)).map_err(db)?;
        let tables: i64 = connection
            .query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |row| row.get(0))
            .map_err(db)?;
        if version != 1 && (version != 0 || tables != 0) {
            return Err(incompatible());
        }
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db)?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS storage_meta(name TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS storage_rows(position INTEGER PRIMARY KEY, kind INTEGER NOT NULL, schema_key TEXT, schema_id TEXT, item TEXT, scope TEXT, stream TEXT, time_ns INTEGER, data BLOB NOT NULL);
            CREATE INDEX IF NOT EXISTS storage_source ON storage_rows(kind,schema_key,position);
            CREATE INDEX IF NOT EXISTS storage_object ON storage_rows(kind,schema_id,item,position);
            CREATE TABLE IF NOT EXISTS arrow_chunks(id INTEGER PRIMARY KEY, source TEXT NOT NULL, first INTEGER NOT NULL, last INTEGER NOT NULL, rows INTEGER NOT NULL);
            CREATE INDEX IF NOT EXISTS arrow_source ON arrow_chunks(source,last,first);
            CREATE TABLE IF NOT EXISTS arrow_columns(chunk INTEGER NOT NULL REFERENCES arrow_chunks(id), name TEXT NOT NULL, data BLOB NOT NULL, minimum REAL, maximum REAL, time_min INTEGER, time_max INTEGER, PRIMARY KEY(chunk,name));
            CREATE TABLE IF NOT EXISTS storage_files(id TEXT PRIMARY KEY, metadata BLOB NOT NULL, length INTEGER NOT NULL, finished BLOB);
            CREATE TABLE IF NOT EXISTS storage_file_chunks(id TEXT NOT NULL REFERENCES storage_files(id), offset INTEGER NOT NULL, data BLOB NOT NULL, PRIMARY KEY(id,offset));
            CREATE TABLE IF NOT EXISTS storage_mutations(id TEXT PRIMARY KEY, data BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS storage_indexes(name TEXT PRIMARY KEY,revision INTEGER NOT NULL,source TEXT NOT NULL,definition BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS storage_index_entries(name TEXT NOT NULL,revision INTEGER NOT NULL,key BLOB NOT NULL,position INTEGER NOT NULL,PRIMARY KEY(name,revision,key,position));
            CREATE TABLE IF NOT EXISTS projection_definitions(name TEXT NOT NULL,revision INTEGER NOT NULL,definition BLOB NOT NULL,retired INTEGER,PRIMARY KEY(name,revision));
            CREATE TABLE IF NOT EXISTS projection_active(name TEXT PRIMARY KEY,revision INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS projection_cuts(name TEXT NOT NULL,revision INTEGER NOT NULL,frontier INTEGER NOT NULL,rows INTEGER NOT NULL,changes INTEGER NOT NULL,schema BLOB NOT NULL,PRIMARY KEY(name,revision,frontier));
            CREATE TABLE IF NOT EXISTS projection_pages(name TEXT NOT NULL,revision INTEGER NOT NULL,frontier INTEGER NOT NULL,first_row INTEGER NOT NULL,rows INTEGER NOT NULL,data BLOB NOT NULL,PRIMARY KEY(name,revision,frontier,first_row));
            CREATE TABLE IF NOT EXISTS projection_changes(name TEXT NOT NULL,revision INTEGER NOT NULL,frontier INTEGER NOT NULL,ordinal INTEGER NOT NULL,key INTEGER NOT NULL,PRIMARY KEY(name,revision,frontier,ordinal));
            CREATE TABLE IF NOT EXISTS object_releases(schema_key TEXT NOT NULL, item TEXT NOT NULL, through INTEGER NOT NULL, PRIMARY KEY(schema_key,item));").map_err(db)?;
        let encoded = text(&history)?;
        let stored: Option<String> =
            tx.query_row("SELECT value FROM storage_meta WHERE name='history'", [], |row| row.get(0)).optional().map_err(db)?;
        if stored.as_ref().is_some_and(|stored| stored != &encoded) {
            return Err(incompatible());
        }
        tx.execute("INSERT OR IGNORE INTO storage_meta VALUES('history',?1)", [&encoded]).map_err(db)?;
        tx.execute_batch("INSERT OR IGNORE INTO storage_meta VALUES('saved','0'); INSERT OR IGNORE INTO storage_meta VALUES('snapshot','0'); INSERT OR IGNORE INTO storage_meta VALUES('floor','0'); PRAGMA user_version=1;").map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(Self {
            history: history.clone(),
            state: Arc::new(Mutex::new(State {
                connection,
                _lease: lease,
                history,
                settings,
                snapshots: BTreeMap::new(),
                closed: false,
            })),
        })
    }

    async fn call<T: Send + 'static>(&self, action: impl FnOnce(&mut State) -> Result<T> + Send + 'static) -> Result<T> {
        let mut state = self.state.clone().lock_owned().await;
        let context = crate::execution::Context::current();
        tokio::task::spawn_blocking(move || context.blocking(|| action(&mut state)))
            .await
            .map_err(|source| Error::backend("SQLite worker", BackendFailure::Permanent, source))?
    }
}

impl State {
    fn check(&self) -> Result<()> {
        if self.closed {
            return Err(Error::Closing { domain: "SQLite storage" });
        }
        Ok(())
    }

    fn cursor(&self) -> Result<StorageCursor> {
        Ok(StorageCursor {
            history: self.history.clone(),
            sequence: meta(&self.connection, "saved")?,
        })
    }

    fn snapshot(&self, snapshot: &BackendSnapshot) -> Result<()> {
        self.check()?;
        self.history.check(&snapshot.at.history)?;
        if self.snapshots.get(&snapshot.token) != Some(&snapshot.at) {
            return Err(Error::NotReady { domain: "expired storage snapshot" });
        }
        Ok(())
    }
}

impl StorageBackend for SqliteBackend {
    fn history(&self) -> &StoreHistory {
        &self.history
    }

    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities {
            native_arrow: true,
            native_index: true,
            durable_projections: true,
            ..StorageCapabilities::default()
        }
    }

    fn open(&self) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.call(|state| {
            state.closed = false;
            state.cursor()
        }))
    }
    fn cursor_saved(&self) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.call(|state| {
            state.check()?;
            state.cursor()
        }))
    }
    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(self.call(|state| {
            state.snapshots.clear();
            state.closed = true;
            Ok(())
        }))
    }

    fn write_schemas(&self, values: StorageBatch<StorageSchema>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.call(move |state| rows::write_schemas(state, values.as_slice())))
    }
    fn write_objects(&self, values: StorageBatch<ObjectEntry>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.call(move |state| rows::write_objects(state, values.as_slice())))
    }
    fn write_records(&self, values: StorageBatch<StoredRecord>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.call(move |state| rows::write_records(state, values.as_slice())))
    }
    fn write_stream(&self, values: StorageBatch<StoredRow>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.call(move |state| rows::write_stream(state, values.as_slice())))
    }
    fn write_files(&self, values: StorageBatch<FileReference>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.call(move |state| rows::write_files(state, values.as_slice())))
    }

    fn open_snapshot<'a>(&'a self, at: &'a StorageCursor) -> StorageFuture<'a, BackendSnapshot> {
        let at = at.clone();
        Box::pin(self.call(move |state| {
            state.check()?;
            state.history.check(&at.history)?;
            let floor = meta(&state.connection, "floor")?;
            if at.sequence < floor {
                return Err(Error::Gap { requested: at.sequence, available: floor });
            }
            if at.sequence > state.cursor()?.sequence {
                return Err(Error::NotReady { domain: "requested storage cut" });
            }
            let tx = state.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db)?;
            let token = meta(&tx, "snapshot")?.checked_add(1).ok_or_else(|| invalid("snapshot identifier overflow"))?;
            set_meta(&tx, "snapshot", token)?;
            tx.commit().map_err(db)?;
            let token = token.to_le_bytes().to_vec();
            state.snapshots.insert(token.clone(), at.clone());
            Ok(BackendSnapshot { at, token })
        }))
    }

    fn release_snapshot<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()> {
        let snapshot = snapshot.clone();
        Box::pin(self.call(move |state| {
            state.snapshot(&snapshot)?;
            state.snapshots.remove(&snapshot.token);
            Ok(())
        }))
    }

    fn read_schemas<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a SchemaSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StorageSchema>> {
        let (snapshot, selection) = (snapshot.clone(), selection.clone());
        Box::pin(self.call(move |state| rows::read_schemas(state, &snapshot, &selection, page)))
    }
    fn read_objects<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a ObjectSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<ObjectEntry>> {
        let (snapshot, selection) = (snapshot.clone(), selection.clone());
        Box::pin(self.call(move |state| rows::read_objects(state, &snapshot, &selection, page)))
    }
    fn read_records<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a RecordSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRecord>> {
        let (snapshot, selection) = (snapshot.clone(), selection.clone());
        Box::pin(self.call(move |state| rows::read_records(state, &snapshot, &selection, page)))
    }
    fn read_stream<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a StreamSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRow>> {
        let (snapshot, selection) = (snapshot.clone(), selection.clone());
        Box::pin(self.call(move |state| rows::read_stream(state, &snapshot, &selection, page)))
    }
    fn read_files<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a FileSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<FileReference>> {
        let (snapshot, selection) = (snapshot.clone(), selection.clone());
        Box::pin(self.call(move |state| rows::read_files(state, &snapshot, &selection, page)))
    }
    fn read_object<'a>(&'a self, snapshot: &'a BackendSnapshot, point: &'a ObjectPoint) -> StorageFuture<'a, Option<ObjectEntry>> {
        let (snapshot, point) = (snapshot.clone(), point.clone());
        Box::pin(self.call(move |state| rows::read_object(state, &snapshot, &point)))
    }
    fn release_object_history<'a>(
        &'a self, schema: &'a SchemaKey, item: &'a ObjectKey, through: &'a StorageCursor,
    ) -> StorageFuture<'a, ()> {
        let (schema, item, through) = (schema.clone(), item.clone(), through.clone());
        Box::pin(self.call(move |state| {
            state.check()?; state.history.check(&through.history)?;
            if through.sequence > state.cursor()?.sequence { return Err(invalid("release is ahead of durable data")); }
            state.connection.execute("INSERT INTO object_releases VALUES(?1,?2,?3) ON CONFLICT(schema_key,item) DO UPDATE SET through=max(through,excluded.through)", params![text(&schema)?, item.as_str(), integer(through.sequence)?]).map_err(db)?;
            Ok(())
        }))
    }
    fn prune_objects<'a>(
        &'a self, through: &'a StorageCursor, maximum: NonZeroUsize, retention: &'a ObjectCatalog,
    ) -> StorageFuture<'a, bool> {
        let (through, retention) = (through.clone(), retention.clone());
        Box::pin(self.call(move |state| rows::prune(state, &through, maximum, &retention)))
    }

    fn scan_arrow<'a>(
        &'a self, snapshot: &'a BackendSnapshot, read: &'a ArrowScan, cancel: &'a Cancellation,
    ) -> StorageFuture<'a, ArrowPage> {
        let (snapshot, read, cancel) = (snapshot.clone(), read.clone(), cancel.clone());
        Box::pin(self.call(move |state| columns::scan(state, &snapshot, &read, &cancel, false)))
    }

    fn register_index<'a>(
        &'a self, definition: &'a crate::query::index::IndexDefinition, rebuild: bool, work: ReadLimits, cancel: &'a Cancellation,
    ) -> StorageFuture<'a, u64> {
        let (definition, cancel) = (definition.clone(), cancel.clone());
        Box::pin(self.call(move |state| index::register(state, &definition, rebuild, work, &cancel)))
    }
    fn index_definition<'a>(&'a self, name: &'a crate::Identifier) -> StorageFuture<'a, (crate::query::index::IndexDefinition, u64)> {
        let name = name.clone();
        Box::pin(self.call(move |state| {
            state.check()?;
            index::definition(&state.connection, &name)
        }))
    }
    fn scan_index<'a>(
        &'a self, snapshot: &'a BackendSnapshot, read: &'a crate::query::index::IndexRead, cancel: &'a Cancellation,
    ) -> StorageFuture<'a, crate::query::index::IndexPage> {
        let (snapshot, read, cancel) = (snapshot.clone(), read.clone(), cancel.clone());
        Box::pin(self.call(move |state| index::scan(state, &snapshot, &read, &cancel)))
    }
    fn register_projection<'a>(
        &'a self, definition: &'a crate::query::projection::ProjectionDefinition, rebuild: bool,
    ) -> StorageFuture<'a, u64> {
        let definition = definition.clone();
        Box::pin(self.call(move |state| projection::register(state, &definition, rebuild)))
    }
    fn projection_definition<'a>(
        &'a self, name: &'a crate::Identifier,
    ) -> StorageFuture<'a, (crate::query::projection::ProjectionDefinition, u64)> {
        let name = name.clone();
        Box::pin(self.call(move |state| {
            state.check()?;
            projection::definition(&state.connection, &name)
        }))
    }
    fn projection_status<'a>(
        &'a self, name: &'a crate::Identifier, revision: u64, frontier: &'a StorageCursor,
    ) -> StorageFuture<'a, Option<crate::query::projection::ProjectionStatus>> {
        let (name, frontier) = (name.clone(), frontier.clone());
        Box::pin(self.call(move |state| projection::status(state, &name, revision, &frontier)))
    }
    fn projection_frontier<'a>(&'a self, name: &'a crate::Identifier, revision: u64) -> StorageFuture<'a, Option<StorageCursor>> {
        let name = name.clone();
        Box::pin(self.call(move |state| projection::frontier(state, &name, revision)))
    }
    fn publish_projection(&self, publication: crate::query::projection::ProjectionPublication) -> StorageFuture<'_, ()> {
        Box::pin(self.call(move |state| projection::publish(state, &publication)))
    }
    fn read_projection<'a>(
        &'a self, read: &'a crate::query::projection::ProjectionRead,
    ) -> StorageFuture<'a, crate::query::projection::ProjectionPage> {
        let read = read.clone();
        Box::pin(self.call(move |state| projection::read(state, &read)))
    }
    fn retire_projection<'a>(&'a self, name: &'a crate::Identifier, revision: u64, through: &'a StorageCursor) -> StorageFuture<'a, ()> {
        let (name, through) = (name.clone(), through.clone());
        Box::pin(self.call(move |state| projection::retire(state, &name, revision, &through)))
    }
    fn create_file<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        let metadata = metadata.clone();
        Box::pin(self.call(move |state| files::create(state, &metadata)))
    }
    fn write_file<'a>(&'a self, file: &'a StagedFile, offset: u64, bytes: FileChunk) -> StorageFuture<'a, ()> {
        let file = file.clone();
        Box::pin(self.call(move |state| files::write(state, &file, offset, bytes.as_slice())))
    }
    fn finish_file<'a>(
        &'a self, file: &'a StagedFile, position: &'a StorageCursor, length: u64, sha256: [u8; 32],
    ) -> StorageFuture<'a, FileReference> {
        let (file, position) = (file.clone(), position.clone());
        Box::pin(self.call(move |state| files::finish(state, &file, &position, length, sha256)))
    }
    fn abort_file<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, ()> {
        let file = file.clone();
        Box::pin(self.call(move |state| files::abort(state, &file)))
    }
    fn read_file<'a>(
        &'a self, snapshot: &'a BackendSnapshot, file: &'a FileReference, offset: u64, max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>> {
        let (snapshot, file) = (snapshot.clone(), file.clone());
        Box::pin(self.call(move |state| files::read(state, &snapshot, &file, offset, max_bytes)))
    }
    fn supports_mutation_receipts(&self) -> bool {
        true
    }
    fn save_mutations(&self, receipts: StorageBatch<MutationReceipt>) -> StorageFuture<'_, ()> {
        Box::pin(self.call(move |state| {
            state.check()?;
            let tx = state.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db)?;
            for receipt in receipts.as_slice() {
                let id = text(&receipt.mutation)?;
                let bytes = encode(receipt)?;
                let prior: Option<Vec<u8>> =
                    tx.query_row("SELECT data FROM storage_mutations WHERE id=?1", [&id], |row| row.get(0)).optional().map_err(db)?;
                if prior.as_ref().is_some_and(|prior| prior != &bytes) {
                    return Err(conflict("mutation receipt differs"));
                }
                tx.execute("INSERT OR IGNORE INTO storage_mutations VALUES(?1,?2)", params![id, bytes]).map_err(db)?;
            }
            tx.commit().map_err(db)
        }))
    }
    fn recover_mutations(&self, limits: ReadLimits) -> StorageFuture<'_, Vec<MutationReceipt>> {
        Box::pin(self.call(move |state| {
            state.check()?;
            let mut query = state.connection.prepare("SELECT data FROM storage_mutations ORDER BY id").map_err(db)?;
            let mut rows = query.query([]).map_err(db)?;
            let mut output = Vec::new();
            let mut bytes = 0;
            while let Some(row) = rows.next().map_err(db)? {
                let data: Vec<u8> = row.get(0).map_err(db)?;
                bytes += data.len();
                limits.check(output.len() + 1, bytes)?;
                output.push(decode(&data)?);
            }
            Ok(output)
        }))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn meta(connection: &Connection, name: &str) -> Result<u64> {
    let value: String = connection.query_row("SELECT value FROM storage_meta WHERE name=?1", [name], |row| row.get(0)).map_err(db)?;
    value.parse().map_err(|_| incompatible())
}
fn set_meta(connection: &Connection, name: &str, value: u64) -> Result<()> {
    connection.execute("UPDATE storage_meta SET value=?1 WHERE name=?2", params![value.to_string(), name]).map_err(db)?;
    Ok(())
}
fn unsigned(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value = row.get::<_, i64>(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
fn size(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<usize> {
    let value = row.get::<_, i64>(index)?;
    usize::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
fn integer(value: u64) -> Result<i64> {
    value.try_into().map_err(|_| invalid("value exceeds SQLite signed integer range"))
}
fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|source| Error::backend("encode SQLite value", BackendFailure::Permanent, source))
}
fn text<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|source| Error::backend("encode SQLite key", BackendFailure::Permanent, source))
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|source| Error::backend("decode SQLite value", BackendFailure::Corrupt, source))
}
fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "SQLite storage", reason }
}
fn conflict(reason: &'static str) -> Error {
    Error::backend("SQLite storage", BackendFailure::Conflict, std::io::Error::new(std::io::ErrorKind::InvalidData, reason))
}
fn incompatible() -> Error {
    Error::backend(
        "open SQLite storage",
        BackendFailure::Corrupt,
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "incompatible storage format or history; back up the original data, then reset explicitly to a fresh database",
        ),
    )
}
fn db(source: rusqlite::Error) -> Error {
    let kind = match source.sqlite_error_code() {
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => BackendFailure::Transient,
        Some(rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase) => BackendFailure::Corrupt,
        Some(rusqlite::ErrorCode::ConstraintViolation) => BackendFailure::Conflict,
        _ => BackendFailure::Permanent,
    };
    Error::backend("SQLite storage", kind, source)
}

/* === Tests === */
