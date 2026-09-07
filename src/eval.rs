//! Synchronous evaluation and [`Formula`] API.

use serde::{Deserialize, Serialize};

use crate::ast::{BinaryOp, Expr};
use crate::builtins::{builtin_constant, call_function, is_truthy, Registry};
use crate::compile::{eval_binary, eval_unary, fold_constants};
use crate::context::Context;
use crate::error::{Error, ErrorKind};
use crate::parser;
use crate::FORMULA_VERSION;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Formula {
    pub version: u32,
    pub expr: Expr,
}

impl Formula {
    pub fn parse(source: &str) -> Result<Self, Error> {
        Self::parse_with(source, None)
    }

    pub fn parse_with(source: &str, registry: Option<&Registry>) -> Result<Self, Error> {
        let ast = parser::parse(source)?;
        validate_calls(&ast, registry, 0)?;
        let expr = fold_constants(ast, registry)?;
        Ok(Self {
            version: FORMULA_VERSION,
            expr,
        })
    }

    pub fn eval(&self, ctx: &dyn Context) -> Result<f64, Error> {
        self.eval_with(ctx, None)
    }

    pub fn eval_with(&self, ctx: &dyn Context, registry: Option<&Registry>) -> Result<f64, Error> {
        eval_expr(&self.expr, ctx, registry, 0)
    }
}

fn validate_calls(expr: &Expr, registry: Option<&Registry>, depth: usize) -> Result<(), Error> {
    crate::check_ast_depth(depth)?;
    match expr {
        Expr::Literal { .. } | Expr::Variable { .. } => Ok(()),
        Expr::Unary { expr, .. } => validate_calls(expr, registry, depth + 1),
        Expr::Binary { left, right, .. } => {
            validate_calls(left, registry, depth + 1)?;
            validate_calls(right, registry, depth + 1)
        }
        Expr::Ternary {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            validate_calls(cond, registry, depth + 1)?;
            validate_calls(then_branch, registry, depth + 1)?;
            validate_calls(else_branch, registry, depth + 1)
        }
        Expr::Call { name, args, span } => {
            for a in args {
                validate_calls(a, registry, depth + 1)?;
            }
            let known = crate::builtins::builtin_fn(name).is_some()
                || registry.and_then(|r| r.get_custom(name)).is_some();
            if !known {
                return Err(Error::new(
                    ErrorKind::UnknownFunction,
                    format!("unknown function `{name}`"),
                )
                .with_name(name.clone())
                .with_span(*span));
            }
            Ok(())
        }
    }
}

