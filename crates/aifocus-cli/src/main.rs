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
        Some("run") => match args.next() {
            Some(path) => run_file(&path, args.collect()),
            None => {
                eprintln!("error: run requires a source file");
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
            println!("ardisa run <file> [args...]");
            println!("  Compile and execute the module natively without Rust.");
            println!("  The entry function is 'main'; arguments are typed from its signature.");
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

fn run_file(path: &str, raw_args: Vec<String>) -> ExitCode {
    require_ardisa_extension(path);
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}: error[AIF000]: {error}");
            return ExitCode::from(1);
        }
    };

    let module = match ardisa_core::parse(&source) {
        Ok(module) => module,
        Err(errors) => return emit_diagnostics(path, &source, false, errors),
    };
    if let Err(errors) = ardisa_core::sema::check(&module) {
        return emit_diagnostics(path, &source, false, errors);
    }
    if let Err(errors) = ardisa_core::ownership::infer(&module) {
        return emit_diagnostics(path, &source, false, errors);
    }

    let Some(main) = module.items.iter().find_map(|item| match item {
        ardisa_core::Item::Function(function) if function.name == "main" => Some(function),
        _ => None,
    }) else {
        eprintln!("{path}: error[AIF600]: entry function 'main' was not found");
        return ExitCode::from(1);
    };

    let mut values = Vec::with_capacity(main.params.len());
    if raw_args.len() != main.params.len() {
        eprintln!(
            "{path}: error[AIF601]: main expects {} argument(s), got {}",
            main.params.len(),
            raw_args.len()
        );
        return ExitCode::from(1);
    }

    for (raw, parameter) in raw_args.iter().zip(&main.params) {
        match parse_value(raw, &parameter.ty.kind) {
            Ok(value) => values.push(value),
            Err(message) => {
                eprintln!(
                    "{path}: error[AIF602]: argument '{}' for '{}' {}",
                    raw, parameter.name, message
                );
                return ExitCode::from(1);
            }
        }
    }

    let ir = ardisa_core::ir::lower(&module);
    let program = match ardisa_core::native::compile_program(&ir) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("{path}: error[AIF603]: native compilation failed: {error:?}");
            return ExitCode::from(1);
        }
    };
    match ardisa_core::native::run_program(&program, "main", &values) {
        Ok(value) => {
            println!("{}", display_value(&value));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{path}: error[AIF604]: native execution failed: {error:?}");
            ExitCode::from(1)
        }
    }
}

fn parse_value(
    raw: &str,
    ty: &ardisa_core::TypeKind,
) -> Result<ardisa_core::NativeValue, &'static str> {
    match ty {
        ardisa_core::TypeKind::Int => raw
            .parse::<i64>()
            .map(ardisa_core::NativeValue::Int)
            .map_err(|_| "must be an Int"),
        ardisa_core::TypeKind::Bool => match raw {
            "true" => Ok(ardisa_core::NativeValue::Bool(true)),
            "false" => Ok(ardisa_core::NativeValue::Bool(false)),
            _ => Err("must be true or false"),
        },
        ardisa_core::TypeKind::String => Ok(ardisa_core::NativeValue::String(raw.to_owned())),
        _ => Err("has a type not supported by the native CLI yet"),
    }
}

fn display_value(value: &ardisa_core::NativeValue) -> String {
    match value {
        ardisa_core::NativeValue::Int(value) => value.to_string(),
        ardisa_core::NativeValue::Bool(value) => value.to_string(),
        ardisa_core::NativeValue::String(value) => value.clone(),
        ardisa_core::NativeValue::List(values) => {
            let rendered = values.iter().map(display_value).collect::<Vec<_>>();
            format!("[{}]", rendered.join(", "))
        }
        ardisa_core::NativeValue::ResultOk(value) => format!("Ok({})", display_value(value)),
        ardisa_core::NativeValue::ResultErr(value) => format!("Err({})", display_value(value)),
        ardisa_core::NativeValue::Unit => "()".into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typed_cli_arguments() {
        assert_eq!(
            parse_value("42", &ardisa_core::TypeKind::Int).unwrap(),
            ardisa_core::NativeValue::Int(42)
        );
        assert_eq!(
            parse_value("true", &ardisa_core::TypeKind::Bool).unwrap(),
            ardisa_core::NativeValue::Bool(true)
        );
        assert!(parse_value("wat", &ardisa_core::TypeKind::Bool).is_err());
    }

    #[test]
    fn renders_native_values_deterministically() {
        let value =
            ardisa_core::NativeValue::ResultOk(Box::new(ardisa_core::NativeValue::List(vec![
                ardisa_core::NativeValue::Int(1),
                ardisa_core::NativeValue::Int(2),
            ])));
        assert_eq!(display_value(&value), "Ok([1, 2])");
    }
}

fn require_ardisa_extension(path: &str) {
    if !path.ends_with(".ardisa") {
        eprintln!("{path}: error[AIF002]: Ardisa source files must use the .ardisa extension");
        std::process::exit(2);
    }
}
