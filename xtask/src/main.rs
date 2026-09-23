#![forbid(unsafe_code)]

mod comments;
mod docs;
mod replay;
mod resolver_vectors;
mod vectors;

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let rest: Vec<String> = std::env::args().skip(2).collect();
    match task.as_str() {
        "lint-comments" => comments::run(),
        "lint-docs" => docs::run(),
        "replay" => replay::run(&rest),
        "vectors" => vectors::run(),
        "check" => {
            let outcomes = [comments::run(), docs::run(), replay::run(&rest)];
            outcomes
                .into_iter()
                .find(|outcome| *outcome != ExitCode::SUCCESS)
                .unwrap_or(ExitCode::SUCCESS)
        }
        other => {
            eprintln!("unknown task {other:?}");
            eprintln!("tasks: lint-comments, lint-docs, replay, vectors, check");
            ExitCode::from(2)
        }
    }
}
