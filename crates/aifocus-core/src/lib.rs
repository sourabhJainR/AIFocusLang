//! Core language model, parser, and AI-native structural editing API for Ardisa.

pub mod ai;
pub mod ai_types;
pub mod arena;
pub mod ast;
pub mod benchmark;
pub mod bootstrap;
pub mod concurrency;
pub mod compiler_engine;
pub mod diagnostic_memory;
pub mod differential;
pub mod edit;
pub mod effects;
pub mod engineering_feedback;
pub mod evidence;
pub mod format;
pub mod fuzz;
pub mod interop;
pub mod ir;
pub mod learning;
pub mod lower;
pub mod module_graph;
pub mod native;
pub mod ownership;
pub mod optimizer;
pub mod pipeline;
pub mod typed_ir;
pub mod protocol;
pub mod sema;
pub mod source;
pub mod token;

pub use ai_types::{Alternative, Guaranteed, GraphConflictPolicy, GraphTensor, GraphTensorEdge, GraphTensorNode, Probabilistic};
pub use ai::{
    Distribution, Embedding, Modality, Probability, Quantization, RuntimeError, SemanticValue,
    Tensor, TraceBuffer, TraceEvent, TraceLevel,
};
pub use arena::{Arena, ArenaId, RecordField, RecordSchema, RecordValue};
pub use compiler_engine::{AstArena,BackendReport,CompilerEvidence,SafetyGraph,SymbolTables,TokenArena};
pub use concurrency::{
    CancellationToken, ScopeEvent, ScopeReport, StructuredScope, TaskHandle, TaskSpec, TaskTerminal,
};
pub use diagnostic_memory::{DiagnosticHistoryEntry, DiagnosticMemory};
pub use differential::{CORPUS as DIFFERENTIAL_CORPUS, DifferentialCase};
pub use effects::{EffectKind, EffectModel, FunctionEffects};
pub use engineering_feedback::{CapabilityFeedback, EpisodeOutcome, VerificationDepth};
pub use evidence::{CapabilityDecision, CapabilityEvaluation, EvidenceEnvelope, EvidenceGraph};
pub use fuzz::{GeneratedCase, generate as generate_fuzz_case};
pub use interop::{
    AbiParameterContract, InteropType, OwnershipContract, RustFunction, SafeRustBoundary,
};
pub use ir::{IrFunction, IrModule, IrOp, IrValue};
pub use optimizer::optimize;
pub use pipeline::{compile_source, CompiledArtifact, PipelineError, PipelineEvidence, PipelineTiming};
pub use typed_ir::{TypedBasicBlock, TypedIrFunction, TypedIrModule, TypedIrOp, TypedTerminator, TypedValue, TypedValueKind};
pub use learning::{
    LearningEntry, LearningKey, LearningProvenance, LearningStatus, PersistentCompilerLearning,
    ProvenanceKind,
};
pub use module_graph::{ModuleGraph, ModuleGraphError, ModuleNode};
pub use native::{NativeError, NativeFunction, NativeInstr, NativeProgram, NativeValue};
pub use ownership::{AccessKind, BorrowKind, OwnershipClass, OwnershipModel, OwnershipTransition};
pub use protocol::{
    CompilerRequest, CompilerResponse, CompilerSession, CompilerSnapshot, CompilerTrace,
    PROTOCOL_VERSION, ProtocolError, VerificationRequirement,
};

pub use ast::{
    BinaryOp, Block, Expr, ExprKind, Function, Item, Module, NodeId, Parameter, Stmt, StmtKind,
    Type, TypeKind,
};
pub use token::{Token, TokenKind, lex};

use std::collections::HashMap;

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<source::Diagnostic>,
    occurrences: HashMap<String, u64>,
}

pub fn parse(input: &str) -> Result<Module, Vec<source::Diagnostic>> {
    let tokens = token::lex(input)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
        errors: Vec::new(),
        occurrences: HashMap::new(),
    };
    let module = parser.parse_module();

    if parser.errors.is_empty() {
        module.ok_or_else(|| {
            vec![source::Diagnostic::error(
                "AIF199",
                "parser produced no module",
                None,
            )]
        })
    } else {
        Err(parser.errors)
    }
}

