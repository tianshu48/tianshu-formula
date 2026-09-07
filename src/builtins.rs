//! Built-in functions / constants and custom registry.

use std::collections::HashMap;
use std::f64::consts::{E, PI, TAU};
use std::sync::Arc;

use crate::error::{Error, ErrorKind};

pub type BuiltinFn = fn(&[f64]) -> Result<f64, Error>;

pub type CustomFn = Arc<dyn Fn(&[f64]) -> Result<f64, Error> + Send + Sync>;

#[derive(Clone, Default)]
pub struct Registry {
    custom: HashMap<String, CustomFn>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: impl Into<String>, f: CustomFn) -> Result<(), Error> {
        let name = name.into();
        if is_builtin_name(&name) {
            return Err(Error::new(
                ErrorKind::BuiltinOverride,
                format!("cannot override built-in function `{name}`"),
            )
            .with_name(name));
        }
        self.custom.insert(name, f);
        Ok(())
    }

    pub fn get_custom(&self, name: &str) -> Option<&CustomFn> {
        self.custom.get(name)
    }
}

pub fn builtin_constant(name: &str) -> Option<f64> {
    match name {
        "pi" => Some(PI),
        "e" => Some(E),
        "tau" => Some(TAU),
        "inf" => Some(f64::INFINITY),
        _ => None,
    }
}

pub fn is_builtin_name(name: &str) -> bool {
    builtin_constant(name).is_some() || builtin_fn(name).is_some()
}

pub fn builtin_fn(name: &str) -> Option<BuiltinFn> {
    Some(match name {
        "abs" => bi_abs,
        "sign" => bi_sign,
        "floor" => bi_floor,
        "ceil" => bi_ceil,
        "round" => bi_round,
        "trunc" => bi_trunc,
        "fract" => bi_fract,
        "min" => bi_min,
        "max" => bi_max,
        "sum" => bi_sum,
        "clamp" => bi_clamp,
        "sqrt" => bi_sqrt,
        "cbrt" => bi_cbrt,
        "pow" => bi_pow,
        "hypot" => bi_hypot,
        "exp" => bi_exp,
        "ln" => bi_ln,
        "log" => bi_log,
        "log2" => bi_log2,
        "log10" => bi_log10,
        "sin" => bi_sin,
        "cos" => bi_cos,
        "tan" => bi_tan,
        "asin" => bi_asin,
        "acos" => bi_acos,
        "atan" => bi_atan,
        "atan2" => bi_atan2,
        "deg" => bi_deg,
        "rad" => bi_rad,
        "if" => bi_if,
        "select" => bi_if,
        "lerp" => bi_lerp,
        "smoothstep" => bi_smoothstep,
        "step" => bi_step,
        _ => return None,
    })
}

fn arity(args: &[f64], expected: usize, name: &str) -> Result<(), Error> {
    if args.len() != expected {
        return Err(Error::new(
            ErrorKind::Arity,
            format!("{name} expects {expected} argument(s), got {}", args.len()),
        )
        .with_name(name));
    }
    Ok(())
}

fn arity_at_least(args: &[f64], min: usize, name: &str) -> Result<(), Error> {
    if args.len() < min {
        return Err(Error::new(
            ErrorKind::Arity,
            format!(
                "{name} expects at least {min} argument(s), got {}",
                args.len()
            ),
        )
        .with_name(name));
    }
    Ok(())
}

fn check_finite(v: f64, what: &str) -> Result<f64, Error> {
    if v.is_nan() {
        return Err(Error::new(ErrorKind::Nan, format!("{what} produced NaN")));
    }
    Ok(v)
}

fn reject_nan_args(args: &[f64], name: &str) -> Result<(), Error> {
    for a in args {
        if a.is_nan() {
            return Err(Error::new(ErrorKind::Nan, format!("{name} received NaN")).with_name(name));
        }
    }
    Ok(())
}

