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
        Some("build") => match args.next() {
            Some(path) => build_file(&path),
            None => {
                eprintln!("error: build requires a source file");
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
            println!("ardisa check [--json] <file>");
            println!("  Parse, type-check, and validate an Ardisa source file.");
            println!("ardisa build <file>");
            println!("  Lower Ardisa to readable Rust.");
            println!("ardisa fmt <file>");
            println!("  Print canonical Ardisa source.");
            ExitCode::SUCCESS
        }
        Some(command) => {
            eprintln!("error: unknown command '{command}'");
            ExitCode::from(2)
        }
    }
}

fn build_file(path: &str) -> ExitCode {
    require_ardisa_extension(path);
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}: error[AIF000]: {error}");
            return ExitCode::from(1);
        }
    };

    match ardisa_core::parse(&source) {
        Ok(module) => match ardisa_core::sema::check(&module) {
            Ok(()) => match ardisa_core::ownership::infer(&module) {
                Ok(()) => {
                    print!("{}", ardisa_core::lower::lower(&module).rust);
                    ExitCode::SUCCESS
                }
                Err(errors) => emit_diagnostics(path, &source, false, errors),
            },
            Err(errors) => emit_diagnostics(path, &source, false, errors),
        },
        Err(errors) => emit_diagnostics(path, &source, false, errors),
    }
}

fn format_file(path: &str) -> ExitCode {
    require_ardisa_extension(path);
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}: error[AIF000]: {error}");
            return ExitCode::from(1);
        }
    };

    match ardisa_core::parse(&source) {
        Ok(module) => {
            print!("{}", ardisa_core::format::format_module(&module));
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
    require_ardisa_extension(path);
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

    match ardisa_core::parse(&source) {
        Ok(module) => match ardisa_core::sema::check(&module) {
            Ok(()) => match ardisa_core::ownership::infer(&module) {
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
                Err(errors) => emit_diagnostics(path, &source, json, errors),
            },
            Err(errors) => emit_diagnostics(path, &source, json, errors),
        },
        Err(errors) => emit_diagnostics(path, &source, json, errors),
    }
}

fn emit_diagnostics(
    path: &str,
    source: &str,
    json: bool,
    errors: Vec<ardisa_core::source::Diagnostic>,
) -> ExitCode {
    for error in errors {
        if json {
            println!("{}", error.to_json(source));
        } else if let Some(location) = error.location(source) {
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

fn emit_error(
    path: &str,
    code: &'static str,
    message: &str,
    span: Option<ardisa_core::source::Span>,
    source: &str,
    json: bool,
) {
    let diagnostic = ardisa_core::source::Diagnostic::error(code, message, span);
    if json {
        println!("{}", diagnostic.to_json(source));
    } else {
        eprintln!("{path}: error[{code}]: {message}");
    }
}

fn require_ardisa_extension(path: &str) {
    if !path.ends_with(".ardisa") {
        eprintln!("{path}: error[AIF002]: Ardisa source files must use the .ardisa extension");
        std::process::exit(2);
    }
}
