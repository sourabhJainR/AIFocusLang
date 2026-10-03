use std::{env, fs, process::ExitCode};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("check") => match args.next() {
            Some(path) => check(&path),
            None => {
                eprintln!("error: check requires a source file");
                ExitCode::from(2)
            }
        },
        None | Some("help") | Some("--help") | Some("-h") => {
            println!("aifocus check <file>");
            println!("  Validate that an AIFocusLang source file can be read.");
            ExitCode::SUCCESS
        }
        Some(command) => {
            eprintln!("error: unknown command '{command}'");
            ExitCode::from(2)
        }
    }
}

fn check(path: &str) -> ExitCode {
    match fs::read_to_string(path) {
        Ok(source) if source.trim().is_empty() => {
            eprintln!("{path}: error[AIF001]: source file is empty");
            ExitCode::from(1)
        }
        Ok(_) => {
            println!("{path}: ok");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{path}: error[AIF000]: {error}");
            ExitCode::from(1)
        }
    }
}
