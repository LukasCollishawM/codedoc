#![forbid(unsafe_code)]

mod gitops;
mod migrate;
mod render;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand, ValueEnum};
use codedoc_index::Index;
use codedoc_ledger::{Evidence, Kind, Ledger, Scope};
use codedoc_ops as ops;
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

#[derive(clap::Args)]
struct RelateArgs {
    #[arg(help = "The place the fact is about, as path@symbol or path:line")]
    subject: String,

    #[arg(help = "must_execute_after, guarded_by, constrained_by, invalidates, tested_by, \
                  derived_from, contradicts, supersedes or owns")]
    verb: String,

    #[arg(help = "The place at the other end of the link, as path@symbol or path:line")]
    object: String,

    #[arg(long)]
    claim: Option<String>,

    #[arg(long)]
    detail: Option<String>,

    #[arg(long, value_enum, default_value_t = AuthorKind::Human)]
    author: AuthorKind,

    #[arg(long, default_value = "unattributed")]
    identity: String,
}

#[derive(Subcommand)]
enum GitCommand {
    InstallMergeDriver,

    MergeDriver { base: String, ours: String, theirs: String },
}

#[derive(clap::Args)]
struct AttachArgs {
    #[arg(help = "The file the claim is about, relative to the repository root")]
    file: String,

    #[arg(
        long,
        conflicts_with = "line",
        help = "The construct inside that file, as the adapter names it, such as rust://validate"
    )]
    symbol: Option<String>,

    #[arg(
        long,
        conflicts_with = "symbol",
        help = "A line inside the construct, when its name is not to hand. Without either, \
                the claim is about the file"
    )]
    line: Option<u32>,

    #[arg(long, help = "One of `codedoc kinds`, such as invariant, security or rationale")]
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

    Verify {
        files: Vec<String>,

        #[arg(long)]
        since: Option<String>,
    },

    Context {
        target: String,

        #[arg(long)]
        symbol: Option<String>,

        #[arg(long, default_value_t = codedoc_context::DEFAULT_DEPTH)]
        depth: u8,

        #[arg(long)]
        budget: Option<usize>,

        #[arg(long)]
        as_of: Option<String>,
    },

    Reindex,

    List {
        #[arg(long)]
        limit: Option<usize>,

        #[arg(long)]
        file: Option<String>,

        #[arg(long)]
        symbol: Option<String>,

        #[arg(long)]
        as_of: Option<String>,

        #[arg(long)]
        author: Option<String>,
    },

    Search {
        query: Vec<String>,

        #[arg(long)]
        kind: Option<String>,

        #[arg(long)]
        file: Option<String>,

        #[arg(long, default_value_t = 20)]
        limit: usize,
    },

    Affirm {
        record: String,

        #[arg(long)]
        assurance: Option<String>,

        #[arg(long, value_enum, default_value_t = AuthorKind::Human)]
        author: AuthorKind,

        #[arg(long, default_value = "unattributed")]
        identity: String,

        #[arg(long)]
        session: Option<String>,
    },

    History {
        reference: String,
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

    Conflicts,

    Evidence,

    Doctor,

    Brief {
        files: Vec<String>,

        #[arg(long)]
        since: Option<String>,

        #[arg(long, default_value_t = 0)]
        depth: u8,

        #[arg(long)]
        budget: Option<usize>,
    },

    Coverage {
        paths: Vec<String>,

        #[arg(long, default_value_t = 10)]
        limit: usize,
    },

    Gaps {
        paths: Vec<String>,

        #[arg(long, default_value_t = 10)]
        limit: usize,

        #[arg(long, default_value_t = 400)]
        commits: usize,
    },

    Review {
        #[arg(default_value = "HEAD")]
        base: String,

        #[arg(long)]
        out: Option<String>,
    },

    Repair {
        #[arg(long)]
        write: bool,
    },

    Render {
        #[arg(default_value = "markdown")]
        format: String,

        #[arg(long, default_value = "Repository knowledge")]
        title: String,

        #[arg(long)]
        out: Option<String>,
    },

    Relate(RelateArgs),

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

fn parsed() -> Result<Cli, ExitCode> {
    match Cli::try_parse() {
        Ok(cli) => Ok(cli),
        Err(complaint) => {
            let _ = complaint.print();
            Err(match complaint.kind() {
                clap::error::ErrorKind::DisplayHelp
                | clap::error::ErrorKind::DisplayVersion
                | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
                    ExitCode::SUCCESS
                }
                _ => ExitCode::from(4),
            })
        }
    }
}

