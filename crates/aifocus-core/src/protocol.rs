use std::time::Instant;

pub const PROTOCOL_VERSION: &str = "ardisa-compiler-protocol-v1";

use crate::{
    EffectModel, IrModule, Module, NodeId, OwnershipModel, concurrency, edit,
    edit::{NodeQuery, StructuralEdit},
    effects, ir,
    learning::PersistentCompilerLearning,
    ownership, sema,
    source::Diagnostic,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerTrace {
    pub parse_ns: u64,
    pub semantic_ns: u64,
    pub effects_ns: u64,
    pub ir_ns: u64,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRequirement {
    pub name: &'static str,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerSnapshot {
    pub protocol_version: &'static str,
    pub source: String,
    pub module: Module,
    pub trace: CompilerTrace,
    pub ir: IrModule,
    pub effects: EffectModel,
    pub ownership: OwnershipModel,
    pub concurrency_diagnostics: Vec<Diagnostic>,
    pub diagnostics: Vec<Diagnostic>,
    pub verification: Vec<VerificationRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerRequest {
    Inspect,
    QueryNode(NodeId),
    ApplyEdit(StructuralEdit),
    ApplyEdits(Vec<StructuralEdit>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerResponse {
    pub snapshot: CompilerSnapshot,
    pub changed_node: Option<NodeId>,
    pub changed_nodes: Vec<NodeId>,
    pub queried_node: Option<NodeQuery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidSource(Vec<Diagnostic>),
    Semantic(Vec<Diagnostic>),
    Ownership(Vec<Diagnostic>),
    Concurrency(Vec<Diagnostic>),
    Edit(String),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSource(errors) => write!(f, "source parse failed: {errors:?}"),
            Self::Semantic(errors) => write!(f, "semantic validation failed: {errors:?}"),
            Self::Ownership(errors) => write!(f, "ownership validation failed: {errors:?}"),
            Self::Concurrency(errors) => write!(f, "concurrency validation failed: {errors:?}"),
            Self::Edit(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ProtocolError {}

pub fn execute(source: &str, request: CompilerRequest) -> Result<CompilerResponse, ProtocolError> {
    let parse_start = Instant::now();
    let module = crate::parse(source).map_err(ProtocolError::InvalidSource)?;
    let parse_ns = parse_start.elapsed().as_nanos() as u64;
    match request {
        CompilerRequest::Inspect => {
            snapshot(source, module, parse_ns).map(|snapshot| CompilerResponse {
                snapshot,
                changed_node: None,
                changed_nodes: Vec::new(),
                queried_node: None,
            })
        }
        CompilerRequest::QueryNode(node) => {
            let queried_node = edit::query(&module, node)
                .map_err(|error| ProtocolError::Edit(error.to_string()))?;
            snapshot(source, module, parse_ns).map(|snapshot| CompilerResponse {
                snapshot,
                changed_node: None,
                changed_nodes: Vec::new(),
                queried_node: Some(queried_node),
            })
        }
        CompilerRequest::ApplyEdit(edit_request) => {
            let result = edit::apply(source, &module, edit_request)
                .map_err(|error| ProtocolError::Edit(error.to_string()))?;
            let changed_node = Some(result.replaced_node);
            snapshot(&result.source, result.module, parse_ns).map(|snapshot| CompilerResponse {
                snapshot,
                changed_node,
                changed_nodes: vec![changed_node.unwrap()],
                queried_node: None,
            })
        }
        CompilerRequest::ApplyEdits(edits) => {
            let result = edit::apply_transaction(source, &module, &edits)
                .map_err(|error| ProtocolError::Edit(error.to_string()))?;
            snapshot(&result.source, result.module, parse_ns).map(|snapshot| CompilerResponse {
                snapshot,
                changed_node: result.changed_nodes.first().copied(),
                changed_nodes: result.changed_nodes,
                queried_node: None,
            })
        }
    }
}

pub const WIRE_PROTOCOL_VERSION: &str = "ardisa-wire-v1";

/// Encode compiler requests into a dependency-free, deterministic wire format.
/// Source payloads are UTF-8 hex so framing is unambiguous even when source contains newlines.
pub fn encode_request(request: &CompilerRequest) -> String {
    let mut out = format!("{WIRE_PROTOCOL_VERSION}\n");
    match request {
        CompilerRequest::Inspect => out.push_str("INSPECT\n"),
        CompilerRequest::QueryNode(node) => {
            out.push_str(&format!("QUERY\n{}\n", node.0));
        }
        CompilerRequest::ApplyEdit(edit) => {
            out.push_str("EDIT\n");
            encode_edit(edit, &mut out);
        }
        CompilerRequest::ApplyEdits(edits) => {
            out.push_str(&format!("TRANSACTION\n{}\n", edits.len()));
            for edit in edits {
                encode_edit(edit, &mut out);
            }
        }
    }
    out
}

pub fn decode_request(wire: &str) -> Result<CompilerRequest, ProtocolError> {
    let mut lines = wire.split('\n');
    if lines.next() != Some(WIRE_PROTOCOL_VERSION) {
        return Err(ProtocolError::Edit(
            "unsupported wire protocol version".into(),
        ));
    }
    match lines.next() {
        Some("INSPECT") => Ok(CompilerRequest::Inspect),
        Some("QUERY") => {
            let node = lines
                .next()
                .ok_or_else(|| ProtocolError::Edit("query is missing a node id".into()))?
                .parse::<u64>()
                .map_err(|_| ProtocolError::Edit("query node id is not an integer".into()))?;
            Ok(CompilerRequest::QueryNode(NodeId(node)))
        }
        Some("EDIT") => decode_edit(&mut lines).map(CompilerRequest::ApplyEdit),
        Some("TRANSACTION") => {
            let count = lines
                .next()
                .ok_or_else(|| ProtocolError::Edit("transaction is missing its edit count".into()))?
                .parse::<usize>()
                .map_err(|_| {
                    ProtocolError::Edit("transaction edit count is not an integer".into())
                })?;
            let mut edits = Vec::with_capacity(count);
            for _ in 0..count {
                edits.push(decode_edit(&mut lines)?);
            }
            Ok(CompilerRequest::ApplyEdits(edits))
        }
        _ => Err(ProtocolError::Edit("unknown wire request".into())),
    }
}

fn encode_edit(edit: &StructuralEdit, out: &mut String) {
    match edit {
        StructuralEdit::Replace { node, source } => {
            out.push_str(&format!(
                "REPLACE\n{}\n{}\n",
                node.0,
                hex(source.as_bytes())
            ));
        }
        StructuralEdit::InsertBefore { node, source } => {
            out.push_str(&format!(
                "INSERT_BEFORE\n{}\n{}\n",
                node.0,
                hex(source.as_bytes())
            ));
        }
        StructuralEdit::Delete { node } => {
            out.push_str(&format!("DELETE\n{}\n", node.0));
        }
    }
}

fn decode_edit<'a>(lines: &mut std::str::Split<'a, char>) -> Result<StructuralEdit, ProtocolError> {
    let kind = lines
        .next()
        .ok_or_else(|| ProtocolError::Edit("edit kind is missing".into()))?;
    let node = lines
        .next()
        .ok_or_else(|| ProtocolError::Edit("edit node id is missing".into()))?
        .parse::<u64>()
        .map_err(|_| ProtocolError::Edit("edit node id is not an integer".into()))?;
    match kind {
        "DELETE" => Ok(StructuralEdit::Delete { node: NodeId(node) }),
        "REPLACE" | "INSERT_BEFORE" => {
            let encoded = lines
                .next()
                .ok_or_else(|| ProtocolError::Edit("edit source payload is missing".into()))?;
            let bytes = decode_hex(encoded)?;
            let source = String::from_utf8(bytes)
                .map_err(|_| ProtocolError::Edit("edit source is not valid UTF-8".into()))?;
            if kind == "REPLACE" {
                Ok(StructuralEdit::Replace {
                    node: NodeId(node),
                    source,
                })
            } else {
                Ok(StructuralEdit::InsertBefore {
                    node: NodeId(node),
                    source,
                })
            }
        }
        _ => Err(ProtocolError::Edit("unknown structural edit kind".into())),
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ProtocolError> {
    if value.len() % 2 != 0 {
        return Err(ProtocolError::Edit("hex payload has odd length".into()));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let chars = value.as_bytes();
    for pair in chars.chunks_exact(2) {
        let high = hex_digit(pair[0])
            .ok_or_else(|| ProtocolError::Edit("hex payload contains an invalid digit".into()))?;
        let low = hex_digit(pair[1])
            .ok_or_else(|| ProtocolError::Edit("hex payload contains an invalid digit".into()))?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

pub struct CompilerSession {
    pub project: String,
    pub task_kind: String,
    pub learning: PersistentCompilerLearning,
}

impl CompilerSession {
    pub fn new(project: impl Into<String>, task_kind: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            task_kind: task_kind.into(),
            learning: PersistentCompilerLearning::default(),
        }
    }

    pub fn execute(
        &mut self,
        source: &str,
        request: CompilerRequest,
    ) -> Result<CompilerResponse, ProtocolError> {
        let result = execute(source, request);
        if let Err(error) = &result {
            let diagnostics: &[Diagnostic] = match error {
                ProtocolError::InvalidSource(items)
                | ProtocolError::Semantic(items)
                | ProtocolError::Ownership(items)
                | ProtocolError::Concurrency(items) => items.as_slice(),
                ProtocolError::Edit(_) => &[],
            };
            for diagnostic in diagnostics {
                self.learning
                    .record(&self.project, &self.task_kind, diagnostic);
            }
        }
        result
    }
}

fn snapshot(
    source: &str,
    module: Module,
    parse_ns: u64,
) -> Result<CompilerSnapshot, ProtocolError> {
    let semantic_start = Instant::now();
    sema::check(&module).map_err(ProtocolError::Semantic)?;
    let semantic_ns = semantic_start.elapsed().as_nanos() as u64;

    let ownership = ownership::analyze(&module).map_err(ProtocolError::Ownership)?;

    let effects_start = Instant::now();
    let effects = effects::analyze(&module);
    let effects_ns = effects_start.elapsed().as_nanos() as u64;

    let concurrency_diagnostics = concurrency::analyze_with_diagnostics(&module)
        .map(|_| Vec::new())
        .map_err(ProtocolError::Concurrency)?;

    let ir_start = Instant::now();
    let ir = ir::lower(&module);
    let ir_ns = ir_start.elapsed().as_nanos() as u64;

    Ok(CompilerSnapshot {
        protocol_version: PROTOCOL_VERSION,
        source: source.into(),
        module,
        trace: CompilerTrace {
            parse_ns,
            semantic_ns,
            effects_ns,
            ir_ns,
            diagnostics: 0,
        },
        ir,
        effects,
        ownership,
        concurrency_diagnostics,
        diagnostics: Vec::new(),
        verification: verification_requirements(),
    })
}

fn verification_requirements() -> Vec<VerificationRequirement> {
    vec![
        VerificationRequirement {
            name: "parse",
            required: true,
        },
        VerificationRequirement {
            name: "semantic",
            required: true,
        },
        VerificationRequirement {
            name: "ownership",
            required: true,
        },
        VerificationRequirement {
            name: "effects",
            required: true,
        },
        VerificationRequirement {
            name: "ir-lowering",
            required: true,
        },
    ]
}

pub const MAX_STDIO_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// Encode one protocol payload using Content-Length framing.
pub fn encode_stdio_frame(payload: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    if payload.len() > MAX_STDIO_FRAME_BYTES {
        return Err(ProtocolError::Edit(
            "stdio payload exceeds 8 MiB limit".into(),
        ));
    }
    let mut frame = format!("Content-Length: {}\r\n\r\n", payload.len()).into_bytes();
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// Decode one complete Content-Length frame and return trailing bytes.
pub fn decode_stdio_frame(input: &[u8]) -> Result<(Vec<u8>, &[u8]), ProtocolError> {
    const HEADER_END: &[u8] = b"\r\n\r\n";
    let header_end = match input
        .windows(HEADER_END.len())
        .position(|window| window == HEADER_END)
    {
        Some(position) => position,
        None => return Err(ProtocolError::Edit("incomplete stdio header".into())),
    };
    let header = &input[..header_end];
    let prefix = b"Content-Length: ";
    if !header.starts_with(prefix) {
        return Err(ProtocolError::Edit("invalid stdio header".into()));
    }
    let length_text = &header[prefix.len()..];
    if length_text.is_empty() || !length_text.iter().all(u8::is_ascii_digit) {
        return Err(ProtocolError::Edit("invalid Content-Length".into()));
    }
    let length = std::str::from_utf8(length_text)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| ProtocolError::Edit("invalid Content-Length".into()))?;
    if length > MAX_STDIO_FRAME_BYTES {
        return Err(ProtocolError::Edit("stdio payload exceeds 8 MiB limit".into()));
    }
    let payload_start = header_end + HEADER_END.len();
    let payload_end = payload_start
        .checked_add(length)
        .ok_or_else(|| ProtocolError::Edit("stdio frame length overflow".into()))?;
    if input.len() < payload_end {
        return Err(ProtocolError::Edit("incomplete stdio payload".into()));
    }
    Ok((
        input[payload_start..payload_end].to_vec(),
        &input[payload_end..],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Item, StmtKind};

    #[test]
    fn stdio_frames_round_trip_and_support_multiple_messages() {
        let first = encode_stdio_frame(b"ardisa-one").unwrap();
        let second = encode_stdio_frame(b"ardisa-two").unwrap();
        let mut stream = first;
        stream.extend_from_slice(&second);
        let (payload, rest) = decode_stdio_frame(&stream).unwrap();
        assert_eq!(payload, b"ardisa-one");
        let (payload, rest) = decode_stdio_frame(rest).unwrap();
        assert_eq!(payload, b"ardisa-two");
        assert!(rest.is_empty());
    }

    #[test]
    fn stdio_frames_reject_oversized_payloads() {
        let payload = vec![0u8; MAX_STDIO_FRAME_BYTES + 1];
        assert!(encode_stdio_frame(&payload).is_err());
    }

    #[test]
    fn wire_requests_round_trip_without_ambiguous_source_framing() {
        let request = CompilerRequest::ApplyEdits(vec![
            StructuralEdit::Replace {
                node: NodeId(7),
                source: "a + b\nif a\n  b\n".into(),
            },
            StructuralEdit::Delete { node: NodeId(8) },
        ]);
        let wire = encode_request(&request);
        assert_eq!(decode_request(&wire).unwrap(), request);
        assert!(wire.starts_with("ardisa-wire-v1\nTRANSACTION\n2\n"));
    }

    #[test]
    fn inspect_exposes_ir_effects_and_ownership() {
        let source = "module x\nfn main(a: Int) -> Int\n  a + 1\n";
        let response = execute(source, CompilerRequest::Inspect).unwrap();
        assert_eq!(response.snapshot.protocol_version, PROTOCOL_VERSION);
        assert_eq!(response.snapshot.module.name, "x");
        assert_eq!(response.snapshot.ir.functions.len(), 1);
        assert!(response.snapshot.effects.functions.contains_key("main"));
        assert!(!response.snapshot.ownership.accesses.is_empty());
        assert!(response.snapshot.diagnostics.is_empty());
        assert!(response.snapshot.concurrency_diagnostics.is_empty());
        assert!(
            response
                .snapshot
                .verification
                .iter()
                .all(|requirement| requirement.required)
        );
        assert!(response.snapshot.trace.parse_ns > 0 || response.snapshot.trace.semantic_ns > 0);
    }

    #[test]
    fn apply_edit_returns_new_validated_snapshot() {
        let source = "module x\nfn main() -> Int\n  1\n";
        let module = crate::parse(source).unwrap();
        let Item::Function(function) = &module.items[0];
        let StmtKind::Expr(expr) = &function.body.stmts[0].kind else {
            panic!("expected expression");
        };
        let response = execute(
            source,
            CompilerRequest::ApplyEdit(StructuralEdit::Replace {
                node: expr.id,
                source: "2".into(),
            }),
        )
        .unwrap();
        assert_eq!(response.changed_node, Some(expr.id));
        assert!(response.snapshot.source.contains("2"));
    }

    #[test]
    fn invalid_edit_is_rejected_before_snapshot() {
        let source = "module x\nfn main() -> Int\n  1\n";
        let result = execute(
            source,
            CompilerRequest::ApplyEdit(StructuralEdit::Replace {
                node: NodeId(99),
                source: "2".into(),
            }),
        );
        assert!(matches!(result, Err(ProtocolError::Edit(_))));
    }

    #[test]
    fn session_records_contextual_learning_on_failures() {
        let source = "module x\nfn main() -> Int\n  missing\n";
        let mut session = CompilerSession::new("project-a", "compiler-edit");
        assert!(session.execute(source, CompilerRequest::Inspect).is_err());
        let key = crate::learning::LearningKey {
            project: "project-a".into(),
            task_kind: "compiler-edit".into(),
            diagnostic: "AIF304".into(),
        };
        assert!(session.learning.recurring(&key, 1));
    }

    #[test]
    fn protocol_does_not_own_diagnostic_memory_state() {
        let memory = crate::DiagnosticMemory::default();
        assert!(memory.history("AIF304").is_none());
    }
}
