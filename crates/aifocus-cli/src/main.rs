use std::{env, fs, process::ExitCode};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("check") => {
            let first = args.next();
            match first.as_deref() {
                Some("--json") => match args.next() {
                    Some(path) => check(&path, true),
                    None => {
                        eprintln!("error: check --json requires a source file");
                        ExitCode::from(2)
                    }
                },
                Some(path) => check(path, false),
                None => {
                    eprintln!("error: check requires a source file");
                    ExitCode::from(2)
                }
            }
        }
        Some("fmt") => match args.next() {
            Some(path) => format_file(&path),
            None => {
                eprintln!("error: fmt requires a source file");
                ExitCode::from(2)
            }
        },
        None | Some("help") | Some("--help") | Some("-h") => {
            println!("aifocus check [--json] <file>");
            println!("  Parse and validate an AIFocusLang source file.");
            println!("aifocus fmt <file>");
            println!("  Print canonical AIFocusLang source.");
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

fn check(path: &str, json: bool) -> ExitCode {
    let source = match fs::read_to_string(path) {
        Ok(source) if source.trim().is_empty() => {
            emit_error(path, "AIF001", "source file is empty", None, &source, json);
            return ExitCode::from(1);
        }
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}: error[AIF000]: {error}");
            return ExitCode::from(1);
        }
    };

    match aifocus_core::parse(&source) {
        Ok(module) => match aifocus_core::sema::check(&module) {
            Ok(()) => match aifocus_core::ownership::infer(&module) {
                Ok(()) => {
                if !json {
                    println!(
                        "{path}: ok (module {}, {} item(s))",
                        module.name,
                        module.items.len()
                    );
                }
                    ExitCode::SUCCESS
                }
                Err(errors) => {
                    for error in errors {
                    if json {
                        println!("{}", error.to_json(&source));
                    } else if let Some(location) = error.location(&source) {
                        eprintln!(
                            "{path}:{}:{}: error[{}]: {}",
                            location.line, location.column, error.code, error.message
                        );
                    } else {
                        eprintln!("{path}: error[{}]: {}", error.code, error.message);
                    }
                }
                    ExitCode::from(1)
                }
            },
            Err(errors) => {
                for error in errors {
        Err(errors) => {
            for error in errors {
                if json {
                    println!("{}", error.to_json(&source));
                } else {
                    if let Some(location) = error.location(&source) {
                        eprintln!(
                            "{path}:{}:{}: error[{}]: {}",
                            location.line, location.column, error.code, error.message
                        );
                    } else {
                        eprintln!("{path}: error[{}]: {}", error.code, error.message);
                    }
                }
            }
            ExitCode::from(1)
        }
    }
}

fn emit_error(
    path: &str,
    code: &'static str,
    message: &str,
    span: Option<aifocus_core::source::Span>,
    source: &str,
    json: bool,
) {
    let diagnostic = aifocus_core::source::Diagnostic::error(code, message, span);
    if json {
        println!("{}", diagnostic.to_json(source));
    } else {
        eprintln!("{path}: error[{code}]: {message}");
    }
}
