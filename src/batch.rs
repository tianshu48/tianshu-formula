//! Batch evaluation helpers.

use crate::builtins::Registry;
use crate::context::Context;
use crate::error::Error;
use crate::eval::Formula;

impl Formula {
    pub fn eval_batch<C: Context>(&self, contexts: &[C]) -> Result<Vec<f64>, Error> {
        self.eval_batch_with(contexts, None)
    }

    pub fn eval_batch_with<C: Context>(
        &self,
        contexts: &[C],
        registry: Option<&Registry>,
    ) -> Result<Vec<f64>, Error> {
        let mut out = Vec::with_capacity(contexts.len());
        for ctx in contexts {
            out.push(self.eval_with(ctx, registry)?);
        }
        Ok(out)
    }

    pub fn eval_batch_into<C: Context>(
        &self,
        contexts: &[C],
        out: &mut [f64],
    ) -> Result<(), Error> {
        self.eval_batch_into_with(contexts, out, None)
    }

    pub fn eval_batch_into_with<C: Context>(
        &self,
        contexts: &[C],
        out: &mut [f64],
        registry: Option<&Registry>,
    ) -> Result<(), Error> {
        if contexts.len() != out.len() {
            return Err(Error::new(
                crate::error::ErrorKind::Arity,
                format!(
                    "batch length mismatch: {} contexts vs {} outputs",
                    contexts.len(),
                    out.len()
                ),
            ));
        }
        for (i, ctx) in contexts.iter().enumerate() {
            out[i] = self.eval_with(ctx, registry)?;
        }
        Ok(())
    }
}