fn eval_expr(
    expr: &Expr,
    ctx: &dyn Context,
    registry: Option<&Registry>,
    depth: usize,
) -> Result<f64, Error> {
    crate::check_ast_depth(depth)?;
    match expr {
        Expr::Literal { value, .. } => {
            if value.is_nan() {
                Err(Error::new(ErrorKind::Nan, "literal NaN"))
            } else {
                Ok(*value)
            }
        }
        Expr::Variable { name, span } => {
            if let Some(v) = builtin_constant(name) {
                return Ok(v);
            }
            match ctx.get(name) {
                Some(v) if v.is_nan() => Err(Error::new(
                    ErrorKind::Nan,
                    format!("variable `{name}` is NaN"),
                )
                .with_name(name.clone())
                .with_span(*span)),
                Some(v) => Ok(v),
                None => Err(Error::new(
                    ErrorKind::UndefinedVariable,
                    format!("undefined variable `{name}`"),
                )
                .with_name(name.clone())
                .with_span(*span)),
            }
        }
        Expr::Unary { op, expr, .. } => {
            let v = eval_expr(expr, ctx, registry, depth + 1)?;
            eval_unary(*op, v)
        }
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => match op {
            BinaryOp::And => {
                let a = eval_expr(left, ctx, registry, depth + 1)?;
                if !is_truthy(a) {
                    return Ok(0.0);
                }
                let b = eval_expr(right, ctx, registry, depth + 1)?;
                Ok(if is_truthy(b) { 1.0 } else { 0.0 })
            }
            BinaryOp::Or => {
                let a = eval_expr(left, ctx, registry, depth + 1)?;
                if is_truthy(a) {
                    return Ok(1.0);
                }
                let b = eval_expr(right, ctx, registry, depth + 1)?;
                Ok(if is_truthy(b) { 1.0 } else { 0.0 })
            }
            _ => {
                let a = eval_expr(left, ctx, registry, depth + 1)?;
                let b = eval_expr(right, ctx, registry, depth + 1)?;
                eval_binary(*op, a, b, *span)
            }
        },
        Expr::Ternary {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            let c = eval_expr(cond, ctx, registry, depth + 1)?;
            if is_truthy(c) {
                eval_expr(then_branch, ctx, registry, depth + 1)
            } else {
                eval_expr(else_branch, ctx, registry, depth + 1)
            }
        }
        Expr::Call { name, args, span } => {
            // Short-circuit for if/select: only eval chosen branch
            if name == "if" || name == "select" {
                if args.len() != 3 {
                    return Err(Error::new(
                        ErrorKind::Arity,
                        format!("{name} expects 3 arguments, got {}", args.len()),
                    )
                    .with_name(name.clone())
                    .with_span(*span));
                }
                let c = eval_expr(&args[0], ctx, registry, depth + 1)?;
                return if is_truthy(c) {
                    eval_expr(&args[1], ctx, registry, depth + 1)
                } else {
                    eval_expr(&args[2], ctx, registry, depth + 1)
                };
            }
            let mut values = Vec::with_capacity(args.len());
            for a in args {
                values.push(eval_expr(a, ctx, registry, depth + 1)?);
            }
            call_function(name, &values, registry).map_err(|e| {
                if e.span.is_none() {
                    e.with_span(*span)
                } else {
                    e
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::MapContext;

    #[test]
    fn arithmetic() {
        let f = Formula::parse("1 + 2 * 3").unwrap();
        assert_eq!(f.eval(&MapContext::new()).unwrap(), 7.0);
    }

    #[test]
    fn variables_and_clamp() {
        let f = Formula::parse("clamp(atk * 1.5, 0, max_dmg)").unwrap();
        let mut ctx = MapContext::new();
        ctx.insert("atk", 10.0).insert("max_dmg", 12.0);
        assert_eq!(f.eval(&ctx).unwrap(), 12.0);
    }

    #[test]
    fn div_by_zero() {
        let err = Formula::parse("1 / 0").unwrap_err();
        assert_eq!(err.kind, ErrorKind::DivByZero);
    }

    #[test]
    fn div_by_zero_runtime() {
        let f = Formula::parse("1 / x").unwrap();
        let mut ctx = MapContext::new();
        ctx.insert("x", 0.0);
        let err = f.eval(&ctx).unwrap_err();
        assert_eq!(err.kind, ErrorKind::DivByZero);
    }

    #[test]
    fn short_circuit_and() {
        let f = Formula::parse("x && missing").unwrap();
        let mut ctx = MapContext::new();
        ctx.insert("x", 0.0);
        assert_eq!(f.eval(&ctx).unwrap(), 0.0);
        ctx.insert("x", 1.0);
        let err = f.eval(&ctx).unwrap_err();
        assert_eq!(err.kind, ErrorKind::UndefinedVariable);
    }

    #[test]
    fn short_circuit_or() {
        let f = Formula::parse("x || missing").unwrap();
        let mut ctx = MapContext::new();
        ctx.insert("x", 1.0);
        assert_eq!(f.eval(&ctx).unwrap(), 1.0);
        ctx.insert("x", 0.0);
        let err = f.eval(&ctx).unwrap_err();
        assert_eq!(err.kind, ErrorKind::UndefinedVariable);
    }

    #[test]
    fn if_skips_dead_div_by_zero_branch() {
        let f = Formula::parse("if(x, 2, 1/0)").unwrap();
        let mut ctx = MapContext::new();
        ctx.insert("x", 1.0);
        assert_eq!(f.eval(&ctx).unwrap(), 2.0);
        ctx.insert("x", 0.0);
        let miss = Formula::parse("if(x, 1/0, 3)").unwrap();
        assert_eq!(miss.eval(&ctx).unwrap(), 3.0);
    }

    #[test]
    fn unary_neg_and_folded_or() {
        let f = Formula::parse("-x").unwrap();
        let mut ctx = MapContext::new();
        ctx.insert("x", 3.0);
        assert_eq!(f.eval(&ctx).unwrap(), -3.0);
        assert_eq!(
            Formula::parse("1 || 0")
                .unwrap()
                .eval(&MapContext::new())
                .unwrap(),
            1.0
        );
        assert_eq!(
            Formula::parse("1 && 0")
                .unwrap()
                .eval(&MapContext::new())
                .unwrap(),
            0.0
        );
    }

    #[test]
    fn comparison_boundaries_and_unary_not() {
        let f = Formula::parse("(x <= y) + (x < y) + (x >= y) + (x > y) + (!x) + (!z)").unwrap();
        let mut ctx = MapContext::new();
        ctx.insert("x", 1.0).insert("y", 1.0).insert("z", 0.0);
        assert_eq!(f.eval(&ctx).unwrap(), 3.0);
    }

    #[test]
    fn ternary() {
        let f = Formula::parse("x > 0 ? x : -x").unwrap();
        let mut ctx = MapContext::new();
        ctx.insert("x", -3.0);
        assert_eq!(f.eval(&ctx).unwrap(), 3.0);
    }

    #[test]
    fn pow_right_assoc_value() {
        let f = Formula::parse("2^3^2").unwrap();
        assert_eq!(f.eval(&MapContext::new()).unwrap(), 512.0);
    }

    #[test]
    fn deep_add_chain_is_parse_error_not_stack_overflow() {
        let n = crate::MAX_AST_DEPTH + 50;
        let mut src = String::from("1");
        for _ in 0..n {
            src.push_str("+1");
        }
        let err = Formula::parse(&src).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert!(err.message.contains("deeply nested"));
    }

    #[test]
    fn nested_parens_over_cap_are_parse_error() {
        let n = 80;
        let src = format!("{}1{}", "(".repeat(n), ")".repeat(n));
        let err = Formula::parse(&src).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
    }

    #[test]
    fn nested_calls_over_cap_are_parse_error() {
        let n = 80;
        let src = format!("{}1{}", "abs(".repeat(n), ")".repeat(n));
        let err = Formula::parse(&src).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert!(err.message.contains("deeply nested"));
    }

    #[test]
    fn modest_add_chain_still_parses() {
        let mut src = String::from("1");
        for _ in 0..32 {
            src.push_str("+1");
        }
        let f = Formula::parse(&src).unwrap();
        assert_eq!(f.eval(&MapContext::new()).unwrap(), 33.0);
    }
}
