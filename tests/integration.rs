use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use tianshu_formula::{ErrorKind, Formula, MapContext, Registry};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn golden_fixtures() {
    let path = fixture_dir().join("golden.json");
    let raw = fs::read_to_string(&path).expect("read golden.json");
    let cases: Vec<GoldenCase> = serde_json::from_str(&raw).expect("parse golden.json");
    for case in cases {
        let f = Formula::parse(&case.expr).unwrap_or_else(|e| {
            panic!("parse failed for `{}`: {e}", case.expr);
        });
        let mut ctx = MapContext::new();
        for (k, v) in &case.vars {
            ctx.insert(k.clone(), *v);
        }
        let got = f.eval(&ctx).unwrap_or_else(|e| {
            panic!("eval failed for `{}`: {e}", case.expr);
        });
        let diff = (got - case.expect).abs();
        assert!(
            diff <= case.eps.unwrap_or(1e-9),
            "case `{}`: got {got}, expect {}, eps {:?}",
            case.expr,
            case.expect,
            case.eps
        );
    }
}

#[derive(serde::Deserialize)]
struct GoldenCase {
    expr: String,
    #[serde(default)]
    vars: Vec<(String, f64)>,
    expect: f64,
    eps: Option<f64>,
}

#[test]
fn error_cases() {
    assert_eq!(Formula::parse("").unwrap_err().kind, ErrorKind::EmptyInput);
    assert_eq!(Formula::parse("(1 + 2").unwrap_err().kind, ErrorKind::Parse);
    assert_eq!(
        Formula::parse("nope(1)").unwrap_err().kind,
        ErrorKind::UnknownFunction
    );
    let f = Formula::parse("missing + 1").unwrap();
    assert_eq!(
        f.eval(&MapContext::new()).unwrap_err().kind,
        ErrorKind::UndefinedVariable
    );
    assert_eq!(
        Formula::parse("sqrt(-1)").unwrap_err().kind,
        ErrorKind::Domain
    );
    assert_eq!(
        Formula::parse("1 % 0").unwrap_err().kind,
        ErrorKind::DivByZero
    );
    assert_eq!(
        Formula::parse("clamp(1, 5, 0)").unwrap_err().kind,
        ErrorKind::Domain
    );
}

#[test]
fn and_or_fold_coerces_to_bool() {
    let f = Formula::parse("1 && 5").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 1.0);
    let f = Formula::parse("0 || 5").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 1.0);
    let f = Formula::parse("(1 && 5) * 2").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 2.0);
    let f = Formula::parse("1 && x").unwrap();
    let mut ctx = MapContext::new();
    ctx.insert("x", 5.0);
    assert_eq!(f.eval(&ctx).unwrap(), 1.0);
}

#[test]
fn if_select_short_circuit_fold() {
    // Dead branch must not be evaluated at compile time.
    let f = Formula::parse("if(1, 2, 1/0)").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 2.0);
    let f = Formula::parse("if(0, 1/0, 3)").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 3.0);
    let f = Formula::parse("select(1, 4, sqrt(-1))").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 4.0);
    let f = Formula::parse("1 ? 2 : 1/0").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 2.0);
}

#[test]
fn short_circuit_or_skips_undefined() {
    let f = Formula::parse("1 || missing").unwrap();
    assert_eq!(f.eval(&MapContext::new()).unwrap(), 1.0);
}

#[test]
fn context_nan_is_error() {
    let f = Formula::parse("x").unwrap();
    let mut ctx = MapContext::new();
    ctx.insert("x", f64::NAN);
    assert_eq!(f.eval(&ctx).unwrap_err().kind, ErrorKind::Nan);
}

#[test]
fn batch_length_mismatch() {
    let f = Formula::parse("1").unwrap();
    let contexts = [MapContext::new()];
    let mut out = [0.0, 0.0];
    assert_eq!(
        f.eval_batch_into(&contexts, &mut out).unwrap_err().kind,
        ErrorKind::Arity
    );
}

#[test]
fn cannot_override_builtin_constant_name() {
    let mut reg = Registry::new();
    assert_eq!(
        reg.register("pi", Arc::new(|_a| Ok(0.0))).unwrap_err().kind,
        ErrorKind::BuiltinOverride
    );
}

#[tokio::test]
async fn async_and_batch() {
    let f = Formula::parse("a + b").unwrap();
    let contexts: Vec<MapContext> = (0..100)
        .map(|i| {
            let mut c = MapContext::new();
            c.insert("a", i as f64).insert("b", 1.0);
            c
        })
        .collect();
    let batch = f.eval_batch_async(&contexts).await.unwrap();
    assert_eq!(batch.len(), 100);
    assert_eq!(batch[10], 11.0);

    let mut out = vec![0.0; 100];
    f.eval_batch_into(&contexts, &mut out).unwrap();
    assert_eq!(out[10], 11.0);

    let v = f
        .eval_async(contexts.first().expect("non-empty"))
        .await
        .unwrap();
    assert_eq!(v, 1.0);
}

#[test]
fn custom_registry_and_override_guard() {
    let mut reg = Registry::new();
    assert_eq!(
        reg.register("abs", Arc::new(|_a| Ok(0.0)))
            .unwrap_err()
            .kind,
        ErrorKind::BuiltinOverride
    );
    reg.register(
        "triple",
        Arc::new(|args| {
            if args.len() != 1 {
                return Err(tianshu_formula::Error::new(ErrorKind::Arity, "bad"));
            }
            Ok(args[0] * 3.0)
        }),
    )
    .unwrap();
    let f = Formula::parse_with("triple(4)", Some(&reg)).unwrap();
    assert_eq!(f.eval_with(&MapContext::new(), Some(&reg)).unwrap(), 12.0);
}

#[test]
fn serde_roundtrip() {
    let f = Formula::parse("1 + 2 * pi").unwrap();
    let json = serde_json::to_string(&f).unwrap();
    let back: Formula = serde_json::from_str(&json).unwrap();
    assert_eq!(f, back);
}

#[test]
fn proptest_no_panic_on_balanced_noise() {
    use proptest::prelude::*;
    proptest!(|(digits in prop::collection::vec(0u8..10, 1..8))| {
        let expr = digits
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("+");
        let f = Formula::parse(&expr).expect("parse");
        let got = f.eval(&MapContext::new()).expect("eval");
        let expect: f64 = digits.iter().map(|d| f64::from(*d)).sum();
        prop_assert!((got - expect).abs() < 1e-9);
    });
}
