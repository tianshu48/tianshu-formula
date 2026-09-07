//! Async API wrappers around the synchronous evaluator.

use crate::builtins::Registry;
use crate::context::Context;
use crate::error::Error;
use crate::eval::Formula;

impl Formula {
    pub async fn eval_async(&self, ctx: &dyn Context) -> Result<f64, Error> {
        self.eval(ctx)
    }

    pub async fn eval_async_with(
        &self,
        ctx: &dyn Context,
        registry: Option<&Registry>,
    ) -> Result<f64, Error> {
        self.eval_with(ctx, registry)
    }

    pub async fn eval_batch_async<C: Context>(&self, contexts: &[C]) -> Result<Vec<f64>, Error> {
        self.eval_batch(contexts)
    }

    pub async fn eval_batch_async_with<C: Context>(
        &self,
        contexts: &[C],
        registry: Option<&Registry>,
    ) -> Result<Vec<f64>, Error> {
        self.eval_batch_with(contexts, registry)
    }
}
