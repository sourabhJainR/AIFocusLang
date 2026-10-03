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
        Some("fmt") => match args.next() {
            Some(path) => format_file(&path),
            None => {
                eprintln!("error: fmt requires a source file");
                ExitCode::from(2)
            }
        },
        None | Some("help") | Some("--help") | Some("-h") => {
            println!("aifocus check <file>");
            println!("  Parse and validate an AIFocusLang source file.");
            ExitCode::SUCCESS
        }
        Some(command) => {
            eprintln!("error: unknown command '{command}'");
            ExitCode::from(2)
        }
    }
}

fn format_file(path: &str) -> ExitCode {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}: error[AIF000]: {error}");
            return ExitCode::from(1);
        }
    };

    match aifocus_core::parse(&source) {
        Ok(module) => {
            print!("{}", aifocus_core::format::format_module(&module));
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("{path}: error[{}]: {}", error.code, error.message);
            }
            ExitCode::from(1)
        }
    }
}

fn check(path: &str) -> ExitCode {
    let source = match fs::read_to_string(path) {
        Ok(source) if source.trim().is_empty() => {
            eprintln!("{path}: error[AIF001]: source file is empty");
            return ExitCode::from(1);
        }
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}: error[AIF000]: {error}");
            return ExitCode::from(1);
        }
    };

    match aifocus_core::parse(&source) {
        Ok(module) => {
            println!(
                "{path}: ok (module {}, {} item(s))",
                module.name,
                module.items.len()
            );
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("{path}: error[{}]: {}", error.code, error.message);
            }
            ExitCode::from(1)
        }
    }
}