fn bi_abs(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "abs")?;
    reject_nan_args(args, "abs")?;
    Ok(args[0].abs())
}
fn bi_sign(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "sign")?;
    reject_nan_args(args, "sign")?;
    Ok(if args[0] == 0.0 {
        0.0
    } else if args[0] > 0.0 {
        1.0
    } else {
        -1.0
    })
}
fn bi_floor(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "floor")?;
    reject_nan_args(args, "floor")?;
    Ok(args[0].floor())
}
fn bi_ceil(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "ceil")?;
    reject_nan_args(args, "ceil")?;
    Ok(args[0].ceil())
}
fn bi_round(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "round")?;
    reject_nan_args(args, "round")?;
    Ok(args[0].round())
}
fn bi_trunc(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "trunc")?;
    reject_nan_args(args, "trunc")?;
    Ok(args[0].trunc())
}
fn bi_fract(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "fract")?;
    reject_nan_args(args, "fract")?;
    Ok(args[0].fract())
}
fn bi_min(args: &[f64]) -> Result<f64, Error> {
    arity_at_least(args, 1, "min")?;
    for a in args {
        if a.is_nan() {
            return Err(Error::new(ErrorKind::Nan, "min received NaN").with_name("min"));
        }
    }
    Ok(args.iter().copied().fold(f64::INFINITY, f64::min))
}
fn bi_max(args: &[f64]) -> Result<f64, Error> {
    arity_at_least(args, 1, "max")?;
    for a in args {
        if a.is_nan() {
            return Err(Error::new(ErrorKind::Nan, "max received NaN").with_name("max"));
        }
    }
    Ok(args.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}
fn bi_sum(args: &[f64]) -> Result<f64, Error> {
    arity_at_least(args, 1, "sum")?;
    reject_nan_args(args, "sum")?;
    check_finite(args.iter().copied().sum(), "sum")
}
fn bi_clamp(args: &[f64]) -> Result<f64, Error> {
    arity(args, 3, "clamp")?;
    let (x, lo, hi) = (args[0], args[1], args[2]);
    if x.is_nan() || lo.is_nan() || hi.is_nan() {
        return Err(Error::new(ErrorKind::Nan, "clamp received NaN").with_name("clamp"));
    }
    if lo > hi {
        return Err(Error::new(ErrorKind::Domain, "clamp: min > max").with_name("clamp"));
    }
    Ok(x.clamp(lo, hi))
}
fn bi_sqrt(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "sqrt")?;
    reject_nan_args(args, "sqrt")?;
    if args[0] < 0.0 {
        return Err(Error::new(ErrorKind::Domain, "sqrt of negative").with_name("sqrt"));
    }
    check_finite(args[0].sqrt(), "sqrt")
}
fn bi_cbrt(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "cbrt")?;
    reject_nan_args(args, "cbrt")?;
    check_finite(args[0].cbrt(), "cbrt")
}
fn bi_pow(args: &[f64]) -> Result<f64, Error> {
    arity(args, 2, "pow")?;
    reject_nan_args(args, "pow")?;
    check_finite(args[0].powf(args[1]), "pow")
}
fn bi_hypot(args: &[f64]) -> Result<f64, Error> {
    arity(args, 2, "hypot")?;
    reject_nan_args(args, "hypot")?;
    check_finite(args[0].hypot(args[1]), "hypot")
}
fn bi_exp(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "exp")?;
    reject_nan_args(args, "exp")?;
    check_finite(args[0].exp(), "exp")
}
fn bi_ln(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "ln")?;
    reject_nan_args(args, "ln")?;
    if args[0] <= 0.0 {
        return Err(Error::new(ErrorKind::Domain, "ln domain").with_name("ln"));
    }
    check_finite(args[0].ln(), "ln")
}
fn bi_log(args: &[f64]) -> Result<f64, Error> {
    arity(args, 2, "log")?;
    reject_nan_args(args, "log")?;
    if args[0] <= 0.0 || args[1] <= 0.0 || args[1] == 1.0 {
        return Err(Error::new(ErrorKind::Domain, "log domain").with_name("log"));
    }
    check_finite(args[0].log(args[1]), "log")
}
fn bi_log2(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "log2")?;
    reject_nan_args(args, "log2")?;
    if args[0] <= 0.0 {
        return Err(Error::new(ErrorKind::Domain, "log2 domain").with_name("log2"));
    }
    check_finite(args[0].log2(), "log2")
}
fn bi_log10(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "log10")?;
    reject_nan_args(args, "log10")?;
    if args[0] <= 0.0 {
        return Err(Error::new(ErrorKind::Domain, "log10 domain").with_name("log10"));
    }
    check_finite(args[0].log10(), "log10")
}
fn bi_sin(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "sin")?;
    reject_nan_args(args, "sin")?;
    check_finite(args[0].sin(), "sin")
}
fn bi_cos(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "cos")?;
    reject_nan_args(args, "cos")?;
    check_finite(args[0].cos(), "cos")
}
fn bi_tan(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "tan")?;
    reject_nan_args(args, "tan")?;
    check_finite(args[0].tan(), "tan")
}
fn bi_asin(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "asin")?;
    reject_nan_args(args, "asin")?;
    if !(-1.0..=1.0).contains(&args[0]) {
        return Err(Error::new(ErrorKind::Domain, "asin domain").with_name("asin"));
    }
    check_finite(args[0].asin(), "asin")
}
fn bi_acos(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "acos")?;
    reject_nan_args(args, "acos")?;
    if !(-1.0..=1.0).contains(&args[0]) {
        return Err(Error::new(ErrorKind::Domain, "acos domain").with_name("acos"));
    }
    check_finite(args[0].acos(), "acos")
}
fn bi_atan(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "atan")?;
    reject_nan_args(args, "atan")?;
    check_finite(args[0].atan(), "atan")
}
fn bi_atan2(args: &[f64]) -> Result<f64, Error> {
    arity(args, 2, "atan2")?;
    reject_nan_args(args, "atan2")?;
    check_finite(args[0].atan2(args[1]), "atan2")
}
fn bi_deg(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "deg")?;
    reject_nan_args(args, "deg")?;
    Ok(args[0].to_degrees())
}
fn bi_rad(args: &[f64]) -> Result<f64, Error> {
    arity(args, 1, "rad")?;
    reject_nan_args(args, "rad")?;
    Ok(args[0].to_radians())
}
fn bi_if(args: &[f64]) -> Result<f64, Error> {
    arity(args, 3, "if")?;
    // Cond NaN is falsy via is_truthy; still reject explicit NaN branches? Keep branches as-is.
    Ok(if is_truthy(args[0]) { args[1] } else { args[2] })
}
fn bi_lerp(args: &[f64]) -> Result<f64, Error> {
    arity(args, 3, "lerp")?;
    reject_nan_args(args, "lerp")?;
    Ok(args[0] + (args[1] - args[0]) * args[2])
}
fn bi_smoothstep(args: &[f64]) -> Result<f64, Error> {
    arity(args, 3, "smoothstep")?;
    reject_nan_args(args, "smoothstep")?;
    let (e0, e1, x) = (args[0], args[1], args[2]);
    if e0 == e1 {
        return Err(Error::new(ErrorKind::Domain, "smoothstep edges equal").with_name("smoothstep"));
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    Ok(t * t * (3.0 - 2.0 * t))
}
fn bi_step(args: &[f64]) -> Result<f64, Error> {
    arity(args, 2, "step")?;
    reject_nan_args(args, "step")?;
    Ok(if args[1] < args[0] { 0.0 } else { 1.0 })
}

/// Truthiness: `0` is false, anything else true.
#[inline]
pub fn is_truthy(v: f64) -> bool {
    v != 0.0 && !v.is_nan()
}

pub fn call_function(name: &str, args: &[f64], registry: Option<&Registry>) -> Result<f64, Error> {
    if let Some(f) = builtin_fn(name) {
        return f(args);
    }
    if let Some(reg) = registry {
        if let Some(f) = reg.get_custom(name) {
            return f(args).map_err(|e| {
                if e.kind == ErrorKind::Custom {
                    e
                } else {
                    Error::new(ErrorKind::Custom, e.message).with_name(name)
                }
            });
        }
    }
    Err(Error::new(
        ErrorKind::UnknownFunction,
        format!("unknown function `{name}`"),
    )
    .with_name(name))
}
