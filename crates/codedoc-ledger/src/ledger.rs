use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use codedoc_core::{CanonicalError, LedgerHead, RecordId};
use thiserror::Error;

use crate::record::{Record, RecordContent};
use crate::scope::Scope;

pub const LEDGER_DIRECTORY: &str = ".codedoc";
pub const RECORDS_DIRECTORY: &str = "ledger";
pub const SHARD_EXTENSION: &str = "jsonl";

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LedgerError {
    #[error("no ledger found at {root}; run codedoc init")]
    Absent { root: String },

    #[error("a ledger already exists at {root}")]
    AlreadyPresent { root: String },

    #[error("ledger io failure at {path}: {detail}")]
    Io { path: String, detail: String },

    #[error("record on line {line} of {shard} is malformed: {source}")]
    Malformed { shard: String, line: usize, source: CanonicalError },

    #[error("record {record} could not be encoded: {source}")]
    Encoding { record: String, source: CanonicalError },

    #[error("the {scope} scope is not available at {root}")]
    ScopeUnavailable { scope: &'static str, root: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub records: usize,
    pub tips: Vec<RecordId>,
    pub orphans: Vec<RecordId>,
    pub duplicates: Vec<RecordId>,
}

impl Verification {
    pub fn is_intact(&self) -> bool {
        self.orphans.is_empty() && self.duplicates.is_empty()
    }
}

pub struct Ledger {
    root: PathBuf,
    base: PathBuf,
    scope: Scope,
}

impl Ledger {
    pub fn initialise(root: &Path) -> Result<Self, LedgerError> {
        Ledger::initialise_scope(root, Scope::Shared)
    }

    pub fn initialise_scope(root: &Path, scope: Scope) -> Result<Self, LedgerError> {
        let Some(base) = scope.directory(root) else {
            return Err(LedgerError::ScopeUnavailable {
                scope: scope.as_str(),
                root: root.display().to_string(),
            });
        };
        if base.exists() {
            return Err(LedgerError::AlreadyPresent { root: root.display().to_string() });
        }
        let records = base.join(RECORDS_DIRECTORY);
        fs::create_dir_all(&records).map_err(|source| LedgerError::Io {
            path: records.display().to_string(),
            detail: source.to_string(),
        })?;
        if scope.leaves_repository_evidence() {
            write_file(&base.join(".gitignore"), b"index.sqlite\nindex.sqlite-*\n")?;
        }
        write_file(
            &base.join("config.toml"),
            format!("schema = 1\nscope = \"{}\"\n", scope.as_str()).as_bytes(),
        )?;
        Ok(Ledger { root: root.to_path_buf(), base, scope })
    }

    pub fn discover(start: &Path) -> Result<Self, LedgerError> {
        let mut current = start.to_path_buf();
        loop {
            let present = Scope::ALL
                .into_iter()
                .filter_map(|scope| scope.directory(&current))
                .any(|directory| directory.is_dir());
            if present {
                return Ledger::open(&current);
            }
            if !current.pop() {
                return Err(LedgerError::Absent { root: start.display().to_string() });
            }
        }
    }

    pub fn open(root: &Path) -> Result<Self, LedgerError> {
        for scope in Scope::ALL {
            if let Ok(ledger) = Ledger::open_scope(root, scope) {
                return Ok(ledger);
            }
        }
        Err(LedgerError::Absent { root: root.display().to_string() })
    }

    pub fn open_scope(root: &Path, scope: Scope) -> Result<Self, LedgerError> {
        let Some(base) = scope.directory(root) else {
            return Err(LedgerError::ScopeUnavailable {
                scope: scope.as_str(),
                root: root.display().to_string(),
            });
        };
        if !base.is_dir() {
            return Err(LedgerError::Absent { root: root.display().to_string() });
        }
        Ok(Ledger { root: root.to_path_buf(), base, scope })
    }

    pub fn scope(&self) -> Scope {
        self.scope
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    pub fn open_or_initialise(root: &Path) -> Result<Self, LedgerError> {
        match Ledger::open(root) {
            Ok(ledger) => Ok(ledger),
            Err(LedgerError::Absent { .. }) => Ledger::initialise(root),
            Err(other) => Err(other),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn records_directory(&self) -> PathBuf {
        self.base.join(RECORDS_DIRECTORY)
    }

    fn shard_for(&self, id: RecordId) -> PathBuf {
        let rendered = id.to_string();
        let prefix = &rendered[..2];
        self.records_directory().join(format!("{prefix}.{SHARD_EXTENSION}"))
    }

    pub fn records(&self) -> Result<Vec<Record>, LedgerError> {
        let directory = self.records_directory();
        if !directory.is_dir() {
            return Ok(Vec::new());
        }
        let mut shards: Vec<PathBuf> = fs::read_dir(&directory)
            .map_err(|source| LedgerError::Io {
                path: directory.display().to_string(),
                detail: source.to_string(),
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == SHARD_EXTENSION))
            .collect();
        shards.sort();

        let mut collected = Vec::new();
        for shard in shards {
            let raw = fs::read(&shard).map_err(|source| LedgerError::Io {
                path: shard.display().to_string(),
                detail: source.to_string(),
            })?;
            for (offset, line) in raw.split(|byte| *byte == b'\n').enumerate() {
                if line.is_empty() {
                    continue;
                }
                let record =
                    Record::decode_line(line).map_err(|source| LedgerError::Malformed {
                        shard: shard.display().to_string(),
                        line: offset + 1,
                        source,
                    })?;
                collected.push(record);
            }
        }
        collected.sort_by_key(|record| (record.content().created, record.id().to_string()));
        Ok(collected)
    }

    pub fn tips(&self) -> Result<Vec<RecordId>, LedgerError> {
        let records = self.records()?;
        let referenced: BTreeSet<String> = records
            .iter()
            .filter_map(|record| record.content().chain.as_ref())
            .map(|head| head.to_string())
            .collect();
        Ok(records
            .iter()
            .map(Record::id)
            .filter(|id| !referenced.contains(&id.to_string()))
            .collect())
    }

    pub fn head(&self) -> Result<Option<LedgerHead>, LedgerError> {
        let mut tips = self.tips()?;
        tips.sort_by_key(RecordId::to_string);
        Ok(tips.last().and_then(|id| id.to_string().parse::<LedgerHead>().ok()))
    }

    pub fn append(&self, mut content: RecordContent) -> Result<Record, LedgerError> {
        content.chain = self.head()?;
        let record = Record::seal(content)
            .map_err(|source| LedgerError::Encoding { record: "pending".to_owned(), source })?;
        let line = record
            .encode_line()
            .map_err(|source| LedgerError::Encoding { record: record.id().to_string(), source })?;
        let shard = self.shard_for(record.id());
        if let Some(parent) = shard.parent() {
            fs::create_dir_all(parent).map_err(|source| LedgerError::Io {
                path: parent.display().to_string(),
                detail: source.to_string(),
            })?;
        }
        let mut handle =
            fs::OpenOptions::new().create(true).append(true).open(&shard).map_err(|source| {
                LedgerError::Io { path: shard.display().to_string(), detail: source.to_string() }
            })?;
        handle.write_all(&line).and_then(|()| handle.write_all(b"\n")).map_err(|source| {
            LedgerError::Io { path: shard.display().to_string(), detail: source.to_string() }
        })?;
        Ok(record)
    }

    pub fn append_batch(&self, drafts: Vec<RecordContent>) -> Result<Vec<Record>, LedgerError> {
        let mut head = self.head()?;
        let mut sealed = Vec::with_capacity(drafts.len());
        let mut shards: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();

        for mut content in drafts {
            content.chain = head;
            let record = Record::seal(content)
                .map_err(|source| LedgerError::Encoding { record: "pending".to_owned(), source })?;
            let line = record.encode_line().map_err(|source| LedgerError::Encoding {
                record: record.id().to_string(),
                source,
            })?;
            let buffer = shards.entry(self.shard_for(record.id())).or_default();
            buffer.extend_from_slice(&line);
            buffer.push(b'\n');
            head = record.id().to_string().parse().ok();
            sealed.push(record);
        }

        let directory = self.records_directory();
        fs::create_dir_all(&directory).map_err(|source| LedgerError::Io {
            path: directory.display().to_string(),
            detail: source.to_string(),
        })?;
        for (shard, buffer) in shards {
            let mut handle =
                fs::OpenOptions::new().create(true).append(true).open(&shard).map_err(
                    |source| LedgerError::Io {
                        path: shard.display().to_string(),
                        detail: source.to_string(),
                    },
                )?;
            handle.write_all(&buffer).map_err(|source| LedgerError::Io {
                path: shard.display().to_string(),
                detail: source.to_string(),
            })?;
        }
        Ok(sealed)
    }

    pub fn replace_all(&self, records: &[Record]) -> Result<(), LedgerError> {
        let directory = self.records_directory();
        if directory.is_dir() {
            let entries = fs::read_dir(&directory).map_err(|source| LedgerError::Io {
                path: directory.display().to_string(),
                detail: source.to_string(),
            })?;
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                let is_shard =
                    path.extension().is_some_and(|extension| extension == SHARD_EXTENSION);
                if is_shard {
                    fs::remove_file(&path).map_err(|source| LedgerError::Io {
                        path: path.display().to_string(),
                        detail: source.to_string(),
                    })?;
                }
            }
        }
        fs::create_dir_all(&directory).map_err(|source| LedgerError::Io {
            path: directory.display().to_string(),
            detail: source.to_string(),
        })?;

        let mut shards: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
        for record in records {
            let line = record.encode_line().map_err(|source| LedgerError::Encoding {
                record: record.id().to_string(),
                source,
            })?;
            let buffer = shards.entry(self.shard_for(record.id())).or_default();
            buffer.extend_from_slice(&line);
            buffer.push(b'\n');
        }
        for (shard, buffer) in shards {
            fs::write(&shard, &buffer).map_err(|source| LedgerError::Io {
                path: shard.display().to_string(),
                detail: source.to_string(),
            })?;
        }
        Ok(())
    }

    pub fn verify(&self) -> Result<Verification, LedgerError> {
        let records = self.records()?;
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for record in &records {
            *seen.entry(record.id().to_string()).or_insert(0) += 1;
        }
        let known: BTreeSet<String> = seen.keys().cloned().collect();
        let duplicates = seen
            .iter()
            .filter(|(_, count)| **count > 1)
            .filter_map(|(id, _)| id.parse::<RecordId>().ok())
            .collect();
        let orphans = records
            .iter()
            .filter(|record| {
                record
                    .content()
                    .chain
                    .as_ref()
                    .is_some_and(|head| !known.contains(&head.to_string()))
            })
            .map(Record::id)
            .collect();
        Ok(Verification { records: records.len(), tips: self.tips()?, orphans, duplicates })
    }
}

fn write_file(path: &Path, contents: &[u8]) -> Result<(), LedgerError> {
    fs::write(path, contents).map_err(|source| LedgerError::Io {
        path: path.display().to_string(),
        detail: source.to_string(),
    })
}