impl Parser {
    fn parse_module(&mut self) -> Option<Module> {
        self.skip_newlines();
        let start = self.current().span.start;
        self.expect(TokenKind::Module, "module declaration")?;
        let name_token = self.expect(TokenKind::Ident, "module name")?;
        let name = name_token.lexeme.clone();
        self.expect(TokenKind::Newline, "end of module declaration")?;

        let mut items = Vec::new();
        while !self.at(TokenKind::Eof) {
            self.skip_newlines();
            if self.at(TokenKind::Eof) {
                break;
            }
            if let Some(item) = self.parse_item() {
                items.push(item);
            } else {
                self.recover_top_level();
            }
        }

        Some(Module {
            id: self.id("module", &name),
            span: source::Span::new(start, self.previous_span().end),
            name,
            items,
        })
    }

    fn parse_item(&mut self) -> Option<Item> {
        if self.at(TokenKind::Fn) {
            self.parse_function().map(Item::Function)
        } else {
            self.error("AIF201", "expected a top-level function");
            None
        }
    }

    fn parse_function(&mut self) -> Option<Function> {
        let start = self.bump().span.start;
        let name_token = self.expect(TokenKind::Ident, "function name")?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::LParen, "'(' after function name")?;
        let mut params = Vec::new();
        if !self.at(TokenKind::RParen) {
            loop {
                let param_start = self.current().span.start;
                let param_name = self.expect(TokenKind::Ident, "parameter name")?;
                self.expect(TokenKind::Colon, "':' after parameter name")?;
                let ty = self.parse_type()?;
                let param_end = ty.span.end;
                let param_name_text = param_name.lexeme.clone();
                params.push(Parameter {
                    id: self.id("param", &param_name_text),
                    span: source::Span::new(param_start, param_end),
                    name: param_name_text,
                    ty,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen, "')' after parameters")?;

        let return_type = if self.eat(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(TokenKind::Newline, "end of function declaration")?;
        let body = self.parse_block("function body")?;

        Some(Function {
            id: self.id("fn", &name),
            span: source::Span::new(start, body.span.end),
            name,
            params,
            return_type,
            body,
        })
    }

    fn parse_type(&mut self) -> Option<Type> {
        let start = self.current().span.start;
        if self.eat(TokenKind::LParen) {
            let close = self.expect(TokenKind::RParen, "')' in unit type")?;
            return Some(Type {
                id: self.id("type", "()"),
                span: source::Span::new(start, close.span.end),
                kind: TypeKind::Unit,
            });
        }

        let token = self.expect(TokenKind::Ident, "type name")?;
        let name = token.lexeme.clone();
        let kind = match name.as_str() {
            "Int" => TypeKind::Int,
            "Bool" => TypeKind::Bool,
            "String" => TypeKind::String,
            "List" if self.eat(TokenKind::LAngle) => {
                let element = self.parse_type()?;
                self.expect(TokenKind::RAngle, "'>' in List type")?;
                TypeKind::List(Box::new(element))
            }
            "Result" if self.eat(TokenKind::LAngle) => {
                let ok = self.parse_type()?;
                self.expect(TokenKind::Comma, "',' in Result type")?;
                let err = self.parse_type()?;
                self.expect(TokenKind::RAngle, "'>' in Result type")?;
                TypeKind::Result(Box::new(ok), Box::new(err))
            }
            _ => TypeKind::Named(name.clone()),
        };
        Some(Type {
            id: self.id("type", &name),
            span: source::Span::new(start, self.previous_span().end),
            kind,
        })
    }

    fn parse_block(&mut self, context: &str) -> Option<Block> {
        let start = self.current().span.start;
        if !matches!(self.current().kind, TokenKind::Indent(_)) {
            self.error("AIF202", format!("expected indented {context}"));
            return None;
        }
        self.bump();

        let mut stmts = Vec::new();
        self.skip_newlines();
        while !self.at(TokenKind::Dedent) && !self.at(TokenKind::Eof) {
            if let Some(stmt) = self.parse_stmt() {
                stmts.push(stmt);
            } else {
                self.recover_statement();
            }
            self.skip_newlines();
        }

        let end = if self.eat(TokenKind::Dedent) {
            self.previous_span().end
        } else {
            self.current().span.end
        };
        Some(Block {
            id: self.id("block", &start.to_string()),
            span: source::Span::new(start, end),
            stmts,
        })
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        let start = self.current().span.start;

        if self.eat(TokenKind::Let) {
            let name = self.expect(TokenKind::Ident, "binding name")?;
            self.expect(TokenKind::Equal, "'=' in let binding")?;
            let value = self.parse_expr(0)?;
            let end = value.span.end;
            self.expect(TokenKind::Newline, "end of let statement")?;
            return Some(Stmt {
                id: self.id("let", &name.lexeme),
                span: source::Span::new(start, end),
                kind: StmtKind::Let {
                    name: name.lexeme,
                    value,
                },
            });
        }

        if self.eat(TokenKind::Set) {
            let name = self.expect(TokenKind::Ident, "binding name")?;
            if self.eat(TokenKind::LBracket) {
                let index = self.parse_expr(0)?;
                self.expect(TokenKind::RBracket, "']' after assignment index")?;
                self.expect(TokenKind::Equal, "'=' in indexed set statement")?;
                let value = self.parse_expr(0)?;
                let end = value.span.end;
                self.expect(TokenKind::Newline, "end of set statement")?;
                let collection = Expr {
                    id: self.id("name", &name.lexeme),
                    span: name.span,
                    kind: ExprKind::Name(name.lexeme.clone()),
                };
                return Some(Stmt {
                    id: self.id("set-index", &name.lexeme),
                    span: source::Span::new(start, end),
                    kind: StmtKind::SetIndex {
                        collection,
                        index,
                        value,
                    },
                });
            }
            self.expect(TokenKind::Equal, "'=' in set statement")?;
            let value = self.parse_expr(0)?;
            let end = value.span.end;
            self.expect(TokenKind::Newline, "end of set statement")?;
            return Some(Stmt {
                id: self.id("set", &name.lexeme),
                span: source::Span::new(start, end),
                kind: StmtKind::Set {
                    name: name.lexeme,
                    value,
                },
            });
        }

        if self.eat(TokenKind::Return) {
            let value = if self.at(TokenKind::Newline) {
                None
            } else {
                Some(self.parse_expr(0)?)
            };
            let end = value
                .as_ref()
                .map_or(self.previous_span().end, |expr| expr.span.end);
            self.expect(TokenKind::Newline, "end of return statement")?;
            return Some(Stmt {
                id: self.id("return", &start.to_string()),
                span: source::Span::new(start, end),
                kind: StmtKind::Return(value),
            });
        }

        if self.eat(TokenKind::Scope) {
            self.expect(TokenKind::Newline, "end of scope declaration")?;
            let body = self.parse_block("scope body")?;
            let end = body.span.end;
            return Some(Stmt {
                id: self.id("scope", &start.to_string()),
                span: source::Span::new(start, end),
                kind: StmtKind::Scope { body },
            });
        }

        if self.eat(TokenKind::Spawn) {
            let name = self.expect(TokenKind::Ident, "task name")?;
            self.expect(TokenKind::Equal, "'=' in spawn statement")?;
            let call = self.parse_expr(0)?;
            if !matches!(call.kind, ExprKind::Call { .. }) {
                self.error("AIF500", "spawn requires a function call");
                return None;
            }
            let end = call.span.end;
            self.expect(TokenKind::Newline, "end of spawn statement")?;
            return Some(Stmt {
                id: self.id("spawn", &start.to_string()),
                span: source::Span::new(start, end),
                kind: StmtKind::Spawn {
                    name: name.lexeme,
                    call,
                },
            });
        }

        if self.eat(TokenKind::Join) || self.eat(TokenKind::Cancel) {
            let is_cancel = self.tokens[self.pos.saturating_sub(1)].kind == TokenKind::Cancel;
            let name = self.expect(TokenKind::Ident, "task name")?;
            let end = name.span.end;
            self.expect(TokenKind::Newline, "end of task control statement")?;
            return Some(Stmt {
                id: self.id(
                    if is_cancel { "cancel" } else { "join" },
                    &start.to_string(),
                ),
                span: source::Span::new(start, end),
                kind: if is_cancel {
                    StmtKind::Cancel { name: name.lexeme }
                } else {
                    StmtKind::Join { name: name.lexeme }
                },
            });
        }

        if self.eat(TokenKind::While) {
            let condition = self.parse_expr(0)?;
            self.expect(TokenKind::Newline, "end of while condition")?;
            let body = self.parse_block("while body")?;
            let end = body.span.end;
            return Some(Stmt {
                id: self.id("while", &start.to_string()),
                span: source::Span::new(start, end),
                kind: StmtKind::While { condition, body },
            });
        }

        let expr = self.parse_expr(0)?;
        let end = expr.span.end;
        if !matches!(expr.kind, ExprKind::If { .. }) {
            self.expect(TokenKind::Newline, "end of expression statement")?;
        }
        Some(Stmt {
            id: self.id("expr", &start.to_string()),
            span: source::Span::new(start, end),
            kind: StmtKind::Expr(expr),
        })
    }

    fn parse_expr(&mut self, min_bp: u8) -> Option<Expr> {
        let mut left = self.parse_prefix()?;

        loop {
            if self.eat(TokenKind::LBracket) {
                let index = self.parse_expr(0)?;
                let close = self.expect(TokenKind::RBracket, "']' after index")?;
                let start = left.span.start;
                left = Expr {
                    id: self.id("index", &format!("{}:{}", start, close.span.end)),
                    span: source::Span::new(start, close.span.end),
                    kind: ExprKind::Index {
                        collection: Box::new(left),
                        index: Box::new(index),
                    },
                };
                continue;
            }

            if self.eat(TokenKind::LParen) {
                let start = left.span.start;
                let mut args = Vec::new();
                if !self.at(TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr(0)?);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let close = self.expect(TokenKind::RParen, "')' after call arguments")?;
                left = Expr {
                    id: self.id("call", &start.to_string()),
                    span: source::Span::new(start, close.span.end),
                    kind: ExprKind::Call {
                        callee: Box::new(left),
                        args,
                    },
                };
                continue;
            }

            let (op, left_bp, right_bp) = match self.current().kind {
                TokenKind::EqualEqual => (BinaryOp::Equal, 1, 2),
                TokenKind::NotEqual => (BinaryOp::NotEqual, 1, 2),
                TokenKind::LAngle => (BinaryOp::Less, 1, 2),
                TokenKind::LessEqual => (BinaryOp::LessEqual, 1, 2),
                TokenKind::RAngle => (BinaryOp::Greater, 1, 2),
                TokenKind::GreaterEqual => (BinaryOp::GreaterEqual, 1, 2),
                TokenKind::Plus => (BinaryOp::Add, 3, 4),
                TokenKind::Minus => (BinaryOp::Sub, 3, 4),
                TokenKind::Star => (BinaryOp::Mul, 5, 6),
                TokenKind::Slash => (BinaryOp::Div, 5, 6),
                TokenKind::Percent => (BinaryOp::Mod, 5, 6),
                _ => break,
            };
            if left_bp < min_bp {
                break;
            }
            self.bump();
            let right = self.parse_expr(right_bp)?;
            let start = left.span.start;
            let end = right.span.end;
            left = Expr {
                id: self.id("binary", &format!("{start}:{end}")),
                span: source::Span::new(start, end),
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        Some(left)
    }

    fn parse_prefix(&mut self) -> Option<Expr> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Int => {
                self.bump();
                let value = token.lexeme.parse::<i64>().ok()?;
                Some(self.expr(token.span, "int", ExprKind::Int(value)))
            }
            TokenKind::String => {
                self.bump();
                Some(self.expr(token.span, "string", ExprKind::String(token.lexeme)))
            }
            TokenKind::True | TokenKind::False => {
                self.bump();
                Some(self.expr(
                    token.span,
                    "bool",
                    ExprKind::Bool(token.kind == TokenKind::True),
                ))
            }
            TokenKind::Ident => {
                self.bump();
                Some(self.expr(token.span, "name", ExprKind::Name(token.lexeme)))
            }
            TokenKind::LBracket => {
                let start = self.bump().span.start;
                let mut elements = Vec::new();
                if !self.at(TokenKind::RBracket) {
                    loop {
                        elements.push(self.parse_expr(0)?);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let close = self.expect(TokenKind::RBracket, "']' after list")?;
                Some(Expr {
                    id: self.id("list", &start.to_string()),
                    span: source::Span::new(start, close.span.end),
                    kind: ExprKind::List(elements),
                })
            }
            TokenKind::LParen => {
                let start = self.bump().span.start;
                let expr = self.parse_expr(0)?;
                let close = self.expect(TokenKind::RParen, "')' after expression")?;
                Some(Expr {
                    id: self.id("group", &start.to_string()),
                    span: source::Span::new(start, close.span.end),
                    kind: ExprKind::Group(Box::new(expr)),
                })
            }
            TokenKind::If => self.parse_if(),
            _ => {
                self.error("AIF203", "expected an expression");
                None
            }
        }
    }

    fn parse_if(&mut self) -> Option<Expr> {
        let start = self.bump().span.start;
        let condition = self.parse_expr(0)?;
        self.expect(TokenKind::Newline, "end of if condition")?;
        let then_branch = self.parse_block("if body")?;

        let else_branch = if self.eat(TokenKind::Else) {
            self.expect(TokenKind::Newline, "end of else declaration")?;
            Some(self.parse_block("else body")?)
        } else {
            None
        };
        let end = else_branch
            .as_ref()
            .map_or(then_branch.span.end, |block| block.span.end);

        Some(Expr {
            id: self.id("if", &start.to_string()),
            span: source::Span::new(start, end),
            kind: ExprKind::If {
                condition: Box::new(condition),
                then_branch,
                else_branch,
            },
        })
    }

    fn expr(&mut self, span: source::Span, kind: &str, value: ExprKind) -> Expr {
        Expr {
            id: self.id(kind, &span.start.to_string()),
            span,
            kind: value,
        }
    }

    fn id(&mut self, kind: &str, key: &str) -> NodeId {
        let semantic_key = format!("{kind}:{key}");
        let occurrence = self.occurrences.entry(semantic_key.clone()).or_insert(0);
        let current = *occurrence;
        *occurrence += 1;
        NodeId(fnv1a(format!("{semantic_key}:{current}").as_bytes()))
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn previous_span(&self) -> source::Span {
        if self.pos == 0 {
            self.current().span
        } else {
            self.tokens[self.pos - 1].span
        }
    }

    fn bump(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.current().kind == kind
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind, what: &str) -> Option<Token> {
        if self.at(kind) {
            Some(self.bump())
        } else {
            self.error(
                "AIF204",
                format!("expected {what}, found {:?}", self.current().kind),
            );
            None
        }
    }

    fn skip_newlines(&mut self) {
        while self.at(TokenKind::Newline) {
            self.bump();
        }
    }

    fn recover_top_level(&mut self) {
        while !matches!(self.current().kind, TokenKind::Newline | TokenKind::Eof) {
            self.bump();
        }
        self.skip_newlines();
    }

    fn recover_statement(&mut self) {
        while !matches!(
            self.current().kind,
            TokenKind::Newline | TokenKind::Dedent | TokenKind::Eof
        ) {
            self.bump();
        }
        self.skip_newlines();
    }

    fn error(&mut self, code: &'static str, message: impl Into<String>) {
        self.errors.push(source::Diagnostic::error(
            code,
            message,
            Some(self.current().span),
        ));
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_expression_ast() {
        let module = parse("module hello\nfn add(a: Int, b: Int) -> Int\n  a + b * 2\n").unwrap();
        let Item::Function(function) = &module.items[0];
        assert_eq!(function.params.len(), 2);
        assert!(matches!(
            function.body.stmts[0].kind,
            StmtKind::Expr(Expr {
                kind: ExprKind::Binary {
                    op: BinaryOp::Add,
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn parses_if_and_result_type() {
        let source = "module math\nfn divide(a: Int, b: Int) -> Result<Int, MathError>\n  if b == 0\n    return 0\n  else\n    return a / b\n";
        let module = parse(source).unwrap();
        let Item::Function(function) = &module.items[0];
        assert!(matches!(
            function.return_type.as_ref().unwrap().kind,
            TypeKind::Result(_, _)
        ));
        assert!(matches!(
            function.body.stmts[0].kind,
            StmtKind::Expr(Expr {
                kind: ExprKind::If {
                    else_branch: Some(_),
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn node_ids_ignore_whitespace_changes() {
        let a = parse("module x\nfn f() -> Int\n  1\n").unwrap();
        let b = parse("module x\n\nfn f() -> Int\n    1\n").unwrap();
        let Item::Function(fa) = &a.items[0];
        let Item::Function(fb) = &b.items[0];
        assert_eq!(fa.id, fb.id);
    }
}