fn main() -> ExitCode {
    let cli = match parsed() {
        Ok(cli) => cli,
        Err(code) => return code,
    };
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
            let code =
                failure.downcast_ref::<ops::OpsError>().map(ops::OpsError::exit_code).unwrap_or(4);
            ExitCode::from(code)
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
    let scope = match cli.scope.as_deref() {
        Some(named) => Some(
            Scope::parse(named).ok_or_else(|| anyhow!("scope must be shared, local or global"))?,
        ),
        None => None,
    };
    match &cli.command {
        Command::Init { scope } => command_init(&cli.root, scope),
        Command::Attach(args) => command_attach(&cli.root, scope, args),
        Command::Verify { files, since } => {
            Ok(ops::verify_scoped(&cli.root, files, since.as_deref())?)
        }
        Command::Context { target, symbol, depth, budget, as_of } => {
            command_context(&cli.root, target, symbol.as_deref(), *depth, *budget, as_of.as_deref())
        }
        Command::Reindex => command_reindex(&cli.root),
        Command::List { limit, file, symbol, as_of, author } => command_list(
            &cli.root,
            scope,
            file.as_deref(),
            symbol.as_deref(),
            as_of.as_deref(),
            author.as_deref(),
            *limit,
        ),
        Command::Search { query, kind, file, limit } => Ok((
            ops::search(
                &cli.root,
                scope,
                &query.join(" "),
                kind.as_deref(),
                file.as_deref(),
                *limit,
            )?,
            0,
        )),
        Command::Affirm { record, assurance, author, identity, session } => {
            let attribution = match author {
                AuthorKind::Human => ops::Attribution::human(identity),
                AuthorKind::Agent => {
                    ops::Attribution::agent(identity, session.as_deref().unwrap_or("unrecorded"))
                }
                AuthorKind::Analyzer => ops::Attribution::analyzer(identity),
                AuthorKind::Runtime => ops::Attribution::runtime(identity),
            };
            Ok((
                ops::affirm(
                    &cli.root,
                    scope,
                    record,
                    &attribution,
                    ops::parse_assurance(assurance.as_deref())?,
                )?,
                0,
            ))
        }
        Command::History { reference } => command_history(&cli.root, reference),
        Command::Stats => command_stats(&cli.root, scope),
        Command::Kinds => Ok((json!({"command": "kinds", "kinds": Kind::vocabulary()}), 0)),
        Command::Supersede { record, claim, detail, kind } => Ok((
            ops::supersede(
                &cli.root,
                scope,
                record,
                claim.as_deref(),
                detail.as_deref(),
                kind.as_deref(),
            )?,
            0,
        )),
        Command::Resolve { record, to_symbol, to_line, in_file } => Ok((
            ops::resolve(
                &cli.root,
                scope,
                record,
                &ops::Target {
                    file: in_file.clone().unwrap_or_default(),
                    symbol: to_symbol.clone(),
                    line: *to_line,
                },
            )?,
            0,
        )),
        Command::Retract { record, reason } => {
            Ok((ops::retract(&cli.root, scope, record, reason.as_deref())?, 0))
        }
        Command::Relate(args) => command_relate(&cli.root, scope, args),
        Command::Detached => command_detached(&cli.root),
        Command::Conflicts => Ok(ops::conflicts(&cli.root)?),
        Command::Evidence => Ok(ops::evidence(&cli.root)?),
        Command::Doctor => Ok(ops::doctor(&cli.root)?),
        Command::Brief { files, since, depth, budget } => {
            Ok((ops::brief(&cli.root, files, since.as_deref(), *depth, *budget)?, 0))
        }
        Command::Coverage { paths, limit } => Ok((ops::coverage(&cli.root, paths, *limit)?, 0)),
        Command::Gaps { paths, limit, commits } => {
            Ok((ops::gaps(&cli.root, paths, *limit, *commits)?, 0))
        }
        Command::Review { base, out } => {
            let (payload, code) = ops::review(&cli.root, base)?;
            if let Some(path) = out {
                let body = payload["output"].as_str().unwrap_or_default();
                std::fs::write(path, body).with_context(|| format!("writing {path}"))?;
            }
            Ok((payload, code))
        }
        Command::Repair { write } => Ok((ops::repair(&cli.root, scope, *write)?, 0)),
        Command::Render { format, title, out } => {
            let payload = ops::render(&cli.root, format, title)?;
            if let Some(path) = out {
                let body = payload["output"].as_str().unwrap_or_default();
                std::fs::write(path, body).with_context(|| format!("writing {path}"))?;
            }
            Ok((payload, 0))
        }
        Command::Migrate { write } => migrate::run(&cli.root, *write),
        Command::Git(GitCommand::InstallMergeDriver) => gitops::install_merge_driver(&cli.root),
        Command::Git(GitCommand::MergeDriver { base, ours, theirs }) => {
            gitops::merge_driver(base, ours, theirs)
        }
        Command::Import { paths, write, limit } => {
            Ok((ops::import(&cli.root, scope, paths, *write, *limit)?, 0))
        }
    }
}

