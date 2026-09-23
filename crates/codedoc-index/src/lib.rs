#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use codedoc_core::{Digest, RecordId};
use codedoc_ledger::{Ledger, LedgerError, Record};
use rusqlite::{Connection, params};
use thiserror::Error;

pub const INDEX_FILE: &str = "index.sqlite";

const SCHEMA: &str = "
CREATE TABLE records (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    claim TEXT NOT NULL,
    detail TEXT,
    assurance TEXT NOT NULL,
    author TEXT NOT NULL,
    created INTEGER NOT NULL,
    lifecycle TEXT NOT NULL,
    parent TEXT,
    chain TEXT
) STRICT;
CREATE TABLE anchors (
    record_id TEXT NOT NULL REFERENCES records(id),
    role TEXT NOT NULL,
    file TEXT NOT NULL,
    language TEXT NOT NULL,
    symbol TEXT,
    node_kind TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    content_fingerprint TEXT NOT NULL,
    structural_fingerprint TEXT NOT NULL
) STRICT;
CREATE INDEX anchors_by_file ON anchors(file);
CREATE INDEX anchors_by_symbol ON anchors(symbol);
CREATE INDEX records_by_kind ON records(kind);
";

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IndexError {
    #[error("index storage failure: {detail}")]
    Storage { detail: String },

    #[error("the ledger could not be read while indexing: {source}")]
    Ledger {
        #[from]
        source: LedgerError,
    },

    #[error("record {record} could not be projected: {detail}")]
    Projection { record: String, detail: String },
}

