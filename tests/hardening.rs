//! Systematic error fixtures + hardening tests.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use tianshu_formula::{ErrorKind, Formula, MapContext, Registry};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[derive(serde::Deserialize)]
struct ErrorCase {
    expr: String,
    #[serde(default)]
    vars: Vec<(String, f64)>,
    #[serde(default)]
    nan_vars: Vec<String>,
    kind: String,
    #[serde(default)]
    at_parse: bool,
}

fn kind_from(s: &str) -> ErrorKind {
    match s {
        "EmptyInput" => ErrorKind::EmptyInput,
        "Lex" => ErrorKind::Lex,
        "Parse" => ErrorKind::Parse,
        "UnknownFunction" => ErrorKind::UnknownFunction,
        "Arity" => ErrorKind::Arity,
        "UndefinedVariable" => ErrorKind::UndefinedVariable,
        "DivByZero" => ErrorKind::DivByZero,
        "Domain" => ErrorKind::Domain,
        "Nan" => ErrorKind::Nan,
        "BuiltinOverride" => ErrorKind::BuiltinOverride,
        "Custom" => ErrorKind::Custom,
        other => panic!("unknown kind in fixture: {other}"),
    }
}

#[test]
fn error_fixtures() {
    let path = fixture_dir().join("errors.json");
    let raw = fs::read_to_string(&path).expect("read errors.json");
    let cases: Vec<ErrorCase> = serde_json::from_str(&raw).expect("parse errors.json");
    for case in cases {
        let expect = kind_from(&case.kind);
        if case.at_parse {
            let err = Formula::parse(&case.expr)
                .expect_err(&format!("expected parse error for `{}`", case.expr));
            assert_eq!(err.kind, expect, "parse kind for `{}`", case.expr);
        } else {
            let f = Formula::parse(&case.expr).unwrap_or_else(|e| {
                panic!("parse unexpectedly failed for `{}`: {e}", case.expr);
            });
            let mut ctx = MapContext::new();
            for (k, v) in &case.vars {
                ctx.insert(k.clone(), *v);
            }
            for k in &case.nan_vars {
                ctx.insert(k.clone(), f64::NAN);
            }
            let err = f
                .eval(&ctx)
                .expect_err(&format!("expected eval error for `{}`", case.expr));
            assert_eq!(err.kind, expect, "eval kind for `{}`", case.expr);
        }
    }
}

#[test]
fn oracle_matches_rust_f64() {
    let cases = [
        ("1 + 2 * 3 - 4 / 2", 1.0 + 2.0 * 3.0 - 4.0 / 2.0),
        ("(1 + 2) * (3 - 4)", (1.0 + 2.0) * (3.0 - 4.0)),
        ("2^10", 2.0_f64.powi(10)),
        ("abs(-7.5)", (-7.5_f64).abs()),
        (
            "floor(3.7) + ceil(-1.2)",
            3.7_f64.floor() + (-1.2_f64).ceil(),
        ),
        ("hypot(5, 12)", 5.0_f64.hypot(12.0)),
        ("sin(0) + cos(0)", 0.0_f64.sin() + 0.0_f64.cos()),
        ("clamp(15, 0, 10)", 15.0_f64.clamp(0.0, 10.0)),
        ("min(9, -1, 3)", (-1.0_f64).min(9.0).min(3.0)),
        ("max(9, -1, 3)", (-1.0_f64).max(9.0).max(3.0)),
        ("sum(1, 2, 3)", 6.0),
        ("10 % 4", 10.0 % 4.0),
        ("lerp(2, 8, 0.25)", 2.0 + (8.0 - 2.0) * 0.25),
    ];
    for (expr, expect) in cases {
        let f = Formula::parse(expr).unwrap_or_else(|e| panic!("{expr}: {e}"));
        let got = f
            .eval(&MapContext::new())
            .unwrap_or_else(|e| panic!("{expr}: {e}"));
        assert!(
            (got - expect).abs() < 1e-12,
            "{expr}: got {got} expect {expect}"
        );
    }
}