fn command_init(root: &Path, scope: &str) -> Result<(Value, i32)> {
    let scope =
        Scope::parse(scope).ok_or_else(|| anyhow!("scope must be shared, local or global"))?;
    Ok((ops::initialise(root, scope)?, 0))
}

fn command_attach(root: &Path, scope: Option<Scope>, args: &AttachArgs) -> Result<(Value, i32)> {
    let attribution = match args.author {
        AuthorKind::Human => ops::Attribution::human(&args.identity),
        AuthorKind::Agent => {
            ops::Attribution::agent(&args.identity, args.session.as_deref().unwrap_or("unrecorded"))
        }
        AuthorKind::Analyzer => ops::Attribution::analyzer(&args.identity),
        AuthorKind::Runtime => ops::Attribution::runtime(&args.identity),
    };
    let request = ops::AttachRequest {
        target: ops::Target {
            file: args.file.clone(),
            symbol: args.symbol.clone(),
            line: args.line,
        },
        kind: args.kind.clone(),
        claim: args.claim.clone(),
        detail: args.detail.clone(),
    };
    let provenance = ops::Provenance {
        assurance: ops::parse_assurance(Some(&args.assurance))?,
        evidence: args.evidence.iter().map(|item| parse_evidence(item)).collect(),
        revision: ops::head_revision(root),
    };
    Ok((ops::attach(root, scope, &request, &attribution, provenance)?, 0))
}

fn parse_target(raw: &str) -> ops::Target {
    match raw.rsplit_once(':') {
        Some((file, number)) if number.parse::<u32>().is_ok() => {
            ops::Target { file: file.to_owned(), symbol: None, line: number.parse().ok() }
        }
        _ => match raw.split_once('@') {
            Some((file, symbol)) => {
                ops::Target { file: file.to_owned(), symbol: Some(symbol.to_owned()), line: None }
            }
            None => ops::Target { file: raw.to_owned(), symbol: None, line: None },
        },
    }
}

