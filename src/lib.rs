//! Formula parser and evaluator for Tianshu.

#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod ast;
mod async_api;
mod batch;
mod builtins;
mod cache;
mod compile;
mod context;
mod error;
mod eval;
mod parser;
mod token;

pub use ast::{BinaryOp, Expr, UnaryOp};
pub use builtins::Registry;
pub use cache::{FormulaCache, DEFAULT_FORMULA_CACHE_CAPACITY};
pub use context::{Context, MapContext};
pub use error::{Error, ErrorKind, Span};
pub use eval::Formula;

pub const FORMULA_VERSION: u32 = 1;

/// Max AST nesting for parse / fold / eval. Deeper trees return `ErrorKind::Parse`
/// instead of overflowing the native stack.
pub const MAX_AST_DEPTH: usize = 256;

pub(crate) fn check_ast_depth(depth: usize) -> Result<(), Error> {
    if depth > MAX_AST_DEPTH {
        Err(Error::new(ErrorKind::Parse, "formula too deeply nested"))
    } else {
        Ok(())
    }
}
