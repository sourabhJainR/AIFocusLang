use ardisa_core::{format, parse, sema, ExprKind, Item, StmtKind, TypeKind};

fn parse_and_analyze(source: &str) -> (ardisa_core::Module, sema::SemanticModel) {
    let module = parse(source).expect("source should parse");
    let model = sema::analyze(&module).expect("source should pass semantic analysis");
    (module, model)
}

#[test]
fn infers_list_element_type_and_index_result() {
    let source = "module collections\nfn first() -> Int\n  let values = [10, 20, 30]\n  values[0]\n";
    let (module, model) = parse_and_analyze(source);
    let Item::Function(function) = &module.items[0];

    let StmtKind::Let { value, .. } = &function.body.stmts[0].kind else {
        panic!("expected list binding");
    };
    assert!(matches!(value.kind, ExprKind::List(_)));
    assert_eq!(
        model.inferred_types.get(&value.id).unwrap().kind,
        TypeKind::List(Box::new(ardisa_core::Type {
            id: ardisa_core::NodeId(0),
            span: value.span,
            kind: TypeKind::Int,
        }))
    );

    let StmtKind::Expr(index) = &function.body.stmts[1].kind else {
        panic!("expected indexed expression");
    };
    assert_eq!(model.inferred_types.get(&index.id).unwrap().kind, TypeKind::Int);
}

#[test]
fn rejects_mixed_type_list_elements() {
    let source = "module collections\nfn invalid()\n  [1, true]\n";
    let module = parse(source).expect("mixed-type list is syntactically valid");
    let errors = sema::analyze(&module).expect_err("mixed element types must be rejected");
    assert!(errors.iter().any(|error| error.code == "AIF317"));
}

#[test]
fn formatter_round_trips_list_literals_and_indexing() {
    let source = "module collections\nfn first(values: List<Int>) -> Int\n  values[0]\nfn sample() -> Int\n  first([4, 5])\n";
    let module = parse(source).expect("source should parse");
    let formatted = format::format_module(&module);
    let reparsed = parse(&formatted).expect("formatted source should parse");
    assert_eq!(format::format_module(&reparsed), formatted);
    assert!(sema::check(&reparsed).is_ok());
}
