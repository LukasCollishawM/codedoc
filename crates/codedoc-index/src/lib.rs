#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use codedoc_core::{Digest, RecordId};
use codedoc_ledger::{Ledger, LedgerError, Record};
use rusqlite::{Connection, params};
use thiserror::Error;

pub const INDEX_FILE: &str = "index.sqlite";
pub const INDEX_SCHEMA_VERSION: i64 = 5;

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
    chain TEXT,
    canonical TEXT NOT NULL
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
CREATE INDEX records_by_parent ON records(parent) WHERE parent IS NOT NULL;
CREATE INDEX anchors_by_symbol_record ON anchors(symbol, record_id);
CREATE VIRTUAL TABLE claims USING fts5(
    record_id UNINDEXED,
    claim,
    detail,
    subject,
    tokenize = 'unicode61 remove_diacritics 2'
);
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
    pub fn path_for(base: &Path) -> PathBuf {
        base.join(INDEX_FILE)
    }

    pub fn rebuild(ledger: &Ledger) -> Result<Self, IndexError> {
        let path = Index::path_for(ledger.base());
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|source| IndexError::Storage { detail: source.to_string() })?;
        }
        let connection = Connection::open(&path)?;
        connection.pragma_update(None, "journal_mode", "DELETE")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.execute_batch(SCHEMA)?;
        connection.pragma_update(None, "user_version", INDEX_SCHEMA_VERSION)?;

        let mut records = ledger.records()?;
        records.sort_by_key(|record| record.id().to_string());
        let transaction = connection.unchecked_transaction()?;
        for record in &records {
            project(&connection, record)?;
        }
        transaction.commit()?;
        Ok(Index { connection, path })
    }

    pub fn open(base: &Path) -> Result<Self, IndexError> {
        let path = Index::path_for(base);
        let connection = Connection::open(&path)?;
        Ok(Index { connection, path })
    }

    pub fn is_current(base: &Path) -> bool {
        let path = Index::path_for(base);
        if !path.exists() {
            return false;
        }
        Connection::open(&path)
            .and_then(|connection| {
                connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            })
            .map(|version| version == INDEX_SCHEMA_VERSION)
            .unwrap_or(false)
    }

    pub fn current(ledger: &Ledger) -> Result<Self, IndexError> {
        if Index::is_current(ledger.base()) {
            Index::open(ledger.base())
        } else {
            Index::rebuild(ledger)
        }
    }

    pub fn append(ledger: &Ledger, record: &Record) -> Result<(), IndexError> {
        if !Index::is_current(ledger.base()) {
            Index::rebuild(ledger)?;
            return Ok(());
        }
        let connection = Connection::open(Index::path_for(ledger.base()))?;
        project(&connection, record)
    }

    pub fn append_many(ledger: &Ledger, records: &[Record]) -> Result<(), IndexError> {
        if !Index::is_current(ledger.base()) {
            Index::rebuild(ledger)?;
            return Ok(());
        }
        let connection = Connection::open(Index::path_for(ledger.base()))?;
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

    fn decode_rows(
        &self,
        sql: &str,
        bind: &[&dyn rusqlite::ToSql],
    ) -> Result<Vec<Record>, IndexError> {
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map(bind, |row| row.get::<_, String>(0))?;
        let mut records = Vec::new();
        for line in rows.filter_map(Result::ok) {
            if let Ok(record) = Record::decode_line(line.as_bytes()) {
                records.push(record);
            }
        }
        Ok(records)
    }

    pub fn active_in_file(&self, file: &str) -> Result<Vec<Record>, IndexError> {
        self.decode_rows(
            "SELECT DISTINCT r.canonical FROM records r
             JOIN anchors a ON a.record_id = r.id
             WHERE a.file = ?1 AND r.kind != 'tombstone'
               AND r.id NOT IN (SELECT parent FROM records WHERE parent IS NOT NULL)
             ORDER BY r.canonical",
            &[&file],
        )
    }

    pub fn active_covering_line(&self, file: &str, line: u32) -> Result<Vec<Record>, IndexError> {
        self.decode_rows(
            "SELECT DISTINCT r.canonical FROM records r
             JOIN anchors a ON a.record_id = r.id
             WHERE a.file = ?1 AND a.start_line <= ?2 AND a.end_line >= ?2
               AND r.kind != 'tombstone'
               AND r.id NOT IN (SELECT parent FROM records WHERE parent IS NOT NULL)
             ORDER BY r.canonical",
            &[&file, &line],
        )
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<(Record, f64)>, IndexError> {
        let expression = fts_expression(query);
        if expression.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = self.connection.prepare(
            "SELECT r.canonical, bm25(claims, 0.0, 4.0, 1.0, 3.0) AS relevance
             FROM claims JOIN records r ON r.id = claims.record_id
             WHERE claims MATCH ?1
               AND r.kind != 'tombstone'
               AND r.id NOT IN (SELECT parent FROM records WHERE parent IS NOT NULL)
             ORDER BY relevance
             LIMIT ?2",
        )?;
        let mut rows = statement.query(params![expression, limit as i64])?;
        let mut found = Vec::new();
        while let Some(row) = rows.next()? {
            let canonical: String = row.get(0)?;
            let relevance: f64 = row.get(1)?;
            let record = Record::decode_line(canonical.as_bytes()).map_err(|source| {
                IndexError::Projection { record: canonical.clone(), detail: source.to_string() }
            })?;
            found.push((record, -relevance));
        }
        Ok(found)
    }

    pub fn active_for_symbol(&self, symbol: &str) -> Result<Vec<Record>, IndexError> {
        self.decode_rows(
            "SELECT DISTINCT r.canonical FROM records r
             JOIN anchors a ON a.record_id = r.id
             WHERE a.symbol = ?1 AND r.kind != 'tombstone'
               AND r.id NOT IN (SELECT parent FROM records WHERE parent IS NOT NULL)
             ORDER BY r.canonical",
            &[&symbol],
        )
    }

    pub fn active_relations_touching(&self, symbols: &[String]) -> Result<Vec<Record>, IndexError> {
        if symbols.is_empty() {
            return Ok(Vec::new());
        }
        let mut unique: Vec<&String> = symbols.iter().collect();
        unique.sort();
        unique.dedup();

        let placeholders = vec!["?"; unique.len()].join(",");
        let sql = format!(
            "SELECT DISTINCT r.canonical FROM records r
             JOIN anchors a ON a.record_id = r.id
             WHERE a.symbol IN ({placeholders})
               AND r.kind >= 'relation.' AND r.kind < 'relation/'
               AND r.parent IS NULL
               AND r.id NOT IN (SELECT parent FROM records WHERE parent IS NOT NULL)
             ORDER BY r.canonical"
        );
        let bound: Vec<&dyn rusqlite::ToSql> =
            unique.iter().map(|symbol| *symbol as &dyn rusqlite::ToSql).collect();
        self.decode_rows(&sql, &bound)
    }

    pub fn files(&self) -> Result<Vec<String>, IndexError> {
        let mut statement =
            self.connection.prepare("SELECT DISTINCT file FROM anchors ORDER BY file")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(Result::ok).collect())
    }
}

