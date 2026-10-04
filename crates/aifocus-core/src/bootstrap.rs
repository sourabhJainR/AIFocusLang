use crate::{ir, native, ownership, parse, sema};

pub const BOOTSTRAP_SOURCE: &str = "module bootstrap\nfn main(a: Int, b: Int) -> Int\n  a + b * 2\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapReport {
    pub parsed: bool,
    pub semantically_valid: bool,
    pub ownership_valid: bool,
    pub native_compiled: bool,
    pub native_result: Option<native::NativeValue>,
    pub self_hosting_ready: bool,
    pub blocker: Option<&'static str>,
}

pub fn verify() -> BootstrapReport {
    let Ok(module) = parse(BOOTSTRAP_SOURCE) else {
        return BootstrapReport { parsed:false, semantically_valid:false, ownership_valid:false, native_compiled:false, native_result:None, self_hosting_ready:false, blocker:Some("bootstrap source does not parse") };
    };
    if sema::check(&module).is_err() {
        return BootstrapReport { parsed:true, semantically_valid:false, ownership_valid:false, native_compiled:false, native_result:None, self_hosting_ready:false, blocker:Some("bootstrap source fails semantic validation") };
    }
    if ownership::infer(&module).is_err() {
        return BootstrapReport { parsed:true, semantically_valid:true, ownership_valid:false, native_compiled:false, native_result:None, self_hosting_ready:false, blocker:Some("bootstrap source fails ownership validation") };
    }
    let Ok(code) = native::compile(&ir::lower(&module)) else {
        return BootstrapReport { parsed:true, semantically_valid:true, ownership_valid:true, native_compiled:false, native_result:None, self_hosting_ready:false, blocker:Some("native backend cannot compile the bootstrap subset") };
    };
    let native_result = native::run(&code, &[("a".into(), native::NativeValue::Int(3)), ("b".into(), native::NativeValue::Int(4))]).ok();
    BootstrapReport {
        parsed:true,
        semantically_valid:true,
        ownership_valid:true,
        native_compiled:true,
        native_result,
        self_hosting_ready:false,
        blocker:Some("full self-hosting still requires compiler implementation expressible in Ardisa and native support for its required language/runtime features"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_contract_is_green_but_does_not_fake_self_hosting() {
        let report = verify();
        assert!(report.parsed);
        assert!(report.semantically_valid);
        assert!(report.ownership_valid);
        assert!(report.native_compiled);
        assert_eq!(report.native_result, Some(crate::native::NativeValue::Int(11)));
        assert!(!report.self_hosting_ready);
        assert!(report.blocker.is_some());
    }
}
