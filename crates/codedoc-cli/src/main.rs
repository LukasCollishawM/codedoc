#![forbid(unsafe_code)]

mod git;
mod gitops;
mod import;
mod lifecycle;
mod migrate;
mod render;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use codedoc_anchor::{Anchor, locate};
use codedoc_context::{Target, assemble};
use codedoc_core::RepoPath;
use codedoc_graph::Graph;
use codedoc_index::Index;
use codedoc_lang::Registry;
use codedoc_ledger::{
    AnchorRole, Assurance, Author, Body, Evidence, Kind, Ledger, Lifecycle, RecordContent, Role,
    SCHEMA_VERSION, Scope, Timestamp, Workspace,
};
use codedoc_verify::Verifier;
use serde_json::{Value, json};

#[derive(Parser)]
#[command(name = "codedoc", about = "A semantic memory layer for source repositories", version)]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,

    #[arg(long, global = true)]
    json: bool,

    #[arg(long, global = true, value_name = "SHARED|LOCAL|GLOBAL")]
    scope: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum AuthorKind {
    Human,
    Agent,
    Analyzer,
    Runtime,
}

#[derive(Subcommand)]
enum GitCommand {
    InstallMergeDriver,

    MergeDriver { base: String, ours: String, theirs: String },
}

#[derive(clap::Args)]
struct AttachArgs {
    file: String,

    #[arg(long, conflicts_with = "line")]
    symbol: Option<String>,

    #[arg(long, conflicts_with = "symbol")]
    line: Option<u32>,

    #[arg(long)]
    kind: String,

    #[arg(long)]
    claim: String,

    #[arg(long)]
    detail: Option<String>,

    #[arg(long, default_value = "asserted")]
    assurance: String,

    #[arg(long, value_enum, default_value_t = AuthorKind::Human)]
    author: AuthorKind,

    #[arg(long, default_value = "unattributed")]
    identity: String,

    #[arg(long)]
    session: Option<String>,

    #[arg(long)]
    evidence: Vec<String>,

    #[arg(long)]
    supersedes: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    Init {
        #[arg(long, default_value = "shared")]
        scope: String,
    },

    Attach(AttachArgs),

    Verify,

    Context {
        target: String,

        #[arg(long)]
        symbol: Option<String>,

        #[arg(long, default_value_t = codedoc_context::DEFAULT_DEPTH)]
        depth: u8,

        #[arg(long)]
        budget: Option<usize>,
    },

    Reindex,

    List {
        #[arg(long)]
        file: Option<String>,

        #[arg(long)]
        symbol: Option<String>,
    },

    History {
        record: String,
    },

    Stats,

    Kinds,

    Supersede {
        record: String,

        #[arg(long)]
        claim: Option<String>,

        #[arg(long)]
        detail: Option<String>,

        #[arg(long)]
        kind: Option<String>,
    },

    Resolve {
        record: String,

        #[arg(long = "to-symbol", conflicts_with = "to_line")]
        to_symbol: Option<String>,

        #[arg(long = "to-line", conflicts_with = "to_symbol")]
        to_line: Option<u32>,

        #[arg(long = "in-file")]
        in_file: Option<String>,
    },

    Retract {
        record: String,

        #[arg(long)]
        reason: Option<String>,
    },

    Detached,

    Migrate {
        #[arg(long)]
        write: bool,
    },

    #[command(subcommand)]
    Git(GitCommand),