fn command_relate(root: &Path, scope: Option<Scope>, args: &RelateArgs) -> Result<(Value, i32)> {
    let attribution = match args.author {
        AuthorKind::Agent => ops::Attribution::agent(&args.identity, "unrecorded"),
        AuthorKind::Analyzer => ops::Attribution::analyzer(&args.identity),
        AuthorKind::Runtime => ops::Attribution::runtime(&args.identity),
        AuthorKind::Human => ops::Attribution::human(&args.identity),
    };
    let request = ops::RelateRequest {
        subject: parse_target(&args.subject),
        verb: args.verb.clone(),
        object: parse_target(&args.object),
        claim: args.claim.clone(),
        detail: args.detail.clone(),
    };
    let provenance = ops::Provenance { revision: ops::head_revision(root), ..Default::default() };
    Ok((ops::relate(root, scope, &request, &attribution, provenance)?, 0))
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

fn command_context(
    root: &Path,
    target: &str,
    symbol: Option<&str>,
    depth: u8,
    budget: Option<usize>,
    as_of: Option<&str>,
) -> Result<(Value, i32)> {
    let (file, line) = match target.rsplit_once(':') {
        Some((path, number)) if number.parse::<u32>().is_ok() => {
            (path.to_owned(), number.parse::<u32>().ok())
        }
        _ => (target.to_owned(), None),
    };
    Ok((ops::context(root, &file, line, symbol, depth, budget, as_of)?, 0))
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

fn command_list(
    root: &Path,
    scope: Option<Scope>,
    file: Option<&str>,
    symbol: Option<&str>,
    as_of: Option<&str>,
    author: Option<&str>,
    limit: Option<usize>,
) -> Result<(Value, i32)> {
    Ok((ops::list(root, scope, file, symbol, as_of, author, limit)?, 0))
}

fn command_history(root: &Path, record: &str) -> Result<(Value, i32)> {
    Ok((ops::history(root, record)?, 0))
}

fn command_detached(root: &Path) -> Result<(Value, i32)> {
    Ok(ops::detached(root)?)
}

fn command_stats(root: &Path, scope: Option<Scope>) -> Result<(Value, i32)> {
    Ok((ops::stats(root, scope)?, 0))
}

#[cfg(test)]
mod tests {
    use super::parse_target;

    #[test]
    fn a_trailing_number_is_a_line() {
        let target = parse_target("src/lib.rs:12");
        assert_eq!(target.file, "src/lib.rs");
        assert_eq!(target.line, Some(12));
        assert_eq!(target.symbol, None);
    }

    #[test]
    fn a_symbol_path_is_not_read_as_a_line_number() {
        let target = parse_target("src/lib.rs@rust://compute");
        assert_eq!(
            target.file, "src/lib.rs",
            "the colon branch is tried first, and only the number parsing stops it from \
             splitting a symbol path at its own scheme separator"
        );
        assert_eq!(target.symbol.as_deref(), Some("rust://compute"));
        assert_eq!(target.line, None);
    }

    #[test]
    fn a_bare_path_is_about_the_file() {
        let target = parse_target("src/lib.rs");
        assert_eq!(target.file, "src/lib.rs");
        assert_eq!(target.symbol, None);
        assert_eq!(target.line, None);
    }

    #[test]
    fn a_colon_in_the_path_itself_survives() {
        let target = parse_target("src/a:b.rs@rust://compute");
        assert_eq!(target.file, "src/a:b.rs");
        assert_eq!(target.symbol.as_deref(), Some("rust://compute"));
    }

    #[test]
    fn a_line_wins_over_a_symbol_when_both_are_written() {
        let target = parse_target("src/lib.rs@rust://compute:12");
        assert_eq!(
            target.line,
            Some(12),
            "the two forms are not composable and the colon form is tried first, so \
             writing both silently drops the symbol into the file name"
        );
        assert_eq!(target.file, "src/lib.rs@rust://compute");
        assert_eq!(target.symbol, None);
    }
}
