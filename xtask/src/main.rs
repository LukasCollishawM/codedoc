#![forbid(unsafe_code)]

mod comments;
mod replay;
mod vectors;

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let rest: Vec<String> = std::env::args().skip(2).collect();
    match task.as_str() {
        "lint-comments" => comments::run(),
        "replay" => replay::run(&rest),
        "vectors" => vectors::run(),
        "check" => {
            let first = comments::run();
            let second = replay::run(&rest);
            if first == ExitCode::SUCCESS { second } else { first }
        }
        other => {
            eprintln!("unknown task {other:?}");
            eprintln!("tasks: lint-comments, replay, vectors, check");
            ExitCode::from(2)
        }
    }
}