    Import {
        paths: Vec<String>,

        #[arg(long)]
        write: bool,

        #[arg(long)]
        limit: Option<usize>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(&cli) {
        Ok((value, code)) => {
            let rendered = if cli.json {
                serde_json::to_string_pretty(&value).unwrap_or_default()
            } else {
                render::human(&value)
            };
            match emit(&rendered) {
                Emission::Written => ExitCode::from(code as u8),
                Emission::ConsumerClosed => ExitCode::SUCCESS,
            }
        }
        Err(failure) => {
            if cli.json {
                let payload = json!({"error": failure.to_string()});
                let _ = emit(&serde_json::to_string_pretty(&payload).unwrap_or_default());
            } else {
                eprintln!("codedoc: {failure:#}");
            }
            ExitCode::from(4u8)
        }
    }
}

enum Emission {
    Written,
    ConsumerClosed,
}

fn emit(rendered: &str) -> Emission {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    match handle.write_all(rendered.as_bytes()).and_then(|()| handle.flush()) {
        Ok(()) => Emission::Written,
        Err(failure) if failure.kind() == std::io::ErrorKind::BrokenPipe => {
            Emission::ConsumerClosed
        }
        Err(_) => Emission::Written,
    }
}

fn dispatch(cli: &Cli) -> Result<(Value, i32)> {
    match &cli.command {
        Command::Init { scope } => command_init(&cli.root, scope),
        Command::Attach(args) => command_attach(&cli.root, args),
        Command::Verify => command_verify(&cli.root),
        Command::Context { target, symbol, depth, budget } => {
            command_context(&cli.root, target, symbol.as_deref(), *depth, *budget)
        }
        Command::Reindex => command_reindex(&cli.root),
        Command::List { file, symbol } => {
            command_list(&cli.root, file.as_deref(), symbol.as_deref())
        }
        Command::History { record } => command_history(&cli.root, record),
        Command::Stats => command_stats(&cli.root),
        Command::Kinds => Ok((json!({"command": "kinds", "kinds": Kind::vocabulary()}), 0)),
        Command::Supersede { record, claim, detail, kind } => lifecycle::supersede(
            &cli.root,
            record,
            claim.as_deref(),
            detail.as_deref(),
            kind.as_deref(),
        ),
        Command::Resolve { record, to_symbol, to_line, in_file } => lifecycle::resolve(
            &cli.root,
            record,
            &lifecycle::Relocation {
                symbol: to_symbol.as_deref(),
                line: *to_line,
                file: in_file.as_deref(),
            },
        ),
        Command::Retract { record, reason } => {
            lifecycle::retract(&cli.root, record, reason.as_deref())
        }
        Command::Detached => command_detached(&cli.root),
        Command::Migrate { write } => migrate::run(&cli.root, *write),
        Command::Git(GitCommand::InstallMergeDriver) => gitops::install_merge_driver(&cli.root),
        Command::Git(GitCommand::MergeDriver { base, ours, theirs }) => {
            gitops::merge_driver(base, ours, theirs)
        }
        Command::Import { paths, write, limit } => import::run(&cli.root, paths, *write, *limit),
    }
}

fn command_init(root: &Path, scope: &str) -> Result<(Value, i32)> {
    let scope =
        Scope::parse(scope).ok_or_else(|| anyhow!("scope must be shared, local or global"))?;
    let ledger = Ledger::initialise_scope(root, scope)
        .with_context(|| format!("initialising the {} ledger", scope.as_str()))?;
    Index::rebuild(&ledger).context("building the index")?;
    Ok((
        json!({
            "command": "init",
            "root": root.display().to_string(),
            "scope": scope.as_str(),
            "location": ledger.base().display().to_string(),
            "describes": scope.describe(),
            "leaves_repository_evidence": scope.leaves_repository_evidence(),
        }),
        0,
    ))
}

fn command_attach(root: &Path, args: &AttachArgs) -> Result<(Value, i32)> {
    let AttachArgs {
        file,
        symbol,
        line,
        kind,
        claim,
        detail,
        assurance,
        author,
        identity,
        session,
        evidence,
        supersedes,
    } = args;
    let (symbol, line) = (symbol.as_deref(), *line);
    let (detail, session) = (detail.as_deref(), session.as_deref());
    let supersedes = supersedes.as_deref();
    let author_kind = *author;

    let ledger = Ledger::open(root).context("opening the ledger")?;
    let kind = Kind::parse(kind)
        .ok_or_else(|| anyhow!("unknown kind {kind:?}; run `codedoc kinds` for the vocabulary"))?;
    let assurance = Assurance::parse(assurance)
        .ok_or_else(|| anyhow!("assurance must be asserted, inferred, or speculative"))?;

    let repo_path =
        RepoPath::parse(file).context("the target path must sit inside the repository")?;
    let adapter = Registry::for_path(&repo_path)
        .with_context(|| format!("no language adapter handles {file}"))?;
    let source = fs::read_to_string(root.join(repo_path.as_str()))
        .with_context(|| format!("reading {file}"))?;
    let tree = adapter.parse(&source).context("parsing the target file")?;

    let node = match (symbol, line) {
        (Some(wanted), _) => locate::by_symbol(&tree, &source, adapter, wanted)
            .ok_or_else(|| anyhow!("no symbol {wanted:?} found in {file}"))?,
        (None, Some(wanted)) => locate::by_line(&tree, adapter, wanted)
            .ok_or_else(|| anyhow!("line {wanted} does not cover any node in {file}"))?,
        (None, None) => bail!("attach requires either --symbol or --line"),
    };

    let anchor = Anchor::capture(repo_path, adapter, &source, node);
    let author = match author_kind {
        AuthorKind::Human => Author::Human { identity: identity.to_owned() },
        AuthorKind::Agent => Author::Agent {
            model: identity.to_owned(),
            session: session.unwrap_or("unrecorded").to_owned(),
        },
        AuthorKind::Analyzer => Author::Analyzer { name: identity.to_owned() },
        AuthorKind::Runtime => Author::Runtime { name: identity.to_owned() },
    };

    let content = RecordContent {
        schema: SCHEMA_VERSION,
        kind,
        anchors: vec![AnchorRole { role: Role::Subject, anchor: anchor.clone() }],
        body: Body { claim: claim.to_owned(), detail: detail.map(str::to_owned) },
        evidence: evidence.iter().map(|item| parse_evidence(item)).collect(),
        assurance,
        author,
        code_revision: git::head_revision(root),
        created: Timestamp::now(),
        lifecycle: Lifecycle::Active,
        parent: supersedes
            .map(str::parse)
            .transpose()
            .map_err(|_| anyhow!("--supersedes expects a record identifier"))?,
        chain: None,
        unrecognised: BTreeMap::new(),
    };

    let record = ledger.append(content).context("appending to the ledger")?;
    Index::append(&ledger, &record).context("refreshing the index")?;

    Ok((
        json!({
            "command": "attach",
            "record": record.id().to_string(),
            "kind": record.kind().as_str(),
            "file": anchor.file.as_str(),
            "symbol": anchor.symbol.as_ref().map(ToString::to_string),
            "range": anchor.range.to_string(),
            "claim": claim,
        }),
        0,
    ))
}

fn parse_evidence(raw: &str) -> Evidence {
    match raw.split_once(':') {
        Some(("git", value)) => value
            .parse()
            .map(Evidence::GitRevision)
            .unwrap_or_else(|_| Evidence::Document(raw.to_owned())),
        Some(("test", value)) => Evidence::Test(value.to_owned()),
        Some(("doc", value)) => Evidence::Document(value.to_owned()),
        Some(("record", value)) => value
            .parse()
            .map(Evidence::Record)
            .unwrap_or_else(|_| Evidence::Document(raw.to_owned())),
        Some(("http" | "https", _)) => Evidence::Url(raw.to_owned()),
        _ => Evidence::Document(raw.to_owned()),
    }
}

fn command_verify(root: &Path) -> Result<(Value, i32)> {
    let ledger = Ledger::open(root).context("opening the ledger")?;
    let report = Verifier::new(root).run(&ledger).context("verifying anchors")?;
    let code = report.exit_code();
    let payload = json!({
        "command": "verify",
        "records": report.records,
        "integrity_intact": report.integrity_intact,
        "orphaned_records": report
            .orphaned_records
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "counts": report.counts(),
        "findings": serde_json::to_value(&report.findings)?,
    });
    Ok((payload, code))
}

fn command_context(
    root: &Path,
    target: &str,
    symbol: Option<&str>,
    depth: u8,
    budget: Option<usize>,
) -> Result<(Value, i32)> {
    let ledger = Ledger::discover(root).context("opening the ledger")?;
    let index = match Index::open(ledger.base()) {
        Ok(index) => index,
        Err(_) => Index::rebuild(&ledger).context("building the index")?,
    };

    let (file, line) = match target.rsplit_once(':') {
        Some((path, number)) if number.parse::<u32>().is_ok() => {
            (path.to_owned(), number.parse::<u32>().ok())
        }
        _ => (target.to_owned(), None),
    };
    let normalised = RepoPath::parse(&file).map(|path| path.as_str().to_owned()).unwrap_or(file);

    let mut records = match (symbol, line) {
        (Some(wanted), _) => index.active_for_symbol(wanted),
        (None, Some(wanted)) => index.active_covering_line(&normalised, wanted),
        (None, None) => index.active_in_file(&normalised),
    }
    .context("querying the index")?;

    if depth > 0 {
        let symbols: Vec<String> = records
            .iter()
            .flat_map(|record| record.content().anchors.iter())
            .filter_map(|entry| entry.anchor.symbol.as_ref().map(ToString::to_string))
            .collect();
        records.extend(index.active_relations_touching(&symbols).context("querying relations")?);
    }

    let graph = Graph::from_records(records);
    let mut pack = assemble(
        &graph,
        Target { file: normalised, line, symbol: symbol.map(str::to_owned) },
        depth,
    );
    if let Some(limit) = budget {
        pack.fit_within(limit);
    }
    let payload = json!({
        "command": "context",
        "pack": serde_json::to_value(&pack)?,
        "claims": pack.claim_count(),
        "empty": pack.is_empty(),
    });
    Ok((payload, 0))
}

fn command_reindex(root: &Path) -> Result<(Value, i32)> {
    let ledger = Ledger::open(root).context("opening the ledger")?;
    let index = Index::rebuild(&ledger).context("rebuilding the index")?;
    Ok((
        json!({
            "command": "reindex",
            "records": index.record_count()?,
            "digest": index.content_digest()?.to_string(),
            "files": index.files()?,
        }),
        0,
    ))
}

fn command_list(root: &Path, file: Option<&str>, symbol: Option<&str>) -> Result<(Value, i32)> {
    let workspace = Workspace::discover(root).context("opening the ledger")?;
    let graph = Graph::across(&workspace).context("loading the knowledge graph")?;
    let records = match (file, symbol) {
        (_, Some(wanted)) => graph.for_symbol(wanted),
        (Some(wanted), None) => graph.in_file(wanted),
        (None, None) => graph.active(),
    };
    let rows: Vec<Value> = records
        .iter()
        .map(|record| {
            json!({
                "record": record.id().to_string(),
                "kind": record.kind().as_str(),
                "claim": record.content().body.claim,
                "file": record.subject().map(|anchor| anchor.file.as_str().to_owned()),
                "symbol": record
                    .subject()
                    .and_then(|anchor| anchor.symbol.as_ref().map(ToString::to_string)),
                "created": record.content().created.to_rfc3339(),
            })
        })
        .collect();
    Ok((json!({"command": "list", "count": rows.len(), "records": rows}), 0))
}

fn command_history(root: &Path, record: &str) -> Result<(Value, i32)> {
    let ledger = Ledger::open(root).context("opening the ledger")?;
    let graph = Graph::load(&ledger).context("loading the knowledge graph")?;
    let id = record.parse().map_err(|_| anyhow!("{record:?} is not a record identifier"))?;
    let chain = graph.supersession_chain(id);
    if chain.is_empty() {
        bail!("no record {record} in this ledger");
    }
    let rows: Vec<Value> = chain
        .iter()
        .map(|entry| {
            json!({
                "record": entry.id().to_string(),
                "kind": entry.kind().as_str(),
                "claim": entry.content().body.claim,
                "created": entry.content().created.to_rfc3339(),
                "code_revision": entry.content().code_revision.as_ref().map(ToString::to_string),
            })
        })
        .collect();
    Ok((json!({"command": "history", "revisions": rows.len(), "chain": rows}), 0))
}

fn command_detached(root: &Path) -> Result<(Value, i32)> {
    let workspace = Workspace::discover(root).context("opening the ledger")?;
    let report =
        Verifier::new(workspace.root()).run_across(&workspace).context("verifying anchors")?;
    let rows: Vec<Value> = report
        .findings
        .iter()
        .filter(|finding| finding.status == codedoc_verify::Status::Detached)
        .map(|finding| {
            json!({
                "record": finding.record.to_string(),
                "kind": finding.kind,
                "claim": finding.claim,
                "file": finding.file,
                "symbol": finding.symbol,
                "recorded_range": finding.recorded_range.to_string(),
                "resolution": serde_json::to_value(&finding.resolution).unwrap_or(Value::Null),
            })
        })
        .collect();
    let code = i32::from(!rows.is_empty()) * 2;
    Ok((json!({"command": "detached", "count": rows.len(), "records": rows}), code))
}

fn command_stats(root: &Path) -> Result<(Value, i32)> {
    let workspace = Workspace::discover(root).context("opening the ledger")?;
    let graph = Graph::across(&workspace).context("loading the knowledge graph")?;
    let verification = workspace.verify().context("verifying ledger integrity")?;
    Ok((
        json!({
            "command": "stats",
            "total_records": graph.all().len(),
            "active_records": graph.active().len(),
            "by_kind": graph.counts_by_kind(),
            "tips": verification.tips.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "integrity_intact": verification.is_intact(),
            "scopes": workspace.scopes().iter().map(|scope| scope.as_str()).collect::<Vec<_>>(),
        }),
        0,
    ))
}