#[test]
fn determinism_repeated_eval() {
    let f = Formula::parse("clamp(a * 1.5 + b, 0, 100) ? a : b").unwrap();
    let mut ctx = MapContext::new();
    ctx.insert("a", 10.0).insert("b", 3.0);
    let a = f.eval(&ctx).unwrap();
    for _ in 0..1000 {
        assert_eq!(f.eval(&ctx).unwrap(), a);
    }
}

#[test]
fn parse_eval_idempotent_after_serde() {
    let f = Formula::parse("max(1, 2, 3) + foo.bar").unwrap();
    let mut ctx = MapContext::new();
    ctx.insert("foo.bar", 4.0);
    let before = f.eval(&ctx).unwrap();
    let json = serde_json::to_string(&f).unwrap();
    let back: Formula = serde_json::from_str(&json).unwrap();
    assert_eq!(back.eval(&ctx).unwrap(), before);
}

#[test]
fn custom_fn_requires_registry_at_eval() {
    let mut reg = Registry::new();
    reg.register("inc", Arc::new(|a| Ok(a[0] + 1.0))).unwrap();
    let _folded = Formula::parse_with("inc(1)", Some(&reg)).unwrap();
    // Use non-literal arg so it can't fold away.
    let f = Formula::parse_with("inc(x)", Some(&reg)).unwrap();
    let mut ctx = MapContext::new();
    ctx.insert("x", 10.0);
    assert_eq!(f.eval_with(&ctx, Some(&reg)).unwrap(), 11.0);
    assert_eq!(f.eval(&ctx).unwrap_err().kind, ErrorKind::UnknownFunction);
}

#[test]
fn arity_errors() {
    assert_eq!(
        Formula::parse("clamp(1, 2)").unwrap_err().kind,
        ErrorKind::Arity
    );
    assert_eq!(
        Formula::parse("abs(1, 2)").unwrap_err().kind,
        ErrorKind::Arity
    );
    assert_eq!(Formula::parse("pow(2)").unwrap_err().kind, ErrorKind::Arity);
}

#[test]
fn lex_and_parse_garbage() {
    assert_eq!(Formula::parse("@").unwrap_err().kind, ErrorKind::Lex);
    assert_eq!(Formula::parse("1e").unwrap_err().kind, ErrorKind::Lex);
    assert_eq!(Formula::parse("1 +").unwrap_err().kind, ErrorKind::Parse);
    assert_eq!(Formula::parse("foo.").unwrap_err().kind, ErrorKind::Parse);
    assert_eq!(Formula::parse("a ? b").unwrap_err().kind, ErrorKind::Parse);
}

#[test]
fn pow_domain_nan() {
    // (-1)^0.5 → NaN under IEEE for real powf
    assert_eq!(
        Formula::parse("pow(-1, 0.5)").unwrap_err().kind,
        ErrorKind::Nan
    );
}

#[test]
fn inf_constant_allowed() {
    let f = Formula::parse("inf > 1e308").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 1.0);
}

#[test]
fn nested_short_circuit_chains() {
    let f = Formula::parse("0 && (1/0) && missing").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 0.0);
    let f = Formula::parse("1 || (1/0) || missing").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 1.0);
    // Dynamic cond must not fold dead branch errors away incorrectly.
    let f = Formula::parse("x ? 1 : 1/0").unwrap();
    let mut ctx = MapContext::new();
    ctx.insert("x", 1.0);
    assert_eq!(f.eval(&ctx).unwrap(), 1.0);
    let f = Formula::parse("if(x, 2, 1/0)").unwrap();
    assert_eq!(f.eval(&ctx).unwrap(), 2.0);
}

#[test]
fn unary_and_compare_mix() {
    let f = Formula::parse("!0 && !!1 && -( -3 ) == 3").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 1.0);
}

#[test]
fn batch_matches_single() {
    let f = Formula::parse("a * a + b").unwrap();
    let contexts: Vec<MapContext> = (0..50)
        .map(|i| {
            let mut c = MapContext::new();
            c.insert("a", i as f64).insert("b", i as f64 * 0.5);
            c
        })
        .collect();
    let batch = f.eval_batch(&contexts).unwrap();
    for (i, ctx) in contexts.iter().enumerate() {
        assert_eq!(batch[i], f.eval(ctx).unwrap());
    }
}
