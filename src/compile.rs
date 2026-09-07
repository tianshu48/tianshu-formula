//! Constant folding / compile pass.

use crate::ast::{BinaryOp, Expr, UnaryOp};
use crate::builtins::{builtin_constant, call_function, is_truthy, Registry};
use crate::error::{Error, ErrorKind};

pub fn fold_constants(expr: Expr, registry: Option<&Registry>) -> Result<Expr, Error> {
    fold_constants_at(expr, registry, 0)
}

fn fold_constants_at(expr: Expr, registry: Option<&Registry>, depth: usize) -> Result<Expr, Error> {
    crate::check_ast_depth(depth)?;
    match expr {
        Expr::Literal { .. } => Ok(expr),
        Expr::Variable { ref name, span } => {
            if let Some(v) = builtin_constant(name) {
                Ok(Expr::Literal { value: v, span })
            } else {
                Ok(expr)
            }
        }
        Expr::Unary { op, expr, span } => {
            let inner = fold_constants_at(*expr, registry, depth + 1)?;
            if let Expr::Literal { value, .. } = &inner {
                let v = eval_unary(op, *value)?;
                return Ok(Expr::Literal { value: v, span });
            }
            Ok(Expr::Unary {
                op,
                expr: Box::new(inner),
                span,
            })
        }
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => match op {
            // Fold left first; only fold right if not short-circuited.
            BinaryOp::And => fold_and_or(true, *left, *right, span, registry, depth + 1),
            BinaryOp::Or => fold_and_or(false, *left, *right, span, registry, depth + 1),
            _ => {
                let left = fold_constants_at(*left, registry, depth + 1)?;
                let right = fold_constants_at(*right, registry, depth + 1)?;
                if let (Expr::Literal { value: a, .. }, Expr::Literal { value: b, .. }) =
                    (&left, &right)
                {
                    let v = eval_binary(op, *a, *b, span)?;
                    return Ok(Expr::Literal { value: v, span });
                }
                Ok(Expr::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                    span,
                })
            }
        },
        Expr::Ternary {
            cond,
            then_branch,
            else_branch,
            span,
        } => {
            let cond = fold_constants_at(*cond, registry, depth + 1)?;
            if let Expr::Literal { value, .. } = &cond {
                return if is_truthy(*value) {
                    fold_constants_at(*then_branch, registry, depth + 1)
                } else {
                    fold_constants_at(*else_branch, registry, depth + 1)
                };
            }
            // Dynamic cond: do not fold branches (may contain intentional dead errors).
            Ok(Expr::Ternary {
                cond: Box::new(cond),
                then_branch,
                else_branch,
                span,
            })
        }
        Expr::Call { name, args, span } => {
            if (name == "if" || name == "select") && args.len() == 3 {
                let mut args = args;
                let else_raw = args.pop();
                let then_raw = args.pop();
                let cond_raw = args.pop();
                if let (Some(cond_raw), Some(then_raw), Some(else_raw)) =
                    (cond_raw, then_raw, else_raw)
                {
                    let cond = fold_constants_at(cond_raw, registry, depth + 1)?;
                    if let Expr::Literal { value, .. } = &cond {
                        return if is_truthy(*value) {
                            fold_constants_at(then_raw, registry, depth + 1)
                        } else {
                            fold_constants_at(else_raw, registry, depth + 1)
                        };
                    }
                    return Ok(Expr::Call {
                        name,
                        args: vec![cond, then_raw, else_raw],
                        span,
                    });
                }
                return Ok(Expr::Call {
                    name,
                    args: Vec::new(),
                    span,
                });
            }

            let args: Result<Vec<_>, _> = args
                .into_iter()
                .map(|a| fold_constants_at(a, registry, depth + 1))
                .collect();
            let args = args?;
            if args.iter().all(|a| matches!(a, Expr::Literal { .. })) {
                let values: Vec<f64> = args
                    .iter()
                    .filter_map(|a| match a {
                        Expr::Literal { value, .. } => Some(*value),
                        _ => None,
                    })
                    .collect();
                if values.len() == args.len() {
                    match call_function(&name, &values, registry) {
                        Ok(v) => return Ok(Expr::Literal { value: v, span }),
                        Err(e) if e.kind == ErrorKind::UnknownFunction => {}
                        Err(e) => return Err(e),
                    }
                }
            }
            Ok(Expr::Call { name, args, span })
        }
    }
}

