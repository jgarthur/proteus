use std::path::PathBuf;
use std::process::ExitCode;

use proteus::runner::{batch_main, BatchOptions, BatchOutcome, ErrorKind};

fn main() -> ExitCode {
    match parse_options(std::env::args().skip(1).collect()) {
        Ok(options) => match batch_main(options) {
            Ok(BatchOutcome::Success) => ExitCode::SUCCESS,
            Ok(BatchOutcome::Failed) => ExitCode::from(1),
            Ok(BatchOutcome::Interrupted) => ExitCode::from(130),
            Err(error) => {
                eprintln!("proteus-batch: {error}");
                ExitCode::from(match error.kind() {
                    ErrorKind::InvalidInput => 2,
                    ErrorKind::Operational => 1,
                })
            }
        },
        Err(message) => {
            eprintln!("proteus-batch: {message}");
            ExitCode::from(2)
        }
    }
}

fn parse_options(arguments: Vec<String>) -> Result<BatchOptions, String> {
    let mut manifest_path = None;
    let mut retry_incomplete = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--manifest" => {
                index += 1;
                if manifest_path.is_some() || index >= arguments.len() {
                    return Err(
                        "usage: proteus-batch --manifest <path> [--retry-incomplete]".to_owned(),
                    );
                }
                manifest_path = Some(PathBuf::from(&arguments[index]));
            }
            "--retry-incomplete" => {
                if retry_incomplete {
                    return Err("--retry-incomplete may be specified only once".to_owned());
                }
                retry_incomplete = true;
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
        index += 1;
    }
    let manifest_path = manifest_path
        .ok_or_else(|| "usage: proteus-batch --manifest <path> [--retry-incomplete]".to_owned())?;
    Ok(BatchOptions {
        manifest_path,
        retry_incomplete,
    })
}