impl From<rusqlite::Error> for IndexError {
    fn from(source: rusqlite::Error) -> Self {
        IndexError::Storage { detail: source.to_string() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedAnchor {
    pub record_id: RecordId,
    pub kind: String,
    pub claim: String,
    pub file: String,
    pub symbol: Option<String>,
    pub node_kind: String,
    pub start_line: u32,
    pub end_line: u32,
    pub lifecycle: String,
}

pub struct Index {
    connection: Connection,
    path: PathBuf,
}

impl Index {
    pub fn path_for(root: &Path) -> PathBuf {
        root.join(codedoc_ledger::ledger::LEDGER_DIRECTORY).join(INDEX_FILE)
    }

    pub fn rebuild(ledger: &Ledger) -> Result<Self, IndexError> {
        let path = Index::path_for(ledger.root());
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|source| IndexError::Storage { detail: source.to_string() })?;
        }
        let connection = Connection::open(&path)?;
        connection.pragma_update(None, "journal_mode", "DELETE")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.execute_batch(SCHEMA)?;

        let mut records = ledger.records()?;
        records.sort_by_key(|record| record.id().to_string());
        let transaction = connection.unchecked_transaction()?;
        for record in &records {
            project(&connection, record)?;
        }
        transaction.commit()?;
        Ok(Index { connection, path })
    }

    pub fn open(root: &Path) -> Result<Self, IndexError> {
        let path = Index::path_for(root);
        let connection = Connection::open(&path)?;
        Ok(Index { connection, path })
    }

    pub fn append(ledger: &Ledger, record: &Record) -> Result<(), IndexError> {
        let path = Index::path_for(ledger.root());
        if !path.exists() {
            Index::rebuild(ledger)?;
            return Ok(());
        }
        let connection = Connection::open(&path)?;
        project(&connection, record)
    }

    pub fn append_many(ledger: &Ledger, records: &[Record]) -> Result<(), IndexError> {
        let path = Index::path_for(ledger.root());
        if !path.exists() {
            Index::rebuild(ledger)?;
            return Ok(());
        }
        let connection = Connection::open(&path)?;
        let transaction = connection.unchecked_transaction()?;
        for record in records {
            project(&connection, record)?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn record_count(&self) -> Result<usize, IndexError> {
        let count: i64 =
            self.connection.query_row("SELECT count(*) FROM records", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    pub fn content_digest(&self) -> Result<Digest, IndexError> {
        let mut payload = Vec::new();
        let mut statement = self.connection.prepare(
            "SELECT id, kind, claim, ifnull(detail,''), assurance, author, created, lifecycle,
                    ifnull(parent,''), ifnull(chain,'')
             FROM records ORDER BY id",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            for column in 0..10 {
                let value: String = row.get_ref(column)?.as_str().map(str::to_owned).or_else(
                    |_| -> Result<String, rusqlite::Error> {
                        Ok(row.get::<_, i64>(column)?.to_string())
                    },
                )?;
                payload.extend_from_slice(value.as_bytes());
                payload.push(0);
            }
        }
        let mut anchor_statement = self.connection.prepare(
            "SELECT record_id, role, file, ifnull(symbol,''), node_kind, start_line, end_line,
                    content_fingerprint, structural_fingerprint
             FROM anchors ORDER BY record_id, role, file, start_line",
        )?;
        let mut anchor_rows = anchor_statement.query([])?;
        while let Some(row) = anchor_rows.next()? {
            for column in 0..9 {
                let value: String = row.get_ref(column)?.as_str().map(str::to_owned).or_else(
                    |_| -> Result<String, rusqlite::Error> {
                        Ok(row.get::<_, i64>(column)?.to_string())
                    },
                )?;
                payload.extend_from_slice(value.as_bytes());
                payload.push(0);
            }
        }
        Ok(Digest::of_domain("codedoc.index.digest.v1", &payload))
    }

    pub fn anchors_in_file(&self, file: &str) -> Result<Vec<IndexedAnchor>, IndexError> {
        let mut statement = self.connection.prepare(
            "SELECT a.record_id, r.kind, r.claim, a.file, a.symbol, a.node_kind,
                    a.start_line, a.end_line, r.lifecycle
             FROM anchors a JOIN records r ON r.id = a.record_id
             WHERE a.file = ?1
             ORDER BY a.start_line, a.record_id",
        )?;
        let rows = statement.query_map(params![file], |row| {
            Ok(IndexedAnchor {
                record_id: row
                    .get::<_, String>(0)?
                    .parse()
                    .unwrap_or_else(|_| RecordId::of(b"unparsable")),
                kind: row.get(1)?,
                claim: row.get(2)?,
                file: row.get(3)?,
                symbol: row.get(4)?,
                node_kind: row.get(5)?,
                start_line: row.get::<_, i64>(6)? as u32,
                end_line: row.get::<_, i64>(7)? as u32,
                lifecycle: row.get(8)?,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn anchors_for_symbol(&self, symbol: &str) -> Result<Vec<IndexedAnchor>, IndexError> {
        let mut statement = self.connection.prepare(
            "SELECT a.record_id, r.kind, r.claim, a.file, a.symbol, a.node_kind,
                    a.start_line, a.end_line, r.lifecycle
             FROM anchors a JOIN records r ON r.id = a.record_id
             WHERE a.symbol = ?1
             ORDER BY a.start_line, a.record_id",
        )?;
        let rows = statement.query_map(params![symbol], |row| {
            Ok(IndexedAnchor {
                record_id: row
                    .get::<_, String>(0)?
                    .parse()
                    .unwrap_or_else(|_| RecordId::of(b"unparsable")),
                kind: row.get(1)?,
                claim: row.get(2)?,
                file: row.get(3)?,
                symbol: row.get(4)?,
                node_kind: row.get(5)?,
                start_line: row.get::<_, i64>(6)? as u32,
                end_line: row.get::<_, i64>(7)? as u32,
                lifecycle: row.get(8)?,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn files(&self) -> Result<Vec<String>, IndexError> {
        let mut statement =
            self.connection.prepare("SELECT DISTINCT file FROM anchors ORDER BY file")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(Result::ok).collect())
    }
}

fn project(connection: &Connection, record: &Record) -> Result<(), IndexError> {
    let content = record.content();
    let kind = content.kind.as_str();
    let assurance = content.assurance.as_str();
    let author = serde_json::to_string(&content.author).map_err(|source| {
        IndexError::Projection { record: record.id().to_string(), detail: source.to_string() }
    })?;
    let lifecycle = content.lifecycle.as_str();

    connection.execute(
        "INSERT INTO records (id, kind, claim, detail, assurance, author, created, lifecycle,
                              parent, chain)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            record.id().to_string(),
            kind,
            content.body.claim,
            content.body.detail,
            assurance,
            author,
            content.created.unix_seconds(),
            lifecycle,
            content.parent.map(|parent| parent.to_string()),
            content.chain.map(|chain| chain.to_string()),
        ],
    )?;

    for entry in &content.anchors {
        let role = entry.role.as_str();
        connection.execute(
            "INSERT INTO anchors (record_id, role, file, language, symbol, node_kind,
                                  start_line, end_line, content_fingerprint,
                                  structural_fingerprint)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.id().to_string(),
                role,
                entry.anchor.file.as_str(),
                entry.anchor.language,
                entry.anchor.symbol.as_ref().map(ToString::to_string),
                entry.anchor.node_kind,
                i64::from(entry.anchor.range.start_line),
                i64::from(entry.anchor.range.end_line),
                entry.anchor.content.to_string(),
                entry.anchor.structural.to_string(),
            ],
        )?;
    }
    Ok(())
}
