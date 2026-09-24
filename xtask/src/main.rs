#![forbid(unsafe_code)]

mod allows;
mod comments;
mod docs;
mod invocations;
mod prose;
mod replay;
mod resolver_vectors;
mod vectors;

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let rest: Vec<String> = std::env::args().skip(2).collect();
    match task.as_str() {
        "lint-allows" => allows::run(),
        "lint-invocations" => invocations::run(),
        "lint-comments" => comments::run(),
        "lint-docs" => docs::run(),
        "lint-prose" => prose::run(),
        "replay" => replay::run(&rest),
        "vectors" => vectors::run(),
        "check" => {
            let outcomes = [comments::run(), docs::run(), prose::run(), replay::run(&rest)];
            outcomes
                .into_iter()
                .find(|outcome| *outcome != ExitCode::SUCCESS)
                .unwrap_or(ExitCode::SUCCESS)
        }
        other => {
            eprintln!("unknown task {other:?}");
            eprintln!("tasks: lint-comments, lint-docs, lint-prose, replay, vectors, check");
            ExitCode::from(2)
        }
    }
}