fn fold_and_or(
    is_and: bool,
    left: Expr,
    right: Expr,
    span: crate::error::Span,
    registry: Option<&Registry>,
    depth: usize,
) -> Result<Expr, Error> {
    let left = fold_constants_at(left, registry, depth)?;
    if let Expr::Literal { value, .. } = &left {
        if is_and {
            if !is_truthy(*value) {
                return Ok(Expr::Literal { value: 0.0, span });
            }
        } else if is_truthy(*value) {
            return Ok(Expr::Literal { value: 1.0, span });
        }
        let right = fold_constants_at(right, registry, depth)?;
        if let Expr::Literal { value: rv, .. } = &right {
            return Ok(Expr::Literal {
                value: if is_truthy(*rv) { 1.0 } else { 0.0 },
                span,
            });
        }
        let op = if is_and { BinaryOp::And } else { BinaryOp::Or };
        let lit = if is_and { 1.0 } else { 0.0 };
        return Ok(Expr::Binary {
            op,
            left: Box::new(Expr::Literal {
                value: lit,
                span: left.span(),
            }),
            right: Box::new(right),
            span,
        });
    }
    // Dynamic left: still fold right for optimization (eval short-circuits at runtime).
    // Prefer leaving right unfolded when it might error? Runtime short-circuits, but
    // compile-time folding right could reject `x && (1/0)`. Keep right unfolded.
    Ok(Expr::Binary {
        op: if is_and { BinaryOp::And } else { BinaryOp::Or },
        left: Box::new(left),
        right: Box::new(right),
        span,
    })
}

pub(crate) fn eval_unary(op: UnaryOp, v: f64) -> Result<f64, Error> {
    let out = match op {
        UnaryOp::Plus => v,
        UnaryOp::Neg => -v,
        UnaryOp::Not => {
            if is_truthy(v) {
                0.0
            } else {
                1.0
            }
        }
    };
    finish(out)
}

pub(crate) fn eval_binary(
    op: BinaryOp,
    a: f64,
    b: f64,
    span: crate::error::Span,
) -> Result<f64, Error> {
    let out = match op {
        BinaryOp::Add => a + b,
        BinaryOp::Sub => a - b,
        BinaryOp::Mul => a * b,
        BinaryOp::Div => {
            if b == 0.0 {
                return Err(Error::new(ErrorKind::DivByZero, "division by zero").with_span(span));
            }
            a / b
        }
        BinaryOp::Mod => {
            if b == 0.0 {
                return Err(Error::new(ErrorKind::DivByZero, "modulo by zero").with_span(span));
            }
            a % b
        }
        BinaryOp::Pow => a.powf(b),
        BinaryOp::Lt => bool01(a < b),
        BinaryOp::Le => bool01(a <= b),
        BinaryOp::Gt => bool01(a > b),
        BinaryOp::Ge => bool01(a >= b),
        BinaryOp::Eq => bool01(a == b),
        BinaryOp::Ne => bool01(a != b),
        BinaryOp::And => bool01(is_truthy(a) && is_truthy(b)),
        BinaryOp::Or => bool01(is_truthy(a) || is_truthy(b)),
    };
    finish(out)
}

fn bool01(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
}

fn finish(v: f64) -> Result<f64, Error> {
    if v.is_nan() {
        Err(Error::new(ErrorKind::Nan, "expression produced NaN"))
    } else {
        Ok(v)
    }
}
