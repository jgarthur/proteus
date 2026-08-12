use std::path::PathBuf;
use std::process::ExitCode;

use proteus::runner::{current_build_info, run_main, ErrorKind, RunOptions};

fn main() -> ExitCode {
    match dispatch(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err((kind, message)) => {
            eprintln!("proteus-run: {message}");
            ExitCode::from(match kind {
                ErrorKind::InvalidInput => 2,
                ErrorKind::Operational => 1,
            })
        }
    }
}

fn dispatch(arguments: Vec<String>) -> Result<(), (ErrorKind, String)> {
    if arguments.iter().any(|argument| argument == "--build-info") {
        if arguments != ["--build-info"] {
            return Err((
                ErrorKind::InvalidInput,
                "--build-info cannot be combined with other arguments".to_owned(),
            ));
        }
        let info = current_build_info().map_err(|error| (error.kind(), error.to_string()))?;
        serde_json::to_writer(std::io::stdout(), &info)
            .map_err(|error| (ErrorKind::Operational, error.to_string()))?;
        println!();
        return Ok(());
    }

    let mut manifest_path = None;
    let mut threads = 1_u32;
    let mut threads_seen = false;
    let mut supervised = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--manifest" => {
                index += 1;
                if manifest_path.is_some() || index >= arguments.len() {
                    return Err((
                        ErrorKind::InvalidInput,
                        "usage: proteus-run --manifest <path> [--threads <N>]".to_owned(),
                    ));
                }
                manifest_path = Some(PathBuf::from(&arguments[index]));
            }
            "--threads" => {
                index += 1;
                if threads_seen || index >= arguments.len() {
                    return Err((
                        ErrorKind::InvalidInput,
                        "--threads requires one positive integer".to_owned(),
                    ));
                }
                threads_seen = true;
                threads = arguments[index].parse().map_err(|_| {
                    (
                        ErrorKind::InvalidInput,
                        "--threads requires one positive integer".to_owned(),
                    )
                })?;
            }
            "--internal-supervised" => {
                if supervised {
                    return Err((
                        ErrorKind::InvalidInput,
                        "--internal-supervised may be specified only once".to_owned(),
                    ));
                }
                supervised = true;
            }
            other => {
                return Err((
                    ErrorKind::InvalidInput,
                    format!("unknown argument {other:?}"),
                ));
            }
        }
        index += 1;
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        (
            ErrorKind::InvalidInput,
            "usage: proteus-run --manifest <path> [--threads <N>]".to_owned(),
        )
    })?;
    run_main(RunOptions {
        manifest_path,
        threads,
        supervised,
    })
    .map_err(|error| (error.kind(), error.to_string()))
}