pub fn fts_expression(query: &str) -> String {
    query
        .split(|glyph: char| !glyph.is_alphanumeric() && glyph != '_')
        .filter(|term| !term.is_empty())
        .map(
            |term| {
                if term.chars().count() >= 3 {
                    format!("\"{term}\"*")
                } else {
                    format!("\"{term}\"")
                }
            },
        )
        .collect::<Vec<_>>()
        .join(" OR ")
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
                              parent, chain, canonical)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
            String::from_utf8(record.encode_line().map_err(|source| IndexError::Projection {
                record: record.id().to_string(),
                detail: source.to_string(),
            })?)
            .map_err(|source| IndexError::Projection {
                record: record.id().to_string(),
                detail: source.to_string(),
            })?,
        ],
    )?;

    let subject: String = content
        .anchors
        .iter()
        .map(|entry| {
            let named = entry.anchor.symbol.as_ref().map(ToString::to_string).unwrap_or_default();
            format!("{} {named}", entry.anchor.file.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ");
    connection.execute(
        "INSERT INTO claims (record_id, claim, detail, subject) VALUES (?1, ?2, ?3, ?4)",
        params![record.id().to_string(), content.body.claim, content.body.detail, subject],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_sqlite_provides_full_text_search() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE VIRTUAL TABLE probe USING fts5(body);").expect(
            "search depends on fts5, which is a compile-time option in sqlite rather than something to discover at run time on a user's machine",
        );
    }

    #[test]
    fn a_query_never_reaches_the_full_text_parser_as_typed() {
        assert_eq!(fts_expression("tenant isolation"), "\"tenant\"* OR \"isolation\"*");
        assert_eq!(fts_expression("NEAR(a b)"), "\"NEAR\"* OR \"a\" OR \"b\"");
        assert_eq!(
            fts_expression("retry-after"),
            "\"retry\"* OR \"after\"*",
            "punctuation inside a word separates it; stripping it instead would fuse two terms into one that matches nothing"
        );
        assert_eq!(fts_expression("  "), "");
        assert_eq!(fts_expression("\"*-"), "");
    }
}
