//! Property-based generation of formulas — parse/eval must never panic.

use proptest::prelude::*;
use tianshu_formula::{Formula, MapContext};

#[derive(Clone, Debug)]
enum GenExpr {
    Num(f64),
    Var(&'static str),
    UnaryNeg(Box<GenExpr>),
    UnaryNot(Box<GenExpr>),
    Bin(&'static str, Box<GenExpr>, Box<GenExpr>),
    Call(&'static str, Vec<GenExpr>),
    Ternary(Box<GenExpr>, Box<GenExpr>, Box<GenExpr>),
}

impl GenExpr {
    fn to_source(&self) -> String {
        match self {
            GenExpr::Num(n) => {
                // Avoid NaN/-0 formatting surprises; stick to simple decimals.
                if n.fract() == 0.0 && n.abs() < 1e9 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            GenExpr::Var(v) => (*v).to_string(),
            GenExpr::UnaryNeg(e) => format!("-({})", e.to_source()),
            GenExpr::UnaryNot(e) => format!("!({})", e.to_source()),
            GenExpr::Bin(op, a, b) => format!("({} {} {})", a.to_source(), op, b.to_source()),
            GenExpr::Call(name, args) => {
                let inner = args
                    .iter()
                    .map(|a| a.to_source())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}({inner})")
            }
            GenExpr::Ternary(c, t, e) => {
                format!(
                    "(({}) ? ({}) : ({}))",
                    c.to_source(),
                    t.to_source(),
                    e.to_source()
                )
            }
        }
    }
}

fn gen_expr() -> impl Strategy<Value = GenExpr> {
    let leaf = prop_oneof![
        (-50i32..50).prop_map(|n| GenExpr::Num(n as f64)),
        Just(GenExpr::Var("a")),
        Just(GenExpr::Var("b")),
        Just(GenExpr::Var("c")),
        Just(GenExpr::Num(0.0)),
        Just(GenExpr::Num(1.0)),
    ];

    leaf.prop_recursive(4, 32, 3, |inner| {
        prop_oneof![
            inner.clone().prop_map(|e| GenExpr::UnaryNeg(Box::new(e))),
            inner.clone().prop_map(|e| GenExpr::UnaryNot(Box::new(e))),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| GenExpr::Bin(
                "+",
                Box::new(a),
                Box::new(b)
            )),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| GenExpr::Bin(
                "-",
                Box::new(a),
                Box::new(b)
            )),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| GenExpr::Bin(
                "*",
                Box::new(a),
                Box::new(b)
            )),
            // Prefer non-zero-looking divisors by wrapping abs(x)+1 style via bin with + 1 later;
            // still allow / and % — errors are OK, panics are not.
            (inner.clone(), inner.clone()).prop_map(|(a, b)| GenExpr::Bin(
                "/",
                Box::new(a),
                Box::new(b)
            )),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| GenExpr::Bin(
                "<",
                Box::new(a),
                Box::new(b)
            )),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| GenExpr::Bin(
                "&&",
                Box::new(a),
                Box::new(b)
            )),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| GenExpr::Bin(
                "||",
                Box::new(a),
                Box::new(b)
            )),
            (inner.clone(), inner.clone(), inner.clone())
                .prop_map(|(c, t, e)| { GenExpr::Ternary(Box::new(c), Box::new(t), Box::new(e)) }),
            inner.clone().prop_map(|e| GenExpr::Call("abs", vec![e])),
            inner.clone().prop_map(|e| GenExpr::Call("floor", vec![e])),
            (inner.clone(), inner.clone(), inner.clone()).prop_map(|(x, lo, hi)| {
                // Ensure lo <= hi structurally when both are nums is hard; clamp may Domain — OK.
                GenExpr::Call("clamp", vec![x, lo, hi])
            }),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| GenExpr::Call("min", vec![a, b])),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| GenExpr::Call("max", vec![a, b])),
            (inner.clone(), inner.clone(), inner.clone())
                .prop_map(|(c, a, b)| { GenExpr::Call("if", vec![c, a, b]) }),
        ]
    })
}

#[test]
fn generated_expr_never_panics() {
    proptest!(|(expr in gen_expr())| {
        let src = expr.to_source();
        if let Ok(f) = Formula::parse(&src) {
            let mut ctx = MapContext::new();
            ctx.insert("a", 2.0).insert("b", 3.0).insert("c", 0.0);
            let _ = f.eval(&ctx);
        }
    });
}

#[test]
fn random_digit_sums_match_oracle() {
    proptest!(|(digits in prop::collection::vec(0u8..10, 1..12))| {
        let expr = digits
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("+");
        let expect: f64 = digits.iter().map(|d| *d as f64).sum();
        let f = Formula::parse(&expr).expect("parse");
        let got = f.eval(&MapContext::new()).expect("eval");
        prop_assert!((got - expect).abs() < 1e-9);
    });
}

#[test]
fn random_mul_add_matches_oracle() {
    proptest!(|(a in -20i32..20, b in -20i32..20, c in -20i32..20)| {
        let expr = format!("{a} + {b} * {c}");
        let expect = a as f64 + (b as f64) * (c as f64);
        let f = Formula::parse(&expr).expect("parse");
        let got = f.eval(&MapContext::new()).expect("eval");
        prop_assert_eq!(got, expect);
    });
}
